use ratatui::style::{Color, Modifier, Style};

use crate::config::ThemeName;

#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub good: Color,
    pub warn: Color,
    pub bad: Color,
    pub selected: Color,
}

impl Theme {
    pub fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Default => Self {
                background: Color::Rgb(10, 14, 20),
                surface: Color::Rgb(17, 24, 34),
                surface_alt: Color::Rgb(24, 33, 46),
                border: Color::Rgb(48, 66, 87),
                text: Color::Rgb(225, 234, 242),
                muted: Color::Rgb(132, 151, 168),
                accent: Color::Rgb(80, 205, 190),
                good: Color::Rgb(105, 210, 138),
                warn: Color::Rgb(244, 185, 74),
                bad: Color::Rgb(238, 103, 105),
                selected: Color::Rgb(34, 74, 78),
            },
            ThemeName::Midnight => Self {
                background: Color::Rgb(8, 10, 19),
                surface: Color::Rgb(16, 20, 36),
                surface_alt: Color::Rgb(29, 35, 58),
                border: Color::Rgb(58, 67, 106),
                text: Color::Rgb(230, 233, 248),
                muted: Color::Rgb(145, 151, 180),
                accent: Color::Rgb(139, 142, 255),
                good: Color::Rgb(106, 211, 153),
                warn: Color::Rgb(242, 184, 91),
                bad: Color::Rgb(241, 105, 125),
                selected: Color::Rgb(48, 47, 93),
            },
            ThemeName::Amber => Self {
                background: Color::Rgb(17, 13, 8),
                surface: Color::Rgb(31, 23, 13),
                surface_alt: Color::Rgb(52, 37, 18),
                border: Color::Rgb(98, 70, 29),
                text: Color::Rgb(247, 235, 209),
                muted: Color::Rgb(174, 149, 111),
                accent: Color::Rgb(255, 178, 73),
                good: Color::Rgb(140, 208, 123),
                warn: Color::Rgb(255, 178, 73),
                bad: Color::Rgb(245, 113, 78),
                selected: Color::Rgb(83, 55, 18),
            },
            ThemeName::Mono => Self {
                background: Color::Black,
                surface: Color::Rgb(18, 18, 18),
                surface_alt: Color::Rgb(34, 34, 34),
                border: Color::DarkGray,
                text: Color::White,
                muted: Color::Gray,
                accent: Color::White,
                good: Color::White,
                warn: Color::White,
                bad: Color::White,
                selected: Color::DarkGray,
            },
        }
    }

    pub fn panel(self) -> Style {
        Style::default().bg(self.surface).fg(self.text)
    }
    pub fn title(self) -> Style {
        Style::default().fg(self.accent).add_modifier(Modifier::BOLD)
    }
}
