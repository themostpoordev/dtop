use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Sparkline, Table},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{gradient_bars, panel, Theme};

/// Network screen — readable at a glance, useful when idle.
///
/// - Header: live rx/tx + packet rates, cumulative totals, and peaks, with an
///   explicit `idle` tag instead of a row of zeros that looks broken.
/// - Two gradient histories whose titles carry current rate + peak.
/// - Per-interface table (busiest first): totals, rates, packet rate, and a
///   combined error/drop counter that stays out of the way (`—`) on healthy
///   links and turns red the moment something is wrong.
pub(super) fn network(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Length(6), Constraint::Min(6)])
        .split(area);

    let nets = &app.data.host.nets;
    let history = &app.data.host_history;
    let rx_total: f64 = nets.iter().map(|n| n.rx_rate).sum();
    let tx_total: f64 = nets.iter().map(|n| n.tx_rate).sum();
    let pps_total: f64 = nets.iter().map(|n| n.rx_pps + n.tx_pps).sum();
    let rx_cum: u64 = nets.iter().map(|n| n.rx_bytes).sum();
    let tx_cum: u64 = nets.iter().map(|n| n.tx_bytes).sum();
    let rx_peak = history.net_rx.peak();
    let tx_peak = history.net_tx.peak();
    let err_total: u64 = nets.iter().map(|n| n.err_total).sum();
    let idle = rx_total < 1.0 && tx_total < 1.0 && rx_peak < 1.0 && tx_peak < 1.0;

    let summary = vec![
        Line::from(vec![
            Span::styled("rx  ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{:>10}/s", format_bytes(rx_total.max(0.0) as u64)),
                Style::default().fg(theme.good),
            ),
            Span::styled("   tx ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{:>10}/s", format_bytes(tx_total.max(0.0) as u64)),
                Style::default().fg(theme.accent),
            ),
            Span::styled("   pkt/s ", Style::default().fg(theme.muted)),
            Span::styled(format!("{pps_total:.0}"), Style::default().fg(theme.text)),
            if err_total > 0 {
                Span::styled(format!("   err/drop {err_total}"), Style::default().fg(theme.bad))
            } else {
                Span::styled("   err/drop —", Style::default().fg(theme.muted))
            },
        ]),
        Line::from(vec![
            Span::styled("total ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("↓ {}  ↑ {}", format_bytes(rx_cum), format_bytes(tx_cum)),
                Style::default().fg(theme.text),
            ),
            Span::styled("   peak ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "↓ {}/s  ↑ {}/s",
                    format_bytes(rx_peak.max(0.0) as u64),
                    format_bytes(tx_peak.max(0.0) as u64)
                ),
                Style::default().fg(theme.text),
            ),
            if idle {
                Span::styled("   · idle", Style::default().fg(theme.muted))
            } else {
                Span::styled(
                    format!(
                        "   · {} interface{}",
                        nets.len(),
                        if nets.len() == 1 { "" } else { "s" }
                    ),
                    Style::default().fg(theme.muted),
                )
            },
        ]),
    ];
    frame.render_widget(Paragraph::new(summary).block(panel(theme, "network")), rows[0]);

    let rx_history = history.net_rx.as_slice();
    let tx_history = history.net_tx.as_slice();
    let max_rate = rx_history
        .iter()
        .chain(tx_history.iter())
        .copied()
        .fold(0.0f64, f64::max)
        .max(1.0)
        .max(rx_total.max(tx_total));
    let rx_norm = rx_history.iter().map(|v| (v / max_rate * 100.0) as u64).collect::<Vec<_>>();
    let tx_norm = tx_history.iter().map(|v| (v / max_rate * 100.0) as u64).collect::<Vec<_>>();
    let graph = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(rows[1]);
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&rx_norm, theme.good, theme.accent, 100))
            .max(100)
            .style(Style::default().fg(theme.good))
            .block(panel(
                theme,
                &format!(
                    "rx {}/s · peak {}/s{}",
                    format_bytes(rx_total.max(0.0) as u64),
                    format_bytes(rx_peak.max(0.0) as u64),
                    if idle { " · idle" } else { "" },
                ),
            )),
        graph[0],
    );
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&tx_norm, theme.accent, theme.good, 100))
            .max(100)
            .style(Style::default().fg(theme.accent))
            .block(panel(
                theme,
                &format!(
                    "tx {}/s · peak {}/s{}",
                    format_bytes(tx_total.max(0.0) as u64),
                    format_bytes(tx_peak.max(0.0) as u64),
                    if idle { " · idle" } else { "" },
                ),
            )),
        graph[1],
    );

    // Interface table — busiest first (already sorted), bounded to viewport
    // with an explicit "showing X/Y" title instead of silently clipping.
    let headers = ["interface", "rx", "tx", "rx/s", "tx/s", "pkt/s", "err"];
    let available = rows[2].height.saturating_sub(3) as usize;
    let table_rows = nets
        .iter()
        .take(available)
        .enumerate()
        .map(|(i, n)| {
            let zebra =
                if i % 2 == 1 { Style::default().bg(theme.surface_alt) } else { Style::default() };
            let pps = n.rx_pps.max(0.0) + n.tx_pps.max(0.0);
            Row::new([
                Cell::from(n.name.clone()),
                Cell::from(format_bytes(n.rx_bytes)),
                Cell::from(format_bytes(n.tx_bytes)),
                Cell::from(format!("{}/s", format_bytes(n.rx_rate.max(0.0) as u64))),
                Cell::from(format!("{}/s", format_bytes(n.tx_rate.max(0.0) as u64))),
                Cell::from(format!("{pps:.0}")),
                if n.err_total > 0 {
                    Cell::from(format!("{}", n.err_total)).style(Style::default().fg(theme.bad))
                } else {
                    Cell::from("—")
                },
            ])
            .style(zebra)
        })
        .collect::<Vec<_>>();
    let shown = table_rows.len();
    let widths = [
        Constraint::Length(16),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(8),
        Constraint::Length(7),
    ];
    let title = if shown < nets.len() {
        format!("interfaces · showing {shown}/{} · busiest first", nets.len())
    } else {
        format!("interfaces ({}) · busiest first", nets.len())
    };
    frame.render_widget(
        Table::new(table_rows, widths)
            .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
            .block(panel(theme, &title))
            .column_spacing(1),
        rows[2],
    );

    if nets.is_empty() {
        let empty = Line::from(Span::styled(
            "no interfaces — /proc/net/dev unavailable",
            Style::default().fg(theme.muted),
        ));
        let inner = Rect {
            x: rows[2].x + 1,
            y: rows[2].y + 2,
            width: rows[2].width.saturating_sub(2),
            height: 1,
        };
        if inner.height > 0 && inner.width > 0 {
            frame.render_widget(Paragraph::new(empty), inner);
        }
    }
}
