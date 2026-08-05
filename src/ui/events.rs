use ratatui::{
    layout::{Constraint, Rect},
    widgets::{Row, Table},
    Frame,
};

use crate::app::App;

use super::{panel, Theme};

pub(super) fn events(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = app
        .data
        .events
        .items
        .iter()
        .rev()
        .filter(|event| {
            app.event_filter.is_empty()
                || format!("{} {} {}", event.kind, event.action, event.actor)
                    .to_ascii_lowercase()
                    .contains(&app.event_filter.to_ascii_lowercase())
        })
        .map(|event| {
            Row::new([
                event.time(),
                event.kind.clone(),
                event.action.clone(),
                event.actor.chars().take(16).collect(),
                event.attributes.clone(),
            ])
        })
        .collect::<Vec<_>>();
    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(16),
            Constraint::Length(18),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(["when", "type", "action", "actor", "attributes"])
            .style(theme.title().bg(theme.surface_alt)),
    )
    .block(panel(theme, "Docker events"));
    frame.render_widget(table, area);
}
