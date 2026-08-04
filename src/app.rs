use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::{
    action::ContainerAction,
    config::{Config, SortOrder, ThemeName, MIN_REFRESH_MS},
    model::{AppData, ConnectionState, Screen},
    runtime::{RuntimeCommand, RuntimeEvent},
};

pub struct App {
    pub config: Config,
    pub config_path: PathBuf,
    pub data: AppData,
    pub screen: Screen,
    pub connection: ConnectionState,
    pub selected_container: usize,
    pub home_selection: usize,
    pub settings_selection: usize,
    pub filter: String,
    pub filter_mode: bool,
    pub event_filter: String,
    pub log_filter: String,
    pub log_follow: bool,
    pub log_scroll: u16,
    pub notice: Option<String>,
    pub confirm: Option<(ContainerAction, String)>,
    pub pending_action: bool,
    pub should_quit: bool,
    command_tx: mpsc::Sender<RuntimeCommand>,
}

impl App {
    pub fn new(
        config: Config,
        config_path: PathBuf,
        command_tx: mpsc::Sender<RuntimeCommand>,
    ) -> Self {
        let log_follow = config.follow_logs;
        Self {
            config,
            config_path,
            data: AppData::default(),
            screen: Screen::Home,
            connection: ConnectionState::default(),
            selected_container: 0,
            home_selection: 0,
            settings_selection: 0,
            filter: String::new(),
            filter_mode: false,
            event_filter: String::new(),
            log_filter: String::new(),
            log_follow,
            log_scroll: 0,
            notice: None,
            confirm: None,
            pending_action: false,
            should_quit: false,
            command_tx,
        }
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.confirm.is_some() {
            return self.handle_confirmation(key).await;
        }
        if self.filter_mode {
            return self.handle_filter_key(key);
        }
        match key.code {
            KeyCode::Char('q')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::CONTROL =>
            {
                self.should_quit = true
            }
            KeyCode::Esc => {
                self.screen = Screen::Home;
                self.command_tx.send(RuntimeCommand::UnsubscribeLogs).await.ok();
            }
            KeyCode::Tab => self.screen = self.screen.next_primary(),
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.notice =
                    Some("Tab sections · Esc home · arrows select · Enter open · q quit".into())
            }
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::PageUp => self.log_scroll = self.log_scroll.saturating_sub(10),
            KeyCode::PageDown => self.log_scroll = self.log_scroll.saturating_add(10),
            KeyCode::Enter => self.enter().await?,
            KeyCode::Char('/')
                if matches!(self.screen, Screen::Containers | Screen::Events | Screen::Logs) =>
            {
                self.filter_mode = true
            }
            KeyCode::Char('d') if self.screen == Screen::Containers => self.open_details().await?,
            KeyCode::Char('l') if self.screen == Screen::Containers => self.open_logs().await?,
            KeyCode::Char('s') if self.screen == Screen::Containers => {
                self.request_action(ContainerAction::Start).await?
            }
            KeyCode::Char('x') if self.screen == Screen::Containers => {
                self.request_action(ContainerAction::Stop).await?
            }
            KeyCode::Char('r') if self.screen == Screen::Containers => {
                self.request_action(ContainerAction::Restart).await?
            }
            KeyCode::Char('p') if self.screen == Screen::Containers => {
                self.request_action(ContainerAction::Pause).await?
            }
            KeyCode::Char('u') if self.screen == Screen::Containers => {
                self.request_action(ContainerAction::Unpause).await?
            }
            KeyCode::Left | KeyCode::Right if self.screen == Screen::Settings => {
                self.change_setting(key.code == KeyCode::Right)
            }
            KeyCode::Char(' ') if self.screen == Screen::Logs => {
                self.log_follow = !self.log_follow;
                self.subscribe_logs().await?;
            }
            KeyCode::Char('c') if self.screen == Screen::Logs => self.data.logs.clear(),
            KeyCode::Char('C') if self.screen == Screen::Events => self.data.events.clear(),
            _ => {}
        }
        Ok(())
    }

    pub fn apply_runtime_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Connection { state, message } => {
                self.connection = state;
                self.notice = Some(message);
            }
            RuntimeEvent::Snapshot { containers } => {
                self.data.containers = containers;
                if self.selected_container >= self.data.containers.len() {
                    self.selected_container = self.data.containers.len().saturating_sub(1);
                }
            }
            RuntimeEvent::Inventory { images, volumes, networks, host_memory } => {
                self.data.images = images;
                self.data.volumes = volumes;
                self.data.networks = networks;
                self.data.host_memory = host_memory;
            }
            RuntimeEvent::Details(details) => self.data.details = Some(details),
            RuntimeEvent::DockerEvent(event) => self.data.events.push(event),
            RuntimeEvent::Log(line) => {
                self.data.logs.push(line);
                if self.log_follow {
                    self.log_scroll = u16::MAX;
                }
            }
            RuntimeEvent::LogsEnded(message) => self.notice = Some(message),
            RuntimeEvent::ActionFinished { action, name } => {
                self.pending_action = false;
                self.notice = Some(format!("{} completed for {name}", action.label()));
            }
            RuntimeEvent::Error(message) => {
                self.pending_action = false;
                self.notice = Some(message);
            }
        }
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        let mut indices = self
            .data
            .containers
            .iter()
            .enumerate()
            .filter(|(_, container)| {
                self.config.show_stopped
                    || container.state == "running"
                    || container.state == "paused"
            })
            .filter(|(_, container)| self.filter.is_empty() || container.matches(&self.filter))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        match self.config.sort {
            SortOrder::Name => {
                indices.sort_by_key(|index| self.data.containers[*index].name.to_ascii_lowercase())
            }
            SortOrder::Cpu => indices.sort_by(|a, b| {
                self.data.containers[*b]
                    .metrics
                    .cpu_percent
                    .total_cmp(&self.data.containers[*a].metrics.cpu_percent)
            }),
            SortOrder::Memory => indices.sort_by(|a, b| {
                self.data.containers[*b]
                    .metrics
                    .memory_bytes
                    .cmp(&self.data.containers[*a].metrics.memory_bytes)
            }),
            SortOrder::Uptime => indices.sort_by(|a, b| {
                self.data.containers[*b].started.cmp(&self.data.containers[*a].started)
            }),
            SortOrder::Status => {
                indices.sort_by_key(|index| self.data.containers[*index].state.clone())
            }
        }
        indices
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        self.data.containers.iter().fold((0, 0, 0), |mut counts, container| {
            match container.state.as_str() {
                "running" => counts.0 += 1,
                "paused" => counts.2 += 1,
                _ => counts.1 += 1,
            };
            counts
        })
    }

    pub fn memory_summary(&self) -> (u64, u64) {
        let mut used = 0u64;
        let mut limit = 0u64;
        for container in &self.data.containers {
            used += container.metrics.memory_bytes;
            limit = limit.max(container.metrics.memory_limit);
        }
        (used, limit)
    }

    pub async fn enter(&mut self) -> Result<()> {
        match self.screen {
            Screen::Home => match self.home_selection {
                0 => self.screen = Screen::Overview,
                1 => self.screen = Screen::Containers,
                2 => self.screen = Screen::Events,
                3 => self.screen = Screen::Images,
                4 => self.screen = Screen::Volumes,
                5 => self.screen = Screen::Networks,
                _ => self.screen = Screen::Settings,
            },
            Screen::Containers => self.open_details().await?,
            Screen::Settings => {
                if self.settings_selection == 7 {
                    self.save_settings()?;
                } else if self.settings_selection == 8 {
                    self.config = Config::default();
                    self.notice = Some("settings reset to defaults".into());
                    self.push_settings().await;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn open_details(&mut self) -> Result<()> {
        if let Some(id) = self.selected_id() {
            self.screen = Screen::Details;
            self.command_tx.send(RuntimeCommand::Inspect(id)).await?;
        } else {
            self.notice = Some("select a container first".into());
        }
        Ok(())
    }
    async fn open_logs(&mut self) -> Result<()> {
        if self.selected_id().is_some() {
            self.screen = Screen::Logs;
            self.log_scroll = 0;
            self.data.logs.clear();
            self.subscribe_logs().await?;
        } else {
            self.notice = Some("select a container first".into());
        }
        Ok(())
    }
    async fn subscribe_logs(&mut self) -> Result<()> {
        if let Some(id) = self.selected_id() {
            self.command_tx
                .send(RuntimeCommand::SubscribeLogs { id, follow: self.log_follow })
                .await?;
        }
        Ok(())
    }
    async fn request_action(&mut self, action: ContainerAction) -> Result<()> {
        if self.pending_action {
            return Ok(());
        }
        let Some(container) = self.selected_container_row() else {
            self.notice = Some("select a container first".into());
            return Ok(());
        };
        if !action.available_for(container) {
            self.notice =
                Some(format!("{} is not available for {}", action.label(), container.name));
            return Ok(());
        }
        if action.requires_confirmation() {
            self.confirm = Some((action, container.name.clone()));
        } else {
            self.execute_action(action).await?;
        }
        Ok(())
    }
    async fn execute_action(&mut self, action: ContainerAction) -> Result<()> {
        if let Some(id) = self.selected_id() {
            self.pending_action = true;
            self.command_tx.send(RuntimeCommand::Action { id, action }).await?;
        }
        Ok(())
    }
    async fn handle_confirmation(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                if let Some((action, _)) = self.confirm.take() {
                    self.execute_action(action).await?;
                }
            }
            KeyCode::Esc => self.confirm = None,
            _ => {}
        }
        Ok(())
    }
    fn handle_filter_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => self.filter_mode = false,
            KeyCode::Backspace => {
                self.active_filter_mut().pop();
            }
            KeyCode::Char(c) => self.active_filter_mut().push(c),
            _ => {}
        }
        Ok(())
    }
    fn active_filter_mut(&mut self) -> &mut String {
        match self.screen {
            Screen::Events => &mut self.event_filter,
            Screen::Logs => &mut self.log_filter,
            _ => &mut self.filter,
        }
    }
    fn move_up(&mut self) {
        match self.screen {
            Screen::Home => self.home_selection = self.home_selection.saturating_sub(1),
            Screen::Settings => self.settings_selection = self.settings_selection.saturating_sub(1),
            Screen::Containers => {
                let visible = self.visible_indices();
                if let Some(position) =
                    visible.iter().position(|index| *index == self.selected_container)
                {
                    if position > 0 {
                        self.selected_container = visible[position - 1];
                    }
                }
            }
            _ => self.log_scroll = self.log_scroll.saturating_sub(1),
        }
    }
    fn move_down(&mut self) {
        match self.screen {
            Screen::Home => self.home_selection = (self.home_selection + 1).min(6),
            Screen::Settings => self.settings_selection = (self.settings_selection + 1).min(8),
            Screen::Containers => {
                let visible = self.visible_indices();
                if let Some(position) =
                    visible.iter().position(|index| *index == self.selected_container)
                {
                    if position + 1 < visible.len() {
                        self.selected_container = visible[position + 1];
                    }
                } else if let Some(first) = visible.first() {
                    self.selected_container = *first;
                }
            }
            _ => self.log_scroll = self.log_scroll.saturating_add(1),
        }
    }
    fn selected_container_row(&self) -> Option<&crate::model::ContainerRow> {
        self.data.containers.get(self.selected_container)
    }
    fn selected_id(&self) -> Option<String> {
        self.selected_container_row().map(|container| container.id.clone())
    }
    fn change_setting(&mut self, right: bool) {
        match self.settings_selection {
            0 => {
                self.config.theme =
                    if right { self.config.theme.next() } else { previous_theme(self.config.theme) }
            }
            1 => {
                self.config.refresh_ms = if right {
                    self.config.refresh_ms.saturating_add(50)
                } else {
                    self.config.refresh_ms.saturating_sub(50).max(MIN_REFRESH_MS)
                };
            }
            2 => {
                self.config.sort =
                    if right { self.config.sort.next() } else { previous_sort(self.config.sort) }
            }
            3 => self.config.show_stopped = !self.config.show_stopped,
            4 => self.config.follow_logs = !self.config.follow_logs,
            5 => self.config.density = self.config.density.toggle(),
            6 => self.config.show_hints = !self.config.show_hints,
            _ => {}
        }
    }
    fn save_settings(&mut self) -> Result<()> {
        self.config.save(&self.config_path)?;
        self.notice = Some(format!("saved {}", self.config_path.display()));
        Ok(())
    }
    async fn push_settings(&self) {
        let _ = self
            .command_tx
            .send(RuntimeCommand::UpdateSettings {
                refresh_ms: self.config.refresh_ms,
                show_stopped: self.config.show_stopped,
            })
            .await;
    }
}

fn previous_theme(value: ThemeName) -> ThemeName {
    let all = ThemeName::ALL;
    let index = all.iter().position(|item| *item == value).unwrap_or(0);
    all[(index + all.len() - 1) % all.len()]
}
fn previous_sort(value: SortOrder) -> SortOrder {
    let all = SortOrder::ALL;
    let index = all.iter().position(|item| *item == value).unwrap_or(0);
    all[(index + all.len() - 1) % all.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_indices_filter_and_sort() {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::new(Config::default(), PathBuf::from("/tmp/dtop-test.toml"), tx);
        app.data.containers = vec![
            crate::model::ContainerRow {
                name: "zeta".into(),
                state: "running".into(),
                ..Default::default()
            },
            crate::model::ContainerRow {
                name: "alpha".into(),
                state: "exited".into(),
                ..Default::default()
            },
        ];
        app.config.sort = SortOrder::Name;
        assert_eq!(app.visible_indices(), vec![1, 0]);
        app.filter = "z".into();
        assert_eq!(app.visible_indices(), vec![0]);
    }
}
