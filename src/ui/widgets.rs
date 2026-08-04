use ratatui::{style::Style, widgets::{Block, Borders}};

pub fn bordered<'a>(title: &'a str, border: ratatui::style::Color) -> Block<'a> {
    Block::default().title(title).borders(Borders::ALL).border_style(Style::default().fg(border))
}
