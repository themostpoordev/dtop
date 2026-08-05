use ratatui::{
    layout::{Constraint, Rect},
    widgets::{Row, Table},
    Frame,
};

use crate::{app::App, model::format_bytes};

use super::{empty, panel, Theme};

pub(super) fn resources(
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

pub(super) fn resource_rows(app: &App, kind: &str) -> Vec<Row<'static>> {
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
