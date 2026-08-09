use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

use crate::{app::App, model::Screen};

use super::{panel, Theme};

pub(super) fn home(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
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
    let items: Vec<&str> =
        Screen::primary(app.config.mode).iter().map(|screen| screen.label()).collect();
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

pub fn summary_cards(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
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
                Style::default().fg(color).add_modifier(ratatui::style::Modifier::BOLD),
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
