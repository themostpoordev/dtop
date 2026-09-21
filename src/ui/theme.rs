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
                surface_alt: Color::Rgb(25, 34, 48),
                border: Color::Rgb(58, 78, 102),
                text: Color::Rgb(228, 236, 244),
                muted: Color::Rgb(140, 158, 175),
                accent: Color::Rgb(86, 210, 194),
                good: Color::Rgb(110, 214, 142),
                warn: Color::Rgb(246, 190, 80),
                bad: Color::Rgb(240, 110, 112),
                selected: Color::Rgb(36, 78, 84),
            },
            ThemeName::Midnight => Self {
                background: Color::Rgb(8, 10, 19),
                surface: Color::Rgb(16, 20, 36),
                surface_alt: Color::Rgb(30, 36, 60),
                border: Color::Rgb(64, 73, 114),
                text: Color::Rgb(232, 235, 250),
                muted: Color::Rgb(152, 158, 188),
                accent: Color::Rgb(146, 149, 255),
                good: Color::Rgb(112, 214, 158),
                warn: Color::Rgb(244, 188, 96),
                bad: Color::Rgb(243, 110, 130),
                selected: Color::Rgb(52, 50, 98),
            },
            // Warm monochrome by design — but every semantic color stays
            // distinguishable: amber accent, deep-orange warn, brick-red bad.
            ThemeName::Amber => Self {
                background: Color::Rgb(17, 13, 8),
                surface: Color::Rgb(31, 23, 13),
                surface_alt: Color::Rgb(54, 38, 19),
                border: Color::Rgb(104, 74, 30),
                text: Color::Rgb(248, 236, 210),
                muted: Color::Rgb(180, 154, 114),
                accent: Color::Rgb(255, 184, 77),
                good: Color::Rgb(148, 212, 126),
                warn: Color::Rgb(255, 138, 61),
                bad: Color::Rgb(244, 88, 59),
                selected: Color::Rgb(88, 58, 20),
            },
            // True mono: hierarchy comes from brightness steps, not hue.
            // Muted text and borders are lifted so they stay legible.
            ThemeName::Mono => Self {
                background: Color::Black,
                surface: Color::Rgb(16, 16, 16),
                surface_alt: Color::Rgb(42, 42, 42),
                border: Color::Rgb(112, 112, 112),
                text: Color::White,
                muted: Color::Rgb(172, 172, 172),
                accent: Color::White,
                good: Color::Rgb(225, 225, 225),
                warn: Color::White,
                bad: Color::White,
                selected: Color::Rgb(62, 62, 62),
            },
            ThemeName::Ocean => Self {
                background: Color::Rgb(7, 15, 25),
                surface: Color::Rgb(12, 26, 40),
                surface_alt: Color::Rgb(20, 40, 60),
                border: Color::Rgb(48, 104, 144),
                text: Color::Rgb(226, 240, 250),
                muted: Color::Rgb(134, 168, 192),
                accent: Color::Rgb(72, 202, 232),
                good: Color::Rgb(94, 222, 162),
                warn: Color::Rgb(250, 192, 92),
                bad: Color::Rgb(246, 112, 122),
                selected: Color::Rgb(22, 72, 102),
            },
            ThemeName::Rose => Self {
                background: Color::Rgb(20, 10, 16),
                surface: Color::Rgb(32, 18, 28),
                surface_alt: Color::Rgb(52, 29, 46),
                border: Color::Rgb(114, 62, 92),
                text: Color::Rgb(248, 232, 240),
                muted: Color::Rgb(188, 152, 172),
                accent: Color::Rgb(255, 132, 172),
                good: Color::Rgb(142, 222, 152),
                warn: Color::Rgb(250, 192, 102),
                bad: Color::Rgb(255, 92, 112),
                selected: Color::Rgb(88, 36, 62),
            },
            ThemeName::Forest => Self {
                background: Color::Rgb(9, 16, 12),
                surface: Color::Rgb(16, 28, 20),
                surface_alt: Color::Rgb(27, 45, 33),
                border: Color::Rgb(62, 102, 72),
                text: Color::Rgb(230, 242, 230),
                muted: Color::Rgb(152, 178, 156),
                accent: Color::Rgb(152, 222, 132),
                good: Color::Rgb(112, 212, 142),
                warn: Color::Rgb(242, 192, 92),
                bad: Color::Rgb(242, 112, 102),
                selected: Color::Rgb(36, 72, 46),
            },
            // Light theme for bright terminal backgrounds: dark ink on
            // paper, with deep (not neon) semantic colors for contrast.
            ThemeName::Paper => Self {
                background: Color::Rgb(242, 240, 235),
                surface: Color::Rgb(255, 255, 253),
                surface_alt: Color::Rgb(231, 227, 219),
                border: Color::Rgb(188, 180, 166),
                text: Color::Rgb(45, 42, 38),
                muted: Color::Rgb(118, 110, 98),
                accent: Color::Rgb(0, 130, 140),
                good: Color::Rgb(30, 140, 70),
                warn: Color::Rgb(178, 108, 0),
                bad: Color::Rgb(190, 50, 45),
                selected: Color::Rgb(214, 230, 224),
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
