use ratatui::{
    layout::{Constraint, Rect},
    style::Style,
    widgets::{Cell, Row, Table},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{panel, Theme};

/// Per-physical-disk I/O: cumulative bytes + read/write rates.
pub(super) fn disk(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let disks = &app.data.host.disks;
    let headers = ["device", "read", "write", "read/s", "write/s"];
    let rows = disks
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
        Table::new(rows, widths)
            .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
            .block(panel(theme, &title))
            .column_spacing(1),
        area,
    );
}
