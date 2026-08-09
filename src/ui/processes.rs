use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{panel, Theme};

/// Top-64 processes by CPU, sortable by column (`s` cycles cpu/mem/pid).
pub(super) fn processes(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(6)])
        .split(area);

    let filter = if app.filter_mode {
        format!("filter: {}_", app.filter)
    } else if app.filter.is_empty() {
        "filter: none · / to search · s sort".into()
    } else {
        format!("filter: {} · / edit", app.filter)
    };
    frame.render_widget(
        Paragraph::new(filter)
            .style(Style::default().fg(if app.filter_mode { theme.accent } else { theme.muted }))
            .block(panel(theme, "processes")),
        chunks[0],
    );

    let headers = ["pid", "name", "state", "cpu", "memory", "threads"];
    let mut rows = app
        .data
        .host
        .processes
        .iter()
        .filter(|p| app.filter.is_empty() || p.matches(&app.filter))
        .collect::<Vec<_>>();
    match app.config.sort {
        crate::config::SortOrder::Cpu => {
            rows.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
        }
        crate::config::SortOrder::Memory => {
            rows.sort_by_key(|p| std::cmp::Reverse(p.rss_bytes));
        }
        crate::config::SortOrder::Name => {
            rows.sort_by_key(|p| p.name.to_ascii_lowercase());
        }
        crate::config::SortOrder::Uptime | crate::config::SortOrder::Status => {}
    }
    let total = rows.len();
    let viewport = chunks[1].height.saturating_sub(2) as usize;
    let table_rows = rows
        .into_iter()
        .take(viewport)
        .map(|p| {
            Row::new([
                Cell::from(p.pid.to_string()),
                Cell::from(p.name.clone()),
                Cell::from(p.state.to_string()),
                Cell::from(format!("{:.1}%", p.cpu_percent)),
                Cell::from(format_bytes(p.rss_bytes)),
                Cell::from(p.threads.to_string()),
            ])
        })
        .collect::<Vec<_>>();
    let widths = [
        Constraint::Length(8),
        Constraint::Min(12),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(8),
    ];
    let title = format!("{total} processes · sort {}", app.config.sort.label());
    frame.render_widget(
        Table::new(table_rows, widths)
            .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
            .block(panel(theme, &title))
            .column_spacing(1),
        chunks[1],
    );
}
