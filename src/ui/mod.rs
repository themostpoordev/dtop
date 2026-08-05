mod containers;
mod details;
mod events;
mod home;
mod logs;
mod overview;
mod resources;
mod settings;
mod theme;

pub use theme::Theme;

pub(crate) use home::summary_cards;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::{
    app::App,
    model::{format_rate, ConnectionState, Screen},
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
        Screen::Home => home::home(frame, app, outer[1], theme),
        Screen::Overview => overview::overview(frame, app, outer[1], theme),
        Screen::Containers => containers::containers(frame, app, outer[1], theme),
        Screen::Details => details::details(frame, app, outer[1], theme),
        Screen::Logs => logs::logs(frame, app, outer[1], theme),
        Screen::Events => events::events(frame, app, outer[1], theme),
        Screen::Images => resources::resources(
            frame,
            app,
            outer[1],
            theme,
            "Images",
            resources::resource_rows(app, "images"),
        ),
        Screen::Volumes => resources::resources(
            frame,
            app,
            outer[1],
            theme,
            "Volumes",
            resources::resource_rows(app, "volumes"),
        ),
        Screen::Networks => resources::resources(
            frame,
            app,
            outer[1],
            theme,
            "Networks",
            resources::resource_rows(app, "networks"),
        ),
        Screen::Settings => settings::settings(frame, app, outer[1], theme),
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
