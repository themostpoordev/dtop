#![forbid(unsafe_code)]

use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use crossterm::event::{Event as CEvent, EventStream, KeyEventKind};
use dtop::{
    app::App,
    config::Config,
    runtime::{RuntimeCommand, RuntimeEvent, Supervisor},
    terminal::TerminalGuard,
    ui,
};
use futures_util::StreamExt;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Default)]
struct Cli {
    config: Option<PathBuf>,
    socket: Option<String>,
}

fn parse_cli() -> Result<Cli> {
    let mut cli = Cli::default();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-c" | "--config" => {
                cli.config = Some(PathBuf::from(args.next().context("--config requires a path")?))
            }
            "-s" | "--socket" => {
                cli.socket = Some(args.next().context("--socket requires a Unix socket path")?)
            }
            "-h" | "--help" => {
                println!(
                    "dtop\n\nUsage: dtop [--config PATH] [--socket PATH]\n\nDocker access is local-only through a Unix socket."
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    Ok(cli)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();
    let cli = parse_cli()?;
    let config_path = Config::path_from_arg(cli.config)?;
    let mut config = Config::load(&config_path).context("load dtop configuration")?;
    if let Some(socket) = cli.socket {
        config.docker_socket = socket;
        config = config.validate()?;
    }

    // Enter the TUI first so the app starts fast and shows connection state
    // in the UI instead of exiting when the daemon is down.
    let mut terminal = TerminalGuard::enter()?;
    let (command_tx, mut event_rx, supervisor) = Supervisor::channels(config.show_stopped);
    let supervisor_task = tokio::spawn(supervisor.run());
    let socket = config.docker_socket.clone();
    let mut app = App::new(config, config_path, command_tx.clone());
    command_tx.send(RuntimeCommand::Connect(socket)).await.ok();
    let result = run_loop(&mut terminal, &mut app, &mut event_rx).await;
    let _ = app.config.save(&app.config_path);
    command_tx.send(RuntimeCommand::Shutdown).await.ok();
    supervisor_task.abort();
    result
}

async fn run_loop(
    terminal: &mut TerminalGuard,
    app: &mut App,
    event_rx: &mut tokio::sync::mpsc::Receiver<RuntimeEvent>,
) -> Result<()> {
    use tokio::time::{interval, Duration, MissedTickBehavior};

    let mut keys = EventStream::new();
    let mut dirty = true;
    // Poll the event channel on a fixed cadence. Do NOT use recv() as a select
    // arm: when another arm wins, the pending recv is cancelled and buffered
    // messages stop waking us — the UI goes stale. A short interval keeps
    // draining even under constant key input.
    let mut drain_tick = interval(Duration::from_millis(16));
    drain_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = drain_tick.tick() => {
                while let Ok(event) = event_rx.try_recv() {
                    app.apply_runtime_event(event);
                    dirty = true;
                }
            }
            _ = signal::ctrl_c() => { app.should_quit = true; dirty = true; }
            maybe_event = keys.next() => {
                if let Some(Ok(CEvent::Key(key))) = maybe_event {
                    if key.kind == KeyEventKind::Press {
                        app.handle_key(key).await?;
                        dirty = true;
                    }
                }
            }
        }
        if dirty {
            terminal.terminal.draw(|frame| ui::render(frame, app)).context("render terminal")?;
            dirty = false;
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
