use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Sparkline, Table},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{gradient_bars, panel, Theme};

/// Per-physical-disk I/O: cumulative bytes, rates, and a gradient rate history.
pub(super) fn disk(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(6)])
        .split(area);

    // Read/write rate history as gradient lines, normalized to the max seen.
    let read_history = app.data.host_history.disk_read.as_slice();
    let write_history = app.data.host_history.disk_write.as_slice();
    let max_rate =
        read_history.iter().chain(write_history.iter()).copied().fold(0.0f64, f64::max).max(1.0);
    let read_norm = read_history.iter().map(|v| (v / max_rate * 100.0) as u64).collect::<Vec<_>>();
    let write_norm =
        write_history.iter().map(|v| (v / max_rate * 100.0) as u64).collect::<Vec<_>>();

    let graph = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(rows[0]);
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&read_norm, theme.good, theme.accent, 100))
            .max(100)
            .style(Style::default().fg(theme.good))
            .block(panel(theme, "read")),
        graph[0],
    );
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&write_norm, theme.accent, theme.good, 100))
            .max(100)
            .style(Style::default().fg(theme.accent))
            .block(panel(theme, "write")),
        graph[1],
    );

    let disks = &app.data.host.disks;
    let headers = ["device", "read", "write", "read/s", "write/s"];
    let table_rows = disks
        .iter()
        .enumerate()
        .map(|(i, d)| {
            Row::new([
                Cell::from(d.name.clone()),
                Cell::from(format_bytes(d.read_bytes)),
                Cell::from(format_bytes(d.write_bytes)),
                Cell::from(format!("{}/s", format_bytes(d.read_rate.max(0.0) as u64))),
                Cell::from(format!("{}/s", format_bytes(d.write_rate.max(0.0) as u64))),
            ])
            .style(if i % 2 == 1 {
                Style::default().bg(theme.surface_alt)
            } else {
                Style::default()
            })
        })
        .collect::<Vec<_>>();
    let widths = [
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(14),
    ];
    let title = format!("{} disks", disks.len());
    frame.render_widget(
        Table::new(table_rows, widths)
            .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
            .block(panel(theme, &title))
            .column_spacing(1),
        rows[1],
    );

    // Empty state: no disks detected — keep the panel informative.
    if disks.is_empty() {
        let empty = Line::from(Span::styled(
            "no physical disks detected",
            Style::default().fg(theme.muted),
        ));
        frame.render_widget(Paragraph::new(empty), rows[1]);
    }
}
