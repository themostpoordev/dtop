mod containers;
mod cpu;
mod details;
mod disk;
mod events;
mod home;
mod logs;
mod memory;
mod network;
mod overview;
mod processes;
mod resources;
mod settings;
mod system;
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
        Screen::System => system::system(frame, app, outer[1], theme),
        Screen::Cpu => cpu::cpu(frame, app, outer[1], theme),
        Screen::Memory => memory::memory(frame, app, outer[1], theme),
        Screen::Disk => disk::disk(frame, app, outer[1], theme),
        Screen::Network => network::network(frame, app, outer[1], theme),
        Screen::Processes => processes::processes(frame, app, outer[1], theme),
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
        .constraints([Constraint::Length(22), Constraint::Min(20), Constraint::Length(30)])
        .split(area);
    let mode_label = match app.config.mode {
        crate::config::Mode::Docker => "docker",
        crate::config::Mode::All => "all",
    };
    let logo = Paragraph::new(Line::from(vec![
        Span::styled(" ◈ dtop", theme.title()),
        Span::styled(" 1.0", Style::default().fg(theme.muted)),
        Span::styled(format!(" [{mode_label}]"), Style::default().fg(theme.accent)),
    ]))
    .block(
        Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(theme.border)),
    );
    frame.render_widget(logo, chunks[0]);
    let primary = Screen::primary(app.config.mode);
    let tabs =
        Tabs::new(primary.iter().map(|screen| Line::from(screen.label())).collect::<Vec<_>>())
            .select(primary.iter().position(|screen| *screen == app.screen).unwrap_or(0))
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

/// Build a proportional bar for a 0–100 value, bounded to `width` cells.
fn bar<'a>(value: f64, width: usize, color: ratatui::style::Color) -> Span<'a> {
    let filled = ((value.clamp(0.0, 100.0) / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    Span::styled(format!("{}{}", "█".repeat(filled), "░".repeat(empty)), Style::default().fg(color))
}

/// Render a time series as a full-height gradient sparkline.
///
/// Returns `Vec<SparklineBar>` where every bar gets a lerped color from `low`
/// to `high` across the value range. The sparkline widget fills the whole
/// area it is rendered into — no more tiny 1-row line in a tall frame.
pub fn gradient_bars(
    values: &[u64],
    low: ratatui::style::Color,
    high: ratatui::style::Color,
    max: u64,
) -> Vec<ratatui::widgets::SparklineBar> {
    let max = max.max(1);
    values
        .iter()
        .map(|value| {
            let v = (*value).min(max);
            ratatui::widgets::SparklineBar::from(v).style(Some(Style::default().fg(lerp_color(
                low,
                high,
                v as f64 / max as f64,
            ))))
        })
        .collect()
}

/// Linear interpolation between two RGB colors, `t` in 0..=1.
pub fn lerp_color(
    low: ratatui::style::Color,
    high: ratatui::style::Color,
    t: f64,
) -> ratatui::style::Color {
    let (lr, lg, lb) = rgb_of(low);
    let (hr, hg, hb) = rgb_of(high);
    let t = t.clamp(0.0, 1.0);
    ratatui::style::Color::Rgb(
        (lr as f64 + (hr as f64 - lr as f64) * t).round() as u8,
        (lg as f64 + (hg as f64 - lg as f64) * t).round() as u8,
        (lb as f64 + (hb as f64 - lb as f64) * t).round() as u8,
    )
}

fn rgb_of(color: ratatui::style::Color) -> (u8, u8, u8) {
    match color {
        ratatui::style::Color::Rgb(r, g, b) => (r, g, b),
        ratatui::style::Color::Black => (0, 0, 0),
        ratatui::style::Color::White => (255, 255, 255),
        ratatui::style::Color::Gray => (128, 128, 128),
        ratatui::style::Color::DarkGray => (64, 64, 64),
        ratatui::style::Color::LightRed => (255, 128, 128),
        ratatui::style::Color::LightGreen => (128, 255, 128),
        ratatui::style::Color::LightYellow => (255, 255, 128),
        ratatui::style::Color::LightBlue => (128, 128, 255),
        ratatui::style::Color::LightMagenta => (255, 128, 255),
        ratatui::style::Color::LightCyan => (128, 255, 255),
        ratatui::style::Color::Red => (200, 40, 40),
        ratatui::style::Color::Green => (40, 200, 40),
        ratatui::style::Color::Yellow => (200, 200, 40),
        ratatui::style::Color::Blue => (40, 40, 200),
        ratatui::style::Color::Magenta => (200, 40, 200),
        ratatui::style::Color::Cyan => (40, 200, 200),
        _ => (128, 128, 128),
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
