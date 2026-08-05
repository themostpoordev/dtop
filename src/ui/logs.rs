use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::App;

use super::{panel, Theme};

pub(super) fn logs(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let all = app
        .data
        .logs
        .items
        .iter()
        .filter(|line| {
            app.log_filter.is_empty()
                || line.text.to_ascii_lowercase().contains(&app.log_filter.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    // Render only the visible viewport to keep large scrollbacks cheap.
    let viewport = area.height.saturating_sub(2) as usize;
    let total = all.len();
    let start = if app.log_follow {
        total.saturating_sub(viewport)
    } else {
        let scrolled = app.log_scroll.min(total as u16) as usize;
        total.saturating_sub(scrolled).saturating_sub(viewport)
    };
    let lines = all
        .iter()
        .skip(start)
        .take(viewport)
        .map(|line| {
            let color: Color = match line.stream {
                crate::model::LogStream::Stderr => theme.warn,
                _ => theme.text,
            };
            Line::from(Span::styled(line.text.trim_end_matches('\n'), Style::default().fg(color)))
        })
        .collect::<Vec<_>>();
    let title = if app.log_follow { "logs · following" } else { "logs · paused" };
    frame.render_widget(
        Paragraph::new(lines).block(panel(theme, title)).wrap(Wrap { trim: false }),
        area,
    );
}
