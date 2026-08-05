use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use bollard::models::ContainerSummary;
use futures_util::{stream, StreamExt};
use tokio::{
    sync::mpsc,
    task::{AbortHandle, JoinSet},
    time,
};

use crate::{
    action::ContainerAction,
    config::REFRESH_MS,
    docker::{
        container_meta, container_row, details, detect_gpus, event, image, log_output, network,
        raw_stats, read_host_memory, volume, DockerClient, GpuInfo, HostMemory, RawStats,
    },
    model::{
        ConnectionState, ContainerDetails, ContainerMeta, ContainerRow, DockerEvent, ImageRow,
        LogLine, NetworkRow, VolumeRow,
    },
};

const CHANNEL_CAPACITY: usize = 256;
const EVENT_CAPACITY: usize = 512;
const INVENTORY_INTERVAL: Duration = Duration::from_secs(30);
const STATS_CONCURRENCY: usize = 16;
const METADATA_CONCURRENCY: usize = 32;

#[derive(Debug)]
pub enum RuntimeCommand {
    Connect(String),
    Refresh,
    RefreshInventory,
    UpdateSettings { show_stopped: bool },
    Inspect(String),
    Action { id: String, action: ContainerAction },
    SubscribeLogs { id: String, follow: bool },
    UnsubscribeLogs,
    Shutdown,
}

#[derive(Debug)]
pub enum RuntimeEvent {
    Connection {
        state: ConnectionState,
        message: String,
    },
    Snapshot {
        containers: Vec<ContainerRow>,
    },
    Inventory {
        images: Vec<ImageRow>,
        volumes: Vec<VolumeRow>,
        networks: Vec<NetworkRow>,
        host_memory: HostMemory,
        gpu: GpuInfo,
    },
    Details(ContainerDetails),
    DockerEvent(DockerEvent),
    Log(LogLine),
    LogsEnded(String),
    ActionFinished {
        action: ContainerAction,
        name: String,
    },
    Error(String),
}

struct StatState {
    current: RawStats,
    previous: Option<RawStats>,
    sampled_at: Instant,
}

pub struct Supervisor {
    commands: mpsc::Receiver<RuntimeCommand>,
    events: mpsc::Sender<RuntimeEvent>,
    client: Option<Arc<DockerClient>>,
    socket: Option<String>,
    show_stopped: bool,
    summaries: Vec<ContainerSummary>,
    metadata: HashMap<String, ContainerMeta>,
    stats: HashMap<String, StatState>,
    running_ids: Vec<String>,
    stats_cursor: usize,
}

