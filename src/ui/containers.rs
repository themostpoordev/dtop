use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{empty, panel, Theme};

pub(super) fn containers(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(6)])
        .split(area);
    let filter = if app.filter_mode {
        format!("filter: {}_", app.filter)
    } else if app.filter.is_empty() {
        "filter: none · / to search".into()
    } else {
        format!("filter: {} · / edit", app.filter)
    };
    frame.render_widget(
        Paragraph::new(filter)
            .style(Style::default().fg(if app.filter_mode { theme.accent } else { theme.muted }))
            .block(panel(theme, "containers")),
        chunks[0],
    );
    let headers = ["name", "state", "health", "cpu", "memory", "network", "uptime", "restarts"];
    let visible = app.visible_indices();
    let total = visible.len();
    // Viewport height minus filter bar and table header.
    let viewport = chunks[1].height.saturating_sub(2) as usize;
    let start = app.container_scroll.min(total.saturating_sub(viewport));
    let end = (start + viewport).min(total);
    let rows = visible[start..end]
        .iter()
        .map(|index| {
            let c = &app.data.containers[*index];
            let selected = *index == app.selected_container;
            let style = if selected {
                Style::default().bg(theme.selected).fg(theme.text)
            } else {
                Style::default().fg(theme.text)
            };
            Row::new(
                [
                    c.name.clone(),
                    c.state.clone(),
                    c.health.clone(),
                    format!("{:.1}%", c.metrics.cpu_percent),
                    format!(
                        "{} / {}",
                        format_bytes(c.metrics.memory_bytes),
                        format_bytes(c.metrics.memory_limit)
                    ),
                    format!(
                        "↓{} ↑{}",
                        super::format_rate(c.delta.network_rx_rate),
                        super::format_rate(c.delta.network_tx_rate)
                    ),
                    c.uptime(),
                    c.restart_count.to_string(),
                ]
                .into_iter()
                .map(Cell::from),
            )
            .style(style)
        })
        .collect::<Vec<_>>();
    let widths = [
        Constraint::Percentage(18),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(9),
        Constraint::Length(22),
        Constraint::Length(26),
        Constraint::Length(10),
        Constraint::Length(9),
    ];
    let hidden = total.saturating_sub(end);
    let mut table_title = format!("{total} containers · sort {}", app.config.sort.label());
    if start > 0 || hidden > 0 {
        table_title.push_str(&format!(" · showing {}-{}", start + 1, end));
    }
    let table = Table::new(rows, widths)
        .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
        .block(panel(theme, &table_title))
        .column_spacing(1);
    frame.render_widget(table, chunks[1]);
    if total == 0 {
        empty(frame, chunks[1], "No containers available");
    }
}
