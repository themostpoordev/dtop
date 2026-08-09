use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{bar, panel, Theme};

/// All-mode home screen: host CPU + load + sparkline, memory bars, disk/net rates.
pub(super) fn system(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(8), Constraint::Min(6)])
        .split(area);

    // CPU summary + sparkline.
    let cpu = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);
    let host = &app.data.host;
    let cpu_history = app.data.host_history.cpu.as_slice_cpu();
    let cpu_latest = host.cpu_total as u64;
    let cpu_block = vec![
        Line::from(Span::styled(
            format!("{cpu_latest}%"),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "load {:.2} {:.2} {:.2} · {} cores",
                host.load_avg[0], host.load_avg[1], host.load_avg[2], host.num_cpus
            ),
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(cpu_block).block(panel(theme, "CPU")), cpu[0]);
    frame.render_widget(
        Sparkline::default().data(&cpu_history).max(100).style(Style::default().fg(theme.accent)),
        cpu[1],
    );

    // Memory summary.
    let memory = &host.memory;
    let mem_pct = if memory.total() > 0 {
        (memory.used() as f64 / memory.total() as f64) * 100.0
    } else {
        0.0
    };
    let mut mem_lines = vec![
        Line::from(vec![
            Span::styled(
                format!("RAM {}/{}", format_bytes(memory.ram_used), format_bytes(memory.ram_total)),
                Style::default().fg(theme.text),
            ),
            Span::styled(format!("  {:.0}%", mem_pct), Style::default().fg(theme.good)),
        ]),
        Line::from(""),
        Line::from(Span::styled("zram", Style::default().fg(theme.muted))),
    ];
    if memory.zram_total > 0 {
        let zram_pct = (memory.zram_used as f64 / memory.zram_total as f64) * 100.0;
        mem_lines.push(Line::from(vec![
            bar(zram_pct, 24, theme.warn),
            Span::styled(format!(" {:.0}%", zram_pct), Style::default().fg(theme.warn)),
        ]));
    } else {
        mem_lines.push(Line::from(Span::styled("not present", Style::default().fg(theme.muted))));
    }
    mem_lines.push(Line::from(Span::styled("swapfile", Style::default().fg(theme.muted))));
    if memory.swapfile_total > 0 {
        let swap_pct = (memory.swapfile_used as f64 / memory.swapfile_total as f64) * 100.0;
        mem_lines.push(Line::from(vec![
            bar(swap_pct, 24, theme.warn),
            Span::styled(format!(" {:.0}%", swap_pct), Style::default().fg(theme.warn)),
        ]));
    } else {
        mem_lines.push(Line::from(Span::styled("not present", Style::default().fg(theme.muted))));
    }
    frame.render_widget(Paragraph::new(mem_lines).block(panel(theme, "memory")), rows[1]);

    // Disk + network rate summary.
    let disk_read: f64 = host.disks.iter().map(|d| d.read_rate).sum();
    let disk_write: f64 = host.disks.iter().map(|d| d.write_rate).sum();
    let net_rx: f64 = host.nets.iter().map(|n| n.rx_rate).sum();
    let net_tx: f64 = host.nets.iter().map(|n| n.tx_rate).sum();
    let io_lines = vec![
        Line::from(vec![
            Span::styled("disk ↓ ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}/s", format_bytes(disk_read.max(0.0) as u64)),
                Style::default().fg(theme.text),
            ),
            Span::styled("  ↑ ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}/s", format_bytes(disk_write.max(0.0) as u64)),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("net  ↓ ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}/s", format_bytes(net_rx.max(0.0) as u64)),
                Style::default().fg(theme.text),
            ),
            Span::styled("  ↑ ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}/s", format_bytes(net_tx.max(0.0) as u64)),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Tab to CPU · Memory · Disk · Network · Processes",
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(io_lines).block(panel(theme, "io")), rows[2]);
}
