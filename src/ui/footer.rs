#![allow(dead_code)]

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line as RatatuiLine,
};

use crate::app::{InputMode, ViewMode};
use crate::dispatch::Keymap;

pub struct FooterModel<'a> {
    pub view: &'a ViewMode,
    pub input_mode: &'a InputMode,
    pub keymap: &'a Keymap,
}

impl<'a> FooterModel<'a> {
    #[must_use]
    pub fn new(view: &'a ViewMode, input_mode: &'a InputMode, keymap: &'a Keymap) -> Self {
        Self {
            view,
            input_mode,
            keymap,
        }
    }
}

pub fn render_footer(_model: FooterModel<'_>) -> RatatuiLine<'static> {
    RatatuiLine::from("")
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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

pub fn centered_rect_max(max_width: u16, max_height: u16, area: Rect) -> Rect {
    let width = area.width.min(max_width);
    let height = area.height.min(max_height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