impl Supervisor {
    pub fn channels(
        show_stopped: bool,
    ) -> (mpsc::Sender<RuntimeCommand>, mpsc::Receiver<RuntimeEvent>, Self) {
        let (command_tx, command_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel(EVENT_CAPACITY);
        let supervisor = Self {
            commands: command_rx,
            events: event_tx,
            client: None,
            socket: None,
            show_stopped,
            summaries: Vec::new(),
            metadata: HashMap::new(),
            stats: HashMap::new(),
            running_ids: Vec::new(),
            stats_cursor: 0,
        };
        (command_tx, event_rx, supervisor)
    }

    pub async fn run(mut self) {
        let mut refresh_tick = self.refresh_tick();
        let mut inventory_tick = time::interval(INVENTORY_INTERVAL);
        inventory_tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        let mut fast_tasks: JoinSet<Result<FastWork>> = JoinSet::new();
        let mut inventory_tasks: JoinSet<Result<InventoryWork>> = JoinSet::new();
        let mut fast_running = false;
        let mut inventory_running = false;
        let mut log_task: Option<AbortHandle> = None;
        let mut event_task: Option<AbortHandle> = None;
        let mut connect_tick = time::interval(Duration::from_secs(3));
        connect_tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        self.emit(RuntimeEvent::Connection {
            state: ConnectionState::Connecting,
            message: "waiting for Docker socket".into(),
        });

        loop {
            tokio::select! {
                _ = connect_tick.tick(), if self.client.is_none() => self.try_connect(&mut event_task, &mut fast_tasks, &mut fast_running, &mut inventory_tasks, &mut inventory_running),
                _ = refresh_tick.tick(), if !fast_running && self.client.is_some() => self.start_fast(&mut fast_tasks, &mut fast_running),
                _ = inventory_tick.tick(), if !inventory_running && self.client.is_some() => self.start_inventory(&mut inventory_tasks, &mut inventory_running),
                Some(result) = fast_tasks.join_next(), if fast_running => {
                    fast_running = false;
                    match result {
                        Ok(Ok(work)) => {
                            self.apply_stats(work);
                            self.emit(RuntimeEvent::Connection { state: ConnectionState::Connected, message: "Docker daemon connected".into() });
                            self.emit(RuntimeEvent::Snapshot { containers: self.rows() });
                        }
                        Ok(Err(error)) => self.emit(RuntimeEvent::Connection { state: classify_error(&error), message: error.to_string() }),
                        Err(error) => self.emit(RuntimeEvent::Error(format!("stats task failed: {error}"))),
                    }
                }
                Some(result) = inventory_tasks.join_next(), if inventory_running => {
                    inventory_running = false;
                    match result {
                        Ok(Ok(work)) => {
                            self.summaries = work.summaries;
                            self.running_ids = self.summaries.iter().filter(|summary| summary.state.as_ref().map(|state| format!("{state:?}").eq_ignore_ascii_case("running")).unwrap_or(false)).filter_map(|summary| summary.id.clone()).collect();
                            self.stats.retain(|id, _| self.running_ids.iter().any(|running_id| running_id == id));
                            self.stats_cursor = 0;
                            self.metadata = work.metadata.into_iter().collect();
                            self.emit(RuntimeEvent::Connection { state: ConnectionState::Connected, message: "Docker daemon connected".into() });
                            self.emit(RuntimeEvent::Snapshot { containers: self.rows() });
                            self.emit(RuntimeEvent::Inventory { images: work.images, volumes: work.volumes, networks: work.networks, host_memory: work.host_memory, gpu: work.gpu });
                        }
                        Ok(Err(error)) => self.emit(RuntimeEvent::Connection { state: classify_error(&error), message: error.to_string() }),
                        Err(error) => self.emit(RuntimeEvent::Error(format!("inventory task failed: {error}"))),
                    }
                }
                command = self.commands.recv() => match command {
                    Some(RuntimeCommand::Connect(socket)) => {
                        self.socket = Some(socket);
                        self.try_connect(&mut event_task, &mut fast_tasks, &mut fast_running, &mut inventory_tasks, &mut inventory_running);
                    }
                    Some(RuntimeCommand::Refresh) => {
                        if self.client.is_none() { self.try_connect(&mut event_task, &mut fast_tasks, &mut fast_running, &mut inventory_tasks, &mut inventory_running); } else if self.summaries.is_empty() { self.start_inventory(&mut inventory_tasks, &mut inventory_running); } else { self.start_fast(&mut fast_tasks, &mut fast_running); }
                    }
                    Some(RuntimeCommand::RefreshInventory) => self.start_inventory(&mut inventory_tasks, &mut inventory_running),
                    Some(RuntimeCommand::UpdateSettings { show_stopped }) => {
                        self.show_stopped = show_stopped;
                        self.start_inventory(&mut inventory_tasks, &mut inventory_running);
                    }
                    Some(RuntimeCommand::Inspect(id)) => self.inspect(id),
                    Some(RuntimeCommand::Action { id, action }) => {
                        self.action(id, action);
                        self.start_inventory(&mut inventory_tasks, &mut inventory_running);
                    }
                    Some(RuntimeCommand::SubscribeLogs { id, follow }) => {
                        if let Some(handle) = log_task.take() { handle.abort(); }
                        log_task = Some(self.spawn_log_worker(id, follow));
                    }
                    Some(RuntimeCommand::UnsubscribeLogs) => { if let Some(handle) = log_task.take() { handle.abort(); } }
                    Some(RuntimeCommand::Shutdown) | None => {
                        if let Some(handle) = log_task.take() { handle.abort(); }
                        if let Some(handle) = event_task.take() { handle.abort(); }
                        fast_tasks.abort_all();
                        inventory_tasks.abort_all();
                        break;
                    }
                }
            }
        }
    }

    fn try_connect(
        &mut self,
        event_task: &mut Option<AbortHandle>,
        fast_tasks: &mut JoinSet<Result<FastWork>>,
        fast_running: &mut bool,
        inventory_tasks: &mut JoinSet<Result<InventoryWork>>,
        inventory_running: &mut bool,
    ) {
        let Some(socket) = self.socket.clone() else { return };
        match DockerClient::connect(socket.clone()) {
            Ok(client) => {
                self.client = Some(Arc::new(client));
                if event_task.is_none() {
                    *event_task = Some(self.spawn_event_worker());
                }
                self.start_inventory(inventory_tasks, inventory_running);
                self.start_fast(fast_tasks, fast_running);
            }
            Err(error) => {
                self.emit(RuntimeEvent::Connection {
                    state: classify_error(&error),
                    message: error.to_string(),
                });
            }
        }
    }

    fn refresh_tick(&self) -> time::Interval {
        let mut tick = time::interval(Duration::from_millis(REFRESH_MS));
        tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        tick
    }

    fn start_fast(&mut self, tasks: &mut JoinSet<Result<FastWork>>, running: &mut bool) {
        if *running || self.summaries.is_empty() {
            return;
        }
        let ids = self.next_stats_ids();
        if ids.is_empty() {
            return;
        }
        let Some(client) = &self.client else { return };
        *running = true;
        let client = Arc::clone(client);
        tasks.spawn(async move { collect_fast(client, ids).await });
    }

    fn start_inventory(&self, tasks: &mut JoinSet<Result<InventoryWork>>, running: &mut bool) {
        if *running {
            return;
        }
        let Some(client) = &self.client else { return };
        *running = true;
        let client = Arc::clone(client);
        let show_stopped = self.show_stopped;
        tasks.spawn(async move { collect_inventory(client, show_stopped).await });
    }

    fn next_stats_ids(&mut self) -> Vec<String> {
        let ids = &self.running_ids;
        if ids.is_empty() {
            return Vec::new();
        }
        // Keep the stats batch bounded to avoid hammering the daemon.
        let budget = STATS_CONCURRENCY * 2;
        let count = budget.min(ids.len());
        let selected = (0..count)
            .map(|offset| ids[(self.stats_cursor + offset) % ids.len()].clone())
            .collect();
        self.stats_cursor = (self.stats_cursor + count) % ids.len();
        selected
    }

    fn apply_stats(&mut self, work: FastWork) {
        for (id, raw) in work.samples {
            let now = Instant::now();
            let old = self.stats.remove(&id);
            let previous = old.as_ref().map(|state| state.current.clone());
            self.stats.insert(id, StatState { current: raw, previous, sampled_at: now });
        }
    }

    fn rows(&self) -> Vec<ContainerRow> {
        self.summaries
            .iter()
            .map(|summary| {
                let id = summary.id.clone().unwrap_or_default();
                let stats = self.stats.get(&id).map(|state| {
                    (
                        &state.current,
                        state.previous.as_ref(),
                        state.sampled_at.elapsed().as_secs_f64().max(0.001),
                    )
                });
                let meta = self.metadata.get(&id);
                container_row(summary, stats, meta)
            })
            .collect()
    }

    fn emit(&self, event: RuntimeEvent) {
        let _ = self.events.try_send(event);
    }

    fn inspect(&self, id: String) {
        let Some(client) = &self.client else { return };
        let client = Arc::clone(client);
        let events = self.events.clone();
        tokio::spawn(async move {
            match client.inspect(&id).await {
                Ok(value) => {
                    let _ = events.try_send(RuntimeEvent::Details(details(value)));
                }
                Err(error) => {
                    let _ =
                        events.try_send(RuntimeEvent::Error(format!("inspect container: {error}")));
                }
            }
        });
    }
    fn action(&self, id: String, action: ContainerAction) {
        let Some(client) = &self.client else { return };
        let client = Arc::clone(client);
        let events = self.events.clone();
        tokio::spawn(async move {
            match client.action(&id, action).await {
                Ok(()) => {
                    let _ = events.try_send(RuntimeEvent::ActionFinished { action, name: id });
                }
                Err(error) => {
                    let _ = events.try_send(RuntimeEvent::Error(format!(
                        "{} container: {error}",
                        action.label()
                    )));
                }
            }
        });
    }

    fn spawn_event_worker(&self) -> AbortHandle {
        let Some(client) = &self.client else { return tokio::spawn(async {}).abort_handle() };
        let client = Arc::clone(client);
        let events = self.events.clone();
        tokio::spawn(async move {
            loop {
                let mut stream = client.events();
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(value) => {
                            if events.try_send(RuntimeEvent::DockerEvent(event(value))).is_err() {
                                return;
                            }
                        }
                        Err(error) => {
                            let _ = events.try_send(RuntimeEvent::Connection {
                                state: classify_error(&error),
                                message: format!("event stream: {error}"),
                            });
                            break;
                        }
                    }
                }
                time::sleep(Duration::from_secs(2)).await;
            }
        })
        .abort_handle()
    }

    fn spawn_log_worker(&self, id: String, follow: bool) -> AbortHandle {
        let Some(client) = &self.client else { return tokio::spawn(async {}).abort_handle() };
        let client = Arc::clone(client);
        let events = self.events.clone();
        tokio::spawn(async move {
            let mut stream = client.logs(&id, follow);
            while let Some(item) = stream.next().await {
                match item {
                    Ok(value) => {
                        for line in log_output(value) {
                            if events.try_send(RuntimeEvent::Log(line)).is_err() {
                                return;
                            }
                        }
                    }
                    Err(error) => {
                        let _ = events.try_send(RuntimeEvent::LogsEnded(error.to_string()));
                        return;
                    }
                }
            }
            let _ = events.try_send(RuntimeEvent::LogsEnded("log stream ended".into()));
        })
        .abort_handle()
    }
}

