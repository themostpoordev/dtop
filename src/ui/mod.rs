mod theme;

pub use theme::Theme;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{BarChart, Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};

use crate::{
    app::App,
    model::{format_bytes, format_rate, ConnectionState, Screen},
};

pub fn render(frame: &mut Frame, app: &App) {
    let theme = Theme::from_name(app.config.theme);
    frame
        .render_widget(Block::default().style(Style::default().bg(theme.background)), frame.area());
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(2)])
        .split(frame.area());
    header(frame, app, outer[0], theme);
    match app.screen {
        Screen::Home => home(frame, app, outer[1], theme),
        Screen::Overview => overview(frame, app, outer[1], theme),
        Screen::Containers => containers(frame, app, outer[1], theme),
        Screen::Details => details(frame, app, outer[1], theme),
        Screen::Logs => logs(frame, app, outer[1], theme),
        Screen::Events => events(frame, app, outer[1], theme),
        Screen::Images => {
            resources(frame, app, outer[1], theme, "Images", resource_rows(app, "images"))
        }
        Screen::Volumes => {
            resources(frame, app, outer[1], theme, "Volumes", resource_rows(app, "volumes"))
        }
        Screen::Networks => {
            resources(frame, app, outer[1], theme, "Networks", resource_rows(app, "networks"))
        }
        Screen::Settings => settings(frame, app, outer[1], theme),
    }
    footer(frame, app, outer[2], theme);
    if app.confirm.is_some() {
        confirmation(frame, app, theme);
    }
}

fn header(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(20), Constraint::Length(30)])
        .split(area);
    let logo = Paragraph::new(Line::from(vec![
        Span::styled(" ◈ dtop", theme.title()),
        Span::styled(" 0.1", Style::default().fg(theme.muted)),
    ]))
    .block(
        Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(theme.border)),
    );
    frame.render_widget(logo, chunks[0]);
    let tabs = Tabs::new(
        Screen::PRIMARY.iter().map(|screen| Line::from(screen.label())).collect::<Vec<_>>(),
    )
    .select(Screen::PRIMARY.iter().position(|screen| *screen == app.screen).unwrap_or(0))
    .style(Style::default().fg(theme.muted))
    .highlight_style(theme.title().bg(theme.surface_alt))
    .divider(" · ");
    frame.render_widget(
        tabs.block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        ),
        chunks[1],
    );
    let connection = match app.connection {
        ConnectionState::Connected => {
            Span::styled("● Docker connected", Style::default().fg(theme.good))
        }
        ConnectionState::Connecting => {
            Span::styled("◌ Connecting", Style::default().fg(theme.warn))
        }
        ConnectionState::PermissionDenied => {
            Span::styled("× Permission denied", Style::default().fg(theme.bad))
        }
        ConnectionState::Unavailable => {
            Span::styled("× Docker unavailable", Style::default().fg(theme.bad))
        }
        ConnectionState::Error => Span::styled("× Docker error", Style::default().fg(theme.bad)),
    };
    frame.render_widget(
        Paragraph::new(Line::from(connection)).alignment(ratatui::layout::Alignment::Right).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        ),
        chunks[2],
    );
}

