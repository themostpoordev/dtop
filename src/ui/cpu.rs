use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::app::App;

use super::{bar, gradient_bars, panel, Theme};

/// Per-core utilization bars, gradient CPU history, and top CPU consumers.
pub(super) fn cpu(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Length(5), Constraint::Min(4)])
        .split(area);
    // History panel: title lines on top, gradient line fills the rest inside one border.
    let history_top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(rows[0]);

    let history = app.data.host_history.cpu.as_slice_cpu();
    let latest = app.data.host.cpu_total;
    let head = vec![
        Line::from(Span::styled(
            format!("{latest:.0}% total"),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "load {:.2} {:.2} {:.2}",
                app.data.host.load_avg[0], app.data.host.load_avg[1], app.data.host.load_avg[2]
            ),
            Style::default().fg(theme.muted),
        )),
    ];
    frame.render_widget(Paragraph::new(head).block(panel(theme, "CPU history")), history_top[0]);
    frame.render_widget(
        Sparkline::default()
            .data(gradient_bars(&history, theme.good, theme.accent, 100))
            .max(100)
            .style(Style::default().fg(theme.accent)),
        history_top[1],
    );

    // Per-core bars — only those that fit.
    let mut core_lines = Vec::new();
    let cores = &app.data.host.cores;
    let available = rows[1].height.saturating_sub(2) as usize;
    for (index, percent) in cores.iter().enumerate().take(available) {
        core_lines.push(Line::from(vec![
            Span::styled(format!("cpu{index:<3} "), Style::default().fg(theme.muted)),
            bar(*percent, 30, theme.accent),
            Span::styled(format!(" {percent:5.1}%"), Style::default().fg(theme.text)),
        ]));
    }
    if core_lines.is_empty() {
        core_lines.push(Line::from(Span::styled(
            "no cpu data yet — waiting for the first sample",
            Style::default().fg(theme.muted),
        )));
    }
    frame.render_widget(Paragraph::new(core_lines).block(panel(theme, "per-core")), rows[1]);

    // Top CPU consumers — same pattern as the memory screen's top-by-memory.
    let mut top = app.data.host.processes.clone();
    top.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
    let proc_available = rows[2].height.saturating_sub(2) as usize;
    let mut proc_lines =
        vec![Line::from(Span::styled("top by cpu", Style::default().fg(theme.muted)))];
    for p in top.into_iter().take(proc_available) {
        proc_lines.push(Line::from(vec![
            Span::styled(format!("{:<7}", p.pid), Style::default().fg(theme.muted)),
            Span::styled(format!("{:<16} ", p.name), Style::default().fg(theme.text)),
            Span::styled(format!("{:.1}%", p.cpu_percent), Style::default().fg(theme.accent)),
        ]));
    }
    frame.render_widget(Paragraph::new(proc_lines).block(panel(theme, "processes")), rows[2]);
}
