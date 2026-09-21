use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Sparkline, Table},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{gradient_bars, panel, Theme};

/// Disk screen — readable at a glance, useful when idle.
///
/// - Header: live rates + IOPS + utilization, cumulative totals, and peaks,
///   so an idle disk still shows *information* instead of a blank panel.
/// - Two gradient histories whose titles carry current rate + peak.
/// - Per-device table (busiest first): totals, rates, IOPS, utilization.
/// - Filesystem table: where each block device is actually mounted.
pub(super) fn disk(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Min(6),
            Constraint::Min(5),
        ])
        .split(area);

    let host = &app.data.host;
    let history = &app.data.host_history;
    let read_total: f64 = host.disks.iter().map(|d| d.read_rate).sum();
    let write_total: f64 = host.disks.iter().map(|d| d.write_rate).sum();
    let read_iops: f64 = host.disks.iter().map(|d| d.read_iops).sum();
    let write_iops: f64 = host.disks.iter().map(|d| d.write_iops).sum();
    let read_cum: u64 = host.disks.iter().map(|d| d.read_bytes).sum();
    let write_cum: u64 = host.disks.iter().map(|d| d.write_bytes).sum();
    let read_peak = history.disk_read.peak();
    let write_peak = history.disk_write.peak();
    let util_max = host.disks.iter().map(|d| d.util).fold(0.0f64, f64::max);
    let idle = read_total < 1.0 && write_total < 1.0 && read_peak < 1.0 && write_peak < 1.0;

    // Summary — two dense lines. Numbers first (scannable), labels muted.
    let summary = vec![
        Line::from(vec![
            Span::styled("read  ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{:>10}/s", format_bytes(read_total.max(0.0) as u64)),
                Style::default().fg(theme.good),
            ),
            Span::styled("   write ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{:>10}/s", format_bytes(write_total.max(0.0) as u64)),
                Style::default().fg(theme.accent),
            ),
            Span::styled("   iops ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{read_iops:.0}/{write_iops:.0}"),
                Style::default().fg(theme.text),
            ),
            Span::styled("   util ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{util_max:.0}%"),
                Style::default().fg(if util_max >= 80.0 { theme.warn } else { theme.text }),
            ),
        ]),
        Line::from(vec![
            Span::styled("total ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("↓ {}  ↑ {}", format_bytes(read_cum), format_bytes(write_cum)),
                Style::default().fg(theme.text),
            ),
            Span::styled("   peak ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "↓ {}/s  ↑ {}/s",
                    format_bytes(read_peak.max(0.0) as u64),
                    format_bytes(write_peak.max(0.0) as u64)
                ),
                Style::default().fg(theme.text),
            ),
            if idle {
                Span::styled("   · idle", Style::default().fg(theme.muted))
            } else {
                Span::styled(
                    format!("   · {} disk{}", host.disks.len(), plural(host.disks.len())),
                    Style::default().fg(theme.muted),
                )
            },
        ]),
    ];
    frame.render_widget(Paragraph::new(summary).block(panel(theme, "disk")), rows[0]);

    // Histories — titles repeat current + peak so a flat (idle) graph
    // still tells you the totals instead of looking broken.
    render_history(
        frame,
        rows[1],
        theme,
        history.disk_read.as_slice(),
        history.disk_write.as_slice(),
        &format!(
            "read {}/s · peak {}/s{}",
            format_bytes(read_total.max(0.0) as u64),
            format_bytes(read_peak.max(0.0) as u64),
            if idle { " · idle" } else { "" },
        ),
        &format!(
            "write {}/s · peak {}/s{}",
            format_bytes(write_total.max(0.0) as u64),
            format_bytes(write_peak.max(0.0) as u64),
            if idle { " · idle" } else { "" },
        ),
    );

    render_disk_table(frame, app, rows[2], theme);
    render_fs_table(frame, app, rows[3], theme);
}

