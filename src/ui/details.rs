use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::App;

use super::{empty, panel, Theme};

pub(super) fn details(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
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
