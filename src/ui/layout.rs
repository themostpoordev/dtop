use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub fn split_main(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(3), Constraint::Min(5)]).split(area);
    (chunks[0], chunks[1])
}
