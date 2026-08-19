use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Sparkline, Table},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{gradient_bars, panel, Theme};

/// Per-interface rx/tx rates + gradient rate history.
pub(super) fn network(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(6)])
        .split(area);

    let nets = &app.data.host.nets;
    let rx_total: f64 = nets.iter().map(|n| n.rx_rate).sum();
    let tx_total: f64 = nets.iter().map(|n| n.tx_rate).sum();

    // Gradient history of total rx/tx over time, normalized to the max seen.
    let rx_history = app.data.host_history.net_rx.as_slice();
    let tx_history = app.data.host_history.net_tx.as_slice();
    let max_rate = rx_history
        .iter()
        .chain(tx_history.iter())
        .copied()
        .fold(0.0f64, f64::max)
        .max(1.0)
        .max(rx_total.max(tx_total));
    let rx_norm = rx_history.iter().map(|v| (v / max_rate * 100.0) as u64).collect::<Vec<_>>();
    let tx_norm = tx_history.iter().map(|v| (v / max_rate * 100.0) as u64).collect::<Vec<_>>();

    // Header + two gradient lines (rx on top, tx below) — each gets a real
    // drawable row (borders take 2 rows of the panel).
    let graph = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(3)])
        .split(rows[0]);
    let header = vec![
        Line::from(vec![
            Span::styled("rx ", Style::default().fg(theme.good)),
            Span::styled(
                format!("{}/s", format_bytes(rx_total.max(0.0) as u64)),
                Style::default().fg(theme.text),
            ),
            Span::styled("  tx ", Style::default().fg(theme.accent)),
            Span::styled(
                format!("{}/s", format_bytes(tx_total.max(0.0) as u64)),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(""),
    ];
    frame.render_widget(Paragraph::new(header).block(panel(theme, "network")), graph[0]);
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&rx_norm, theme.good, theme.accent, 100))
            .max(100)
            .style(Style::default().fg(theme.good))
            .block(panel(theme, "rx")),
        graph[1],
    );
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&tx_norm, theme.accent, theme.good, 100))
            .max(100)
            .style(Style::default().fg(theme.accent))
            .block(panel(theme, "tx")),
        graph[2],
    );

    // Interface table — bounded to viewport.
    let headers = ["interface", "rx", "tx", "rx/s", "tx/s"];
    let available = rows[1].height.saturating_sub(2) as usize;
    let table_rows = nets
        .iter()
        .take(available)
        .map(|n| {
            Row::new([
                Cell::from(n.name.clone()),
                Cell::from(format_bytes(n.rx_bytes)),
                Cell::from(format_bytes(n.tx_bytes)),
                Cell::from(format!("{}/s", format_bytes(n.rx_rate.max(0.0) as u64))),
                Cell::from(format!("{}/s", format_bytes(n.tx_rate.max(0.0) as u64))),
            ])
        })
        .collect::<Vec<_>>();
    let widths = [
        Constraint::Length(16),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(14),
    ];
    frame.render_widget(
        Table::new(table_rows, widths)
            .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
            .block(panel(theme, &format!("{} interfaces", nets.len())))
            .column_spacing(1),
        rows[1],
    );
}
