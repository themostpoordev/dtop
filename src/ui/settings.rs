use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::app::App;

use super::{panel, yes_no, Theme};

pub(super) fn settings(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let entries = [
        ("Theme", app.config.theme.label().to_owned()),
        ("Default sort", app.config.sort.label().to_owned()),
        ("Show stopped", yes_no(app.config.show_stopped)),
        ("Follow logs", yes_no(app.config.follow_logs)),
        ("Density", app.config.density.label().to_owned()),
        ("Keybinding hints", yes_no(app.config.show_hints)),
        ("Show GPU status", yes_no(app.config.show_gpu)),
        ("Save settings", "Enter".into()),
        ("Reset defaults", "Enter".into()),
    ];
    let rows = entries
        .into_iter()
        .enumerate()
        .map(|(index, (label, value))| {
            let style = if index == app.settings_selection {
                Style::default().bg(theme.selected).fg(theme.text)
            } else {
                Style::default().fg(theme.text)
            };
            Row::new([
                if index == app.settings_selection {
                    format!("› {label}")
                } else {
                    format!("  {label}")
                },
                value,
            ])
            .style(style)
        })
        .collect::<Vec<_>>();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);
    frame.render_widget(
        Table::new(rows, [Constraint::Percentage(65), Constraint::Percentage(35)])
            .block(panel(theme, "settings · ←→ change · Enter save")),
        chunks[0],
    );
    let copy = vec![
        Line::from(Span::styled("local-only by design", theme.title())),
        Line::from("Docker requests use only the configured Unix socket."),
        Line::from("No telemetry, registry calls, or environment values."),
        Line::from(""),
        Line::from(Span::styled("fixed refresh", Style::default().fg(theme.muted))),
        Line::from("500 ms"),
    ];
    frame.render_widget(
        Paragraph::new(copy).block(panel(theme, "notes")).wrap(Wrap { trim: true }),
        chunks[1],
    );
}
