use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::app::App;

use super::{bar, panel, Theme};

/// Per-core utilization bars + a total-CPU sparkline.
pub(super) fn cpu(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(8)])
        .split(area);
    // History panel: title lines on top, sparkline fills the rest inside one border.
    let history_top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
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
        Sparkline::default().data(&history).max(100).style(Style::default().fg(theme.accent)),
        history_top[1],
    );

    // Render only the cores that fit the viewport.
    let mut lines = Vec::new();
    let cores = &app.data.host.cores;
    let available = rows[1].height.saturating_sub(2) as usize;
    for (index, percent) in cores.iter().enumerate().take(available) {
        lines.push(Line::from(vec![
            Span::styled(format!("cpu{index:<3} "), Style::default().fg(theme.muted)),
            bar(*percent, 30, theme.accent),
            Span::styled(format!(" {percent:5.1}%"), Style::default().fg(theme.text)),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "no cpu data yet — waiting for the first sample",
            Style::default().fg(theme.muted),
        )));
    }
    frame.render_widget(Paragraph::new(lines).block(panel(theme, "per-core")), rows[1]);
}