fn render_history(
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
    read: Vec<f64>,
    write: Vec<f64>,
    read_title: &str,
    write_title: &str,
) {
    let max_rate = read.iter().chain(write.iter()).copied().fold(0.0f64, f64::max).max(1.0);
    let read_norm = read.iter().map(|v| (v / max_rate * 100.0) as u64).collect::<Vec<_>>();
    let write_norm = write.iter().map(|v| (v / max_rate * 100.0) as u64).collect::<Vec<_>>();
    let graph = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(area);
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&read_norm, theme.good, theme.accent, 100))
            .max(100)
            .style(Style::default().fg(theme.good))
            .block(panel(theme, read_title)),
        graph[0],
    );
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&write_norm, theme.accent, theme.good, 100))
            .max(100)
            .style(Style::default().fg(theme.accent))
            .block(panel(theme, write_title)),
        graph[1],
    );
}

fn render_disk_table(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let disks = &app.data.host.disks;
    let available = area.height.saturating_sub(3) as usize; // borders + header
    let headers = ["device", "read", "write", "read/s", "write/s", "iops r/w", "util"];
    let table_rows = disks
        .iter()
        .take(available)
        .enumerate()
        .map(|(i, d)| {
            let zebra =
                if i % 2 == 1 { Style::default().bg(theme.surface_alt) } else { Style::default() };
            Row::new([
                Cell::from(d.name.clone()),
                Cell::from(format_bytes(d.read_bytes)),
                Cell::from(format_bytes(d.write_bytes)),
                Cell::from(format!("{}/s", format_bytes(d.read_rate.max(0.0) as u64))),
                Cell::from(format!("{}/s", format_bytes(d.write_rate.max(0.0) as u64))),
                Cell::from(format!("{:.0}/{:.0}", d.read_iops.max(0.0), d.write_iops.max(0.0))),
                Cell::from(format!("{:.0}%", d.util)).style(
                    Style::default().fg(if d.util >= 80.0 { theme.warn } else { theme.text }),
                ),
            ])
            .style(zebra)
        })
        .collect::<Vec<_>>();
    let widths = [
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(10),
        Constraint::Length(6),
    ];
    let shown = table_rows.len();
    let title = if shown < disks.len() {
        format!("disks · showing {shown}/{} · busiest first", disks.len())
    } else {
        format!("disks ({}) · busiest first", disks.len())
    };
    frame.render_widget(
        Table::new(table_rows, widths)
            .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
            .block(panel(theme, &title))
            .column_spacing(1),
        area,
    );

    if disks.is_empty() {
        let empty = Line::from(Span::styled(
            "no disk counters — /proc/diskstats unavailable",
            Style::default().fg(theme.muted),
        ));
        let inner =
            Rect { x: area.x + 1, y: area.y + 2, width: area.width.saturating_sub(2), height: 1 };
        if inner.height > 0 && inner.width > 0 {
            frame.render_widget(Paragraph::new(empty), inner);
        }
    }
}

fn render_fs_table(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let mounts = &app.data.host.mounts;
    let available = area.height.saturating_sub(3) as usize;
    let headers = ["mount", "fstype", "device", "size"];
    let table_rows = mounts
        .iter()
        .take(available)
        .enumerate()
        .map(|(i, m)| {
            let zebra =
                if i % 2 == 1 { Style::default().bg(theme.surface_alt) } else { Style::default() };
            Row::new([
                Cell::from(m.mountpoint.clone()),
                Cell::from(m.fstype.clone()),
                Cell::from(short_device(&m.device)),
                Cell::from(if m.size_bytes > 0 {
                    format_bytes(m.size_bytes)
                } else {
                    "—".to_owned()
                }),
            ])
            .style(zebra)
        })
        .collect::<Vec<_>>();
    let widths = [
        Constraint::Min(12),
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Length(10),
    ];
    frame.render_widget(
        Table::new(table_rows, widths)
            .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
            .block(panel(theme, &format!("filesystems ({})", mounts.len())))
            .column_spacing(1),
        area,
    );

    if mounts.is_empty() {
        let empty = Line::from(Span::styled(
            "no block filesystems mounted — waiting for the slow sample",
            Style::default().fg(theme.muted),
        ));
        let inner =
            Rect { x: area.x + 1, y: area.y + 2, width: area.width.saturating_sub(2), height: 1 };
        if inner.height > 0 && inner.width > 0 {
            frame.render_widget(Paragraph::new(empty), inner);
        }
    }
}

/// `/dev/sda3` → `sda3`: the prefix carries no information in a table.
fn short_device(device: &str) -> String {
    device.rsplit('/').next().unwrap_or(device).to_owned()
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