#[derive(Debug)]
struct FastWork {
    samples: Vec<(String, RawStats)>,
}

#[derive(Debug)]
struct InventoryWork {
    summaries: Vec<ContainerSummary>,
    images: Vec<ImageRow>,
    volumes: Vec<VolumeRow>,
    networks: Vec<NetworkRow>,
    metadata: Vec<(String, ContainerMeta)>,
    host_memory: HostMemory,
    gpu: GpuInfo,
}

async fn collect_fast(client: Arc<DockerClient>, ids: Vec<String>) -> Result<FastWork> {
    let samples = stream::iter(ids.into_iter().map(|id| {
        let client = Arc::clone(&client);
        async move {
            let result = client.stats(&id).await;
            (id, result)
        }
    }))
    .buffer_unordered(STATS_CONCURRENCY)
    .filter_map(
        |(id, result)| async move { result.ok().flatten().map(|stats| (id, raw_stats(&stats))) },
    )
    .collect()
    .await;
    Ok(FastWork { samples })
}

async fn collect_inventory(client: Arc<DockerClient>, show_stopped: bool) -> Result<InventoryWork> {
    let (summaries, images, volumes, networks) = tokio::join!(
        client.containers(show_stopped),
        client.images(),
        client.volumes(),
        client.networks()
    );
    let summaries = summaries.context("collect containers")?;
    let ids = summaries.iter().filter_map(|summary| summary.id.clone()).collect::<Vec<_>>();
    let metadata = stream::iter(ids.into_iter().map(|id| {
        let client = Arc::clone(&client);
        async move {
            let result = client.inspect(&id).await.ok().map(|inspect| container_meta(&inspect));
            (id, result)
        }
    }))
    .buffer_unordered(METADATA_CONCURRENCY)
    .filter_map(|(id, meta)| async move { meta.map(|meta| (id, meta)) })
    .collect()
    .await;
    Ok(InventoryWork {
        summaries,
        images: images.context("collect images")?.into_iter().map(image).collect(),
        volumes: volumes.context("collect volumes")?.into_iter().map(volume).collect(),
        networks: networks.context("collect networks")?.into_iter().map(network).collect(),
        metadata,
        host_memory: read_host_memory(),
        gpu: detect_gpus(),
    })
}

fn classify_error(error: &impl std::fmt::Display) -> ConnectionState {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("permission") || message.contains("access denied") {
        ConnectionState::PermissionDenied
    } else if message.contains("connect")
        || message.contains("socket")
        || message.contains("no such file")
    {
        ConnectionState::Unavailable
    } else {
        ConnectionState::Error
    }
}
