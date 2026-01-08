use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::cursor::CursorBuffer;

pub const POPUP_WIDTH: u16 = 26;
pub const POPUP_HEIGHT: u16 = 11;

pub struct PopupLayout {
    pub popup_area: Rect,
    pub content_area: Rect,
    pub query_area: Rect,
}

impl PopupLayout {
    #[must_use]
    pub fn new(area: Rect) -> Self {
        let popup_width = POPUP_WIDTH.min(area.width.saturating_sub(4));
        let popup_height = POPUP_HEIGHT.min(area.height.saturating_sub(4));

        let popup_area = centered_fixed_rect(popup_width, popup_height, area);

        let inner = Rect {
            x: popup_area.x + 1,
            y: popup_area.y + 1,
            width: popup_area.width.saturating_sub(2),
            height: popup_area.height.saturating_sub(2),
        };

        // Add horizontal padding (1 char each side)
        let content_area = Rect {
            x: inner.x + 1,
            y: inner.y,
            width: inner.width.saturating_sub(2),
            height: inner.height.saturating_sub(2),
        };

        let query_area = Rect {
            x: inner.x + 1,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width.saturating_sub(2),
            height: 1,
        };

        Self {
            popup_area,
            content_area,
            query_area,
        }
    }

    #[must_use]
    pub fn is_too_small(&self) -> bool {
        self.content_area.height < 1
    }
}

fn centered_fixed_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

pub fn render_popup_frame(f: &mut Frame, layout: &PopupLayout, title: &str) {
    f.render_widget(Clear, layout.popup_area);

    let block = Block::default()
        .title(Span::styled(format!(" {title} "), Style::new().fg(Color::Cyan)))
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Cyan));

    f.render_widget(block, layout.popup_area);
}

pub fn render_query_input(f: &mut Frame, layout: &PopupLayout, query: &CursorBuffer, focused: bool) {
    let style = if focused {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new().fg(Color::Cyan).dim()
    };

    let query_line = Line::from(vec![
        Span::styled("> ", style),
        Span::styled(query.content().to_string(), style),
    ]);
    f.render_widget(Paragraph::new(query_line), layout.query_area);

    if focused {
        let cursor_x = layout.query_area.x + 2 + query.cursor_display_pos() as u16;
        f.set_cursor_position((cursor_x, layout.query_area.y));
    }
}
