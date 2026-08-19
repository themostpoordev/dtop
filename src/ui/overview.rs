use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{gradient_line, panel, summary_cards, Theme};

pub(super) fn overview(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(12), Constraint::Min(6)])
        .split(area);
    summary_cards(frame, app, rows[0], theme);
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    // CPU panel: big % + scrolling sparkline + per-service bars (btop style).
    let cpu_history = app.data.history.as_slice_cpu();
    let cpu_latest = cpu_history.last().copied().unwrap_or(0);
    let cpu_top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Length(6), Constraint::Min(4)])
        .split(main[0]);
    let big = vec![
        Line::from(Span::styled(
            format!("{cpu_latest}%"),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("total cpu", Style::default().fg(theme.muted))),
    ];
    frame.render_widget(Paragraph::new(big).block(panel(theme, "CPU")), cpu_top[0]);
    frame.render_widget(
        Paragraph::new(gradient_line(&cpu_history, theme.good, theme.accent, 100)),
        cpu_top[1],
    );
    let mut cpu_lines = Vec::new();
    let mut top_cpu = app
        .data
        .containers
        .iter()
        .map(|c| (c.name.clone(), c.metrics.cpu_percent))
        .collect::<Vec<_>>();
    top_cpu.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (name, percent) in top_cpu.into_iter().take(8) {
        if percent <= 0.0 {
            continue;
        }
        let bar_len = ((percent / 100.0) * 20.0) as usize;
        cpu_lines.push(Line::from(vec![
            Span::styled(format!("{name:<16} "), Style::default().fg(theme.muted)),
            Span::styled("█".repeat(bar_len), Style::default().fg(theme.accent)),
            Span::styled(format!(" {percent:5.1}%"), Style::default().fg(theme.text)),
        ]));
    }
    if cpu_lines.is_empty() {
        cpu_lines.push(Line::from(Span::styled(
            "no active containers",
            Style::default().fg(theme.muted),
        )));
    }
    frame.render_widget(Paragraph::new(cpu_lines), cpu_top[2]);

    // Memory panel: list + all services by memory.
    let memory = &app.data.host_memory;
    let mut mem_lines = Vec::new();
    let ram_percent = if memory.ram_total > 0 {
        (memory.ram_used as f64 / memory.ram_total as f64) * 100.0
    } else {
        0.0
    };
    mem_lines.push(Line::from(vec![
        Span::styled("RAM       ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{}/{}", format_bytes(memory.ram_used), format_bytes(memory.ram_total)),
            Style::default().fg(theme.text),
        ),
        Span::styled(format!("  {ram_percent:.0}%"), Style::default().fg(theme.good)),
    ]));
    let mut add_mem_row = |label: &str, used: u64, total: u64, color: ratatui::style::Color| {
        let percent = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
        mem_lines.push(Line::from(vec![
            Span::styled(format!("{label:<9}"), Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}/{}", format_bytes(used), format_bytes(total)),
                Style::default().fg(theme.text),
            ),
            Span::styled(format!("  {percent:.0}%"), Style::default().fg(color)),
        ]));
    };
    add_mem_row("zram", memory.zram_used, memory.zram_total, theme.warn);
    add_mem_row("swapfile", memory.swapfile_used, memory.swapfile_total, theme.warn);
    if memory.zram_total == 0 {
        mem_lines
            .push(Line::from(Span::styled("zram: not present", Style::default().fg(theme.muted))));
    }
    if memory.swapfile_total == 0 {
        mem_lines.push(Line::from(Span::styled(
            "swapfile: not present",
            Style::default().fg(theme.muted),
        )));
    }
    let used_sum = memory.ram_used + memory.zram_used + memory.swapfile_used;
    let total_sum = memory.ram_total + memory.zram_total + memory.swapfile_total;
    let total_percent =
        if total_sum > 0 { (used_sum as f64 / total_sum as f64) * 100.0 } else { 0.0 };
    mem_lines.push(Line::from(""));
    mem_lines.push(Line::from(vec![
        Span::styled("total     ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{}/{}", format_bytes(used_sum), format_bytes(total_sum)),
            Style::default().fg(theme.text),
        ),
        Span::styled(format!("  {total_percent:.0}%"), Style::default().fg(theme.good)),
    ]));
    mem_lines.push(Line::from(""));
    mem_lines.push(Line::from(Span::styled("services", Style::default().fg(theme.muted))));
    let mut top_mem = app
        .data
        .containers
        .iter()
        .map(|c| (c.name.clone(), c.metrics.memory_bytes))
        .filter(|(_, bytes)| *bytes > 0)
        .collect::<Vec<_>>();
    top_mem.sort_by_key(|item| std::cmp::Reverse(item.1));
    // Keep the services list bounded to the panel height so it never overflows
    // on hosts with many running containers.
    let available = main[1].height.saturating_sub(12) as usize;
    let total_mem_services = top_mem.len();
    for (name, bytes) in top_mem.into_iter().take(available) {
        mem_lines.push(Line::from(vec![
            Span::styled(format!("{name:<16} "), Style::default().fg(theme.muted)),
            Span::styled(format_bytes(bytes), Style::default().fg(theme.text)),
        ]));
    }
    if total_mem_services > available {
        mem_lines.push(Line::from(Span::styled(
            format!("+{} more", total_mem_services - available),
            Style::default().fg(theme.warn),
        )));
    }
    frame.render_widget(Paragraph::new(mem_lines).block(panel(theme, "memory")), main[1]);

    let recent = app
        .data
        .events
        .items
        .iter()
        .rev()
        .take(8)
        .map(|event| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", event.time()), Style::default().fg(theme.muted)),
                Span::styled(
                    format!("{} {}", event.kind, event.action),
                    Style::default().fg(theme.text),
                ),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(recent).block(panel(theme, "recent events")), rows[2]);
}
