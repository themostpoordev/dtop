use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{bar, panel, Theme};

/// Host memory bars + the top RSS consumers from the sampled process list.
pub(super) fn memory(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Min(6)])
        .split(area);
    let memory = &app.data.host.memory;

    let mut lines = vec![];
    let mut add_row = |label: &str, used: u64, total: u64, color: ratatui::style::Color| {
        let pct = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
        lines.push(Line::from(vec![
            Span::styled(format!("{label:<9}"), Style::default().fg(theme.muted)),
            bar(pct, 20, color),
            Span::styled(
                format!(" {:>8} / {:<8} {:5.1}%", format_bytes(used), format_bytes(total), pct),
                Style::default().fg(theme.text),
            ),
        ]));
    };
    add_row("RAM", memory.ram_used, memory.ram_total, theme.good);
    add_row("zram", memory.zram_used, memory.zram_total, theme.warn);
    add_row("swapfile", memory.swapfile_used, memory.swapfile_total, theme.warn);
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("total     ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{} / {}", format_bytes(memory.used()), format_bytes(memory.total())),
            Style::default().fg(theme.text),
        ),
    ]));
    frame.render_widget(Paragraph::new(lines).block(panel(theme, "memory")), chunks[0]);

    // Top RSS consumers — the sampled list is already CPU-sorted, so re-sort by memory.
    let mut top = app.data.host.processes.clone();
    top.sort_by_key(|p| std::cmp::Reverse(p.rss_bytes));
    let available = chunks[1].height.saturating_sub(2) as usize;
    let mut proc_lines =
        vec![Line::from(Span::styled("top by memory", Style::default().fg(theme.muted)))];
    for p in top.into_iter().take(available) {
        proc_lines.push(Line::from(vec![
            Span::styled(format!("{:<7}", p.pid), Style::default().fg(theme.muted)),
            Span::styled(format!("{:<16} ", p.name), Style::default().fg(theme.text)),
            Span::styled(format_bytes(p.rss_bytes), Style::default().fg(theme.text)),
            Span::styled(format!("  {:.1}%", p.mem_percent), Style::default().fg(theme.warn)),
        ]));
    }
    frame.render_widget(Paragraph::new(proc_lines).block(panel(theme, "processes")), chunks[1]);
}