fn footer(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let mut hints = if app.config.show_hints {
        "Tab sections  ·  Esc home  ·  ? help  ·  q quit".to_owned()
    } else {
        "q quit".to_owned()
    };
    if let Some(notice) = &app.notice {
        hints.push_str("   ");
        hints.push_str(notice);
    }
    frame.render_widget(
        Paragraph::new(hints).style(Style::default().fg(theme.muted)).block(
            Block::default().borders(Borders::TOP).border_style(Style::default().fg(theme.border)),
        ),
        area,
    );
}
fn home(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(6)])
        .split(chunks[0]);
    let logo = Paragraph::new(Text::from(vec![
        Line::from(Span::styled("    ██████╗ ████████╗ ██████╗ ██████╗", theme.title())),
        Line::from(Span::styled("    ██╔══██╗╚══██╔══╝██╔═══██╗██╔══██╗", theme.title())),
        Line::from(Span::styled("    ██║  ██║   ██║   ██║   ██║██████╔╝", theme.title())),
        Line::from(Span::styled("    ██║  ██║   ██║   ██║   ██║██╔═══╝ ", theme.title())),
        Line::from(Span::styled("    ╚█████╔╝   ██║   ╚██████╔╝██║     ", theme.title())),
        Line::from(Span::styled("     ╚════╝    ╚═╝    ╚═════╝ ╚═╝     ", theme.title())),
    ]))
    .block(panel(theme, "local Docker monitor"));
    frame.render_widget(logo, left[0]);
    let items = ["Overview", "Containers", "Events", "Images", "Volumes", "Networks", "Settings"];
    let selected = app.home_selection.min(items.len() - 1);
    let list = List::new(
        items
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if i == selected { "› " } else { "  " },
                        if i == selected {
                            theme.title()
                        } else {
                            Style::default().fg(theme.muted)
                        },
                    ),
                    Span::raw(item),
                ]))
                .style(if i == selected {
                    Style::default().bg(theme.selected)
                } else {
                    Style::default()
                })
            })
            .collect::<Vec<_>>(),
    )
    .block(panel(theme, "navigate · ↑↓ move · Enter open"));
    frame.render_widget(list, left[1]);
    let summary = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(8)])
        .split(chunks[1]);
    summary_cards(frame, app, summary[0], theme);
    let help = List::new(
        [
            "Tab  switch section",
            "Esc   return home",
            "Enter open selection",
            "?     full help",
            "q     quit",
        ]
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>(),
    )
    .block(panel(theme, "keybindings"))
    .style(Style::default().fg(theme.text));
    frame.render_widget(help, summary[1]);
}
fn summary_cards(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);
    let counts = app.counts();
    for (index, (label, value, color)) in [
        ("running", counts.0, theme.good),
        ("stopped", counts.1, theme.muted),
        ("paused", counts.2, theme.warn),
    ]
    .into_iter()
    .enumerate()
    {
        let text = vec![
            Line::from(Span::styled(
                value.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(label, Style::default().fg(theme.muted))),
        ];
        frame.render_widget(
            Paragraph::new(text)
                .alignment(ratatui::layout::Alignment::Center)
                .block(panel(theme, "")),
            chunks[index],
        );
    }
}
fn overview(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(8)])
        .split(area);
    summary_cards(frame, app, rows[0], theme);
    let lower = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(rows[1]);
    let bars = app
        .visible_indices()
        .iter()
        .take(8)
        .map(|i| {
            let c = &app.data.containers[*i];
            (c.name.as_str(), c.metrics.cpu_percent.max(0.0) as u64)
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        BarChart::default()
            .block(panel(theme, "CPU by container · %"))
            .data(&bars)
            .bar_width(7)
            .bar_gap(2)
            .max(100)
            .bar_style(Style::default().fg(theme.accent))
            .value_style(Style::default().fg(theme.text)),
        lower[0],
    );
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
    frame.render_widget(List::new(recent).block(panel(theme, "recent events")), lower[1]);
}
fn containers(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
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
    let rows = app
        .visible_indices()
        .into_iter()
        .map(|index| {
            let c = &app.data.containers[index];
            let selected = index == app.selected_container;
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
                        format_rate(c.delta.network_rx_rate),
                        format_rate(c.delta.network_tx_rate)
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
    let table_title =
        format!("{} containers · sort {}", app.visible_indices().len(), app.config.sort.label());
    let table = Table::new(rows, widths)
        .header(Row::new(headers).style(theme.title().bg(theme.surface_alt)))
        .block(panel(theme, &table_title))
        .column_spacing(1);
    frame.render_widget(table, chunks[1]);
    if app.data.containers.is_empty() {
        empty(frame, chunks[1], "No containers available");
    }
}
fn details(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let Some(details) = &app.data.details else {
        empty(frame, area, "Select a container and press Enter");
        return;
    };
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area);
    let left = [
        format!("name       {}", details.name),
        format!("id         {}", details.id),
        format!("image      {}", details.image),
        format!("status     {}", details.status),
        format!("health     {}", details.health),
        format!("restarts   {}", details.restart_count),
        format!("created    {}", details.created),
        format!("started    {}", details.started),
    ];
    frame.render_widget(
        Paragraph::new(left.join("\n"))
            .block(panel(theme, "container details"))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );
    let ports = if details.ports.is_empty() { "none".to_owned() } else { details.ports.join("\n") };
    let mounts =
        if details.mounts.is_empty() { "none".to_owned() } else { details.mounts.join("\n") };
    let networks =
        if details.networks.is_empty() { "none".to_owned() } else { details.networks.join("\n") };
    let right = [
        format!("command\n{}", details.command),
        format!("ports\n{ports}"),
        format!("mounts\n{mounts}"),
        format!("networks\n{networks}"),
    ];
    frame.render_widget(
        Paragraph::new(right.join("\n\n"))
            .block(panel(theme, "runtime metadata"))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );
}
fn logs(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let lines = app
        .data
        .logs
        .items
        .iter()
        .filter(|line| {
            app.log_filter.is_empty()
                || line.text.to_ascii_lowercase().contains(&app.log_filter.to_ascii_lowercase())
        })
        .map(|line| {
            let color = match line.stream {
                crate::model::LogStream::Stderr => theme.warn,
                _ => theme.text,
            };
            Line::from(Span::styled(line.text.trim_end_matches('\n'), Style::default().fg(color)))
        })
        .collect::<Vec<_>>();
    let title = if app.log_follow { "logs · following" } else { "logs · paused" };
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(theme, title))
            .scroll((app.log_scroll, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
}
fn events(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let rows = app
        .data
        .events
        .items
        .iter()
        .rev()
        .filter(|event| {
            app.event_filter.is_empty()
                || format!("{} {} {}", event.kind, event.action, event.actor)
                    .to_ascii_lowercase()
                    .contains(&app.event_filter.to_ascii_lowercase())
        })
        .map(|event| {
            Row::new([
                event.time(),
                event.kind.clone(),
                event.action.clone(),
                event.actor.chars().take(16).collect(),
                event.attributes.clone(),
            ])
        })
        .collect::<Vec<_>>();
    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(16),
            Constraint::Length(18),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(["when", "type", "action", "actor", "attributes"])
            .style(theme.title().bg(theme.surface_alt)),
    )
    .block(panel(theme, "Docker events"));
    frame.render_widget(table, area);
}
fn resources(
    frame: &mut Frame,
    _app: &App,
    area: Rect,
    theme: Theme,
    title: &str,
    rows: Vec<Row<'static>>,
) {
    if rows.is_empty() {
        empty(
            frame,
            area,
            &format!("No {title_lower} available", title_lower = title.to_ascii_lowercase()),
        );
        return;
    }
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
        ],
    )
    .header(
        Row::new(["name", "detail", "size", "scope"]).style(theme.title().bg(theme.surface_alt)),
    )
    .block(panel(theme, title));
    frame.render_widget(table, area);
}
fn resource_rows(app: &App, kind: &str) -> Vec<Row<'static>> {
    match kind {
        "images" => app
            .data
            .images
            .iter()
            .map(|image| {
                Row::new([
                    image
                        .tags
                        .first()
                        .cloned()
                        .unwrap_or_else(|| image.id.chars().take(16).collect()),
                    image.id.chars().take(24).collect(),
                    format_bytes(image.size_bytes),
                    image.created.to_string(),
                ])
            })
            .collect(),
        "volumes" => app
            .data
            .volumes
            .iter()
            .map(|volume| {
                Row::new([
                    volume.name.clone(),
                    volume.driver.clone(),
                    volume.mountpoint.clone(),
                    volume.scope.clone(),
                ])
            })
            .collect(),
        _ => app
            .data
            .networks
            .iter()
            .map(|network| {
                Row::new([
                    network.name.clone(),
                    network.driver.clone(),
                    network.containers.to_string(),
                    network.scope.clone(),
                ])
            })
            .collect(),
    }
}
fn settings(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let entries = [
        ("Theme", app.config.theme.label().to_owned()),
        ("Refresh interval", format!("{} ms", app.config.refresh_ms)),
        ("Default sort", app.config.sort.label().to_owned()),
        ("Show stopped", yes_no(app.config.show_stopped)),
        ("Follow logs", yes_no(app.config.follow_logs)),
        ("Density", app.config.density.label().to_owned()),
        ("Keybinding hints", yes_no(app.config.show_hints)),
        ("Save settings", "Enter".into()),
        ("Reset defaults", "Enter".into()),
    ];
    let rows = entries
        .into_iter()
        .enumerate()
        .map(|(index, (label, value))| {
            let style = if index == app.settings_selection {
                Style::default().bg(theme.selected).fg(theme.text)
            } else {
                Style::default().fg(theme.text)
            };
            Row::new([
                if index == app.settings_selection {
                    format!("› {label}")
                } else {
                    format!("  {label}")
                },
                value,
            ])
            .style(style)
        })
        .collect::<Vec<_>>();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);
    frame.render_widget(
        Table::new(rows, [Constraint::Percentage(65), Constraint::Percentage(35)])
            .block(panel(theme, "settings · ←→ change · Enter save")),
        chunks[0],
    );
    let copy = vec![
        Line::from(Span::styled("local-only by design", theme.title())),
        Line::from("Docker requests use only the configured Unix socket."),
        Line::from("No telemetry, registry calls, or environment values."),
        Line::from(""),
        Line::from(Span::styled("minimum refresh", Style::default().fg(theme.muted))),
        Line::from("50 ms"),
    ];
    frame.render_widget(
        Paragraph::new(copy).block(panel(theme, "notes")).wrap(Wrap { trim: true }),
        chunks[1],
    );
}
fn confirmation(frame: &mut Frame, app: &App, theme: Theme) {
    let area = centered_rect(60, 30, frame.area());
    let action = app
        .confirm
        .as_ref()
        .map(|(action, name)| format!("{} {}", action.label(), name))
        .unwrap_or_default();
    let text = vec![
        Line::from(Span::styled("Confirmation required", theme.title())),
        Line::from(""),
        Line::from(action),
        Line::from(""),
        Line::from(Span::styled("Enter confirm   Esc cancel", Style::default().fg(theme.muted))),
    ];
    frame.render_widget(
        Paragraph::new(text).alignment(ratatui::layout::Alignment::Center).block(
            Block::default()
                .title(" action ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.warn))
                .style(Style::default().bg(theme.surface_alt)),
        ),
        area,
    );
}
fn empty(frame: &mut Frame, area: Rect, message: &str) {
    frame.render_widget(
        Paragraph::new(message)
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::default().borders(Borders::ALL)),
        area,
    );
}
fn panel<'a>(theme: Theme, title: &'a str) -> Block<'a> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .style(theme.panel())
}
fn yes_no(value: bool) -> String {
    if value {
        "yes".into()
    } else {
        "no".into()
    }
}
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
