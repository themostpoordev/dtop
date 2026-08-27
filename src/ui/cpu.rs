use std::fmt::Write as _;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::{
    app::App,
    model::{History, HostStats},
};

use super::{bar, gradient_bars, panel, Theme};

/// CPU screen — three stacked panels, each with its own bordered block.
///
/// Layout (top → bottom):
///
/// - **History** — title + load average on the border, full-width gradient
///   sparkline inside. One block, no nested sub-layout, so the last bar
///   always reaches the right edge of the panel — even on narrow terminals.
/// - **Per-core** — one bar per logical core, sized to fit every core
///   (plus a 5-row minimum so the panel stays usable on tiny terminals).
/// - **Processes** — top-N consumers by CPU, partial-sorted so we never
///   pay for a full sort when only N items will render.
pub(super) fn cpu(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let cores_len = app.data.host.cores.len();
    let per_core_height = (cores_len as u16 + 2).max(5);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),               // history (borders + sparkline)
            Constraint::Length(per_core_height), // per-core
            Constraint::Min(4),                  // processes
        ])
        .split(area);

    render_history(frame, rows[0], &app.data.host, &app.data.host_history.cpu, theme);
    render_per_core(frame, rows[1], &app.data.host.cores, theme);
    render_processes(frame, rows[2], &app.data.host.processes, theme);
}

/// CPU history — title is rendered on the block border so the sparkline
/// can use every cell of the inner area. Avoids the nested sub-layout
/// pattern that used to clip the rightmost bar on narrow terminals.
fn render_history(
    frame: &mut Frame,
    area: Rect,
    host: &HostStats,
    history: &History,
    theme: Theme,
) {
    // Build the title once. Reusing a String across frames avoids the
    // format!/drop churn that was happening for every tick.
    let mut title = String::with_capacity(64);
    let _ = write!(
        title,
        " CPU history · {:.0}% total · load {:.2} {:.2} {:.2} ",
        host.cpu_total, host.load_avg[0], host.load_avg[1], host.load_avg[2]
    );

    let data = gradient_bars(&history.as_slice_cpu(), theme.good, theme.accent, 100);

    frame.render_widget(
        Sparkline::default().data(data).max(100).style(Style::default().fg(theme.accent)).block(
            panel(theme, "").title(Line::styled(
                title,
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            )),
        ),
        area,
    );
}

/// Per-core utilization bars — one row per logical core, panel sized to
/// fit them all. Renders nothing on the first tick before samples arrive.
fn render_per_core(frame: &mut Frame, area: Rect, cores: &[f64], theme: Theme) {
    let mut lines: Vec<Line> = Vec::with_capacity(cores.len());
    for (index, percent) in cores.iter().enumerate() {
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

    frame.render_widget(Paragraph::new(lines).block(panel(theme, "per-core")), area);
}

/// Top CPU consumers — partial-sorted to keep this O(n) instead of O(n log n)
/// when only the first few rows will ever be visible.
fn render_processes(
    frame: &mut Frame,
    area: Rect,
    processes: &[crate::model::ProcessRow],
    theme: Theme,
) {
    let inner = area.height.saturating_sub(2) as usize;
    if inner == 0 {
        return;
    }

    // Reserve one row for the "top by cpu" header, then the rest for rows.
    let take = inner.saturating_sub(1);

    // Partial sort: rearrange in place so the top `take` items land at
    // the front in descending CPU order, without paying for a full sort.
    // We work on a scratch slice of references to avoid disturbing the
    // caller's ordering (which the processes screen also relies on).
    let mut scratch: Vec<&crate::model::ProcessRow> = processes.iter().collect();
    if take < scratch.len() {
        scratch.select_nth_unstable_by(take, |a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
        scratch[..take].sort_unstable_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
    } else {
        scratch.sort_unstable_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
    }

    let mut lines: Vec<Line> = Vec::with_capacity(take + 1);
    lines.push(Line::from(Span::styled("top by cpu", Style::default().fg(theme.muted))));
    for p in scratch.into_iter().take(take) {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<7}", p.pid), Style::default().fg(theme.muted)),
            Span::styled(format!("{:<16} ", p.name), Style::default().fg(theme.text)),
            Span::styled(format!("{:.1}%", p.cpu_percent), Style::default().fg(theme.accent)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).block(panel(theme, "processes")), area);
}
