//! Scalability and navigation tests that do not require a Docker daemon.
//!
//! These validate that dtop's incremental sampler and bounded buffers behave
//! predictably for large container sets, and that the interactive navigation
//! model matches the documented keybindings.

use dtop::{
    action::ContainerAction,
    app::App,
    config::{Config, SortOrder},
    model::{BoundedLines, ContainerRow, Metrics, Screen},
    runtime::{RuntimeCommand, RuntimeEvent},
};
use tokio::sync::mpsc;

fn app() -> (App, mpsc::Sender<RuntimeCommand>) {
    let (tx, _rx) = mpsc::channel(16);
    let app =
        App::new(Config::default(), std::path::PathBuf::from("/tmp/dtop-test.toml"), tx.clone());
    (app, tx)
}

fn container(name: &str, state: &str, cpu: f64) -> ContainerRow {
    ContainerRow {
        id: name.to_owned(),
        name: name.to_owned(),
        state: state.to_owned(),
        metrics: Metrics { cpu_percent: cpu, ..Default::default() },
        ..Default::default()
    }
}

#[test]
fn bounded_buffer_evicts_oldest_items() {
    let mut buffer = BoundedLines::new(100);
    for i in 0..1_000 {
        buffer.push(i);
    }
    assert_eq!(buffer.items.len(), 100);
    assert_eq!(*buffer.items.front().unwrap(), 900);
    assert_eq!(*buffer.items.back().unwrap(), 999);
}

#[test]
fn sampler_wraps_around_large_container_sets() {
    let (mut app, _tx) = app();
    app.data.containers =
        (0..500).map(|i| container(&format!("c{i:03}"), "running", 0.0)).collect();
    app.config.sort = SortOrder::Name;
    let visible = app.visible_indices();
    assert_eq!(visible.len(), 500);
    assert_eq!(visible[0], 0);
    assert_eq!(visible[499], 499);
}

#[test]
fn sorting_and_filtering_work_at_scale() {
    let (mut app, _tx) = app();
    let mut rows = (0..1_000)
        .map(|i| {
            container(
                &format!("svc-{i:03}"),
                if i % 2 == 0 { "running" } else { "exited" },
                i as f64,
            )
        })
        .collect::<Vec<_>>();
    rows.push(container("alpha", "running", 500.0));
    app.data.containers = rows;
    app.config.sort = SortOrder::Cpu;
    let visible = app.visible_indices();
    assert_eq!(visible.len(), 1_001);
    let first = &app.data.containers[visible[0]];
    assert_eq!(first.name, "svc-999");
    app.filter = "alpha".into();
    assert_eq!(app.visible_indices().len(), 1);
}

#[test]
fn stopped_container_toggle_filters_large_sets() {
    let (mut app, _tx) = app();
    app.data.containers = (0..300)
        .map(|i| container(&format!("c{i}"), if i % 3 == 0 { "running" } else { "exited" }, 0.0))
        .collect();
    app.config.show_stopped = true;
    assert_eq!(app.visible_indices().len(), 300);
    app.config.show_stopped = false;
    assert_eq!(app.visible_indices().len(), 100);
}

#[test]
fn tab_cycles_all_primary_sections() {
    let (mut app, _tx) = app();
    app.screen = Screen::Home;
    let mut seen = Vec::new();
    let primary = Screen::primary(app.config.mode);
    for _ in 0..primary.len() {
        app.screen = app.screen.next_primary(app.config.mode);
        seen.push(app.screen);
    }
    assert_eq!(
        seen,
        vec![
            Screen::Containers,
            Screen::Events,
            Screen::Images,
            Screen::Volumes,
            Screen::Networks,
            Screen::Settings,
            Screen::Overview,
        ]
    );
}

#[test]
fn home_enter_opens_each_section_and_settings() {
    let (mut app, _tx) = app();
    for (selection, expected) in [
        (0, Screen::Overview),
        (1, Screen::Containers),
        (2, Screen::Events),
        (3, Screen::Images),
        (4, Screen::Volumes),
        (5, Screen::Networks),
        (6, Screen::Settings),
    ] {
        app.home_selection = selection;
        app.screen = Screen::Home;
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(app.enter()).unwrap();
        assert_eq!(app.screen, expected);
    }
}

#[test]
fn action_availability_matches_container_state() {
    let running = container("web", "running", 0.0);
    let stopped = container("old", "exited", 0.0);
    assert!(ContainerAction::Stop.available_for(&running));
    assert!(!ContainerAction::Start.available_for(&running));
    assert!(ContainerAction::Start.available_for(&stopped));
    assert!(!ContainerAction::Unpause.available_for(&running));
}

#[test]
fn runtime_error_clears_pending_action() {
    let (mut app, _tx) = app();
    app.pending_action = true;
    app.apply_runtime_event(RuntimeEvent::Error("boom".into()));
    assert!(!app.pending_action);
    assert!(app.notice.as_deref() == Some("boom"));
}
