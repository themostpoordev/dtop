use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{bar, gradient_bars, panel, Theme};

/// All-mode home screen: host CPU + load + gradient, memory bars, disk/net rates.
pub(super) fn system(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(8), Constraint::Min(5)])
        .split(area);

    // CPU summary left, gradient history right — one dense row.
    let cpu = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
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
        Sparkline::default()
            .data(gradient_bars(&cpu_history, theme.good, theme.accent, 100))
            .max(100)
            .style(Style::default().fg(theme.accent))
            .block(panel(theme, "60 s")),
        cpu[1],
    );

    // Memory summary — every row gets a bar + numbers + percent, uniform.
    let memory = &host.memory;
    let mut mem_lines = Vec::new();
    let mut add_mem_row = |label: &str, used: u64, total: u64, color: ratatui::style::Color| {
        let pct = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
        mem_lines.push(Line::from(vec![
            Span::styled(format!("{label:<9}"), Style::default().fg(theme.muted)),
            bar(pct, 20, color),
            Span::styled(
                format!(" {:>8} / {:<8} {:5.1}%", format_bytes(used), format_bytes(total), pct),
                Style::default().fg(theme.text),
            ),
        ]));
    };
    add_mem_row("RAM", memory.ram_used, memory.ram_total, theme.good);
    add_mem_row("zram", memory.zram_used, memory.zram_total, theme.warn);
    add_mem_row("swapfile", memory.swapfile_used, memory.swapfile_total, theme.warn);
    let mem_pct = if memory.total() > 0 {
        (memory.used() as f64 / memory.total() as f64) * 100.0
    } else {
        0.0
    };
    mem_lines.push(Line::from(""));
    mem_lines.push(Line::from(vec![
        Span::styled("total     ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{} / {}", format_bytes(memory.used()), format_bytes(memory.total())),
            Style::default().fg(theme.text),
        ),
        Span::styled(format!("  {mem_pct:.0}%"), Style::default().fg(theme.good)),
    ]));
    frame.render_widget(Paragraph::new(mem_lines).block(panel(theme, "memory")), rows[1]);

    // Disk + network rate summary — compact two lines.
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
            Span::styled("   net ↓ ", Style::default().fg(theme.muted)),
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
        Line::from(Span::styled(
            "Tab to CPU · Memory · Disk · Network · Processes",
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(io_lines).block(panel(theme, "io")), rows[2]);
}
