use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::scroll_indicator::{ScrollIndicatorStyle, scroll_indicator_text};
use super::theme;
use unicode_width::UnicodeWidthStr;

use crate::cursor::CursorBuffer;

use super::shared::truncate_text;

pub const POPUP_WIDTH: u16 = 26;
pub const POPUP_HEIGHT: u16 = 11;

pub struct PopupLayout {
    pub popup_area: Rect,
    pub content_area: Rect,
    pub query_area: Rect,
    pub scroll_indicator_area: Rect,
}

impl PopupLayout {
    /// Layout with query input area (for date interface)
    #[must_use]
    pub fn with_query(area: Rect) -> Self {
        Self::create(area, true)
    }

    /// Layout without query input area (for tag/project interfaces)
    #[must_use]
    pub fn without_query(area: Rect) -> Self {
        Self::create(area, false)
    }

    fn create(area: Rect, has_query: bool) -> Self {
        let popup_width = POPUP_WIDTH.min(area.width.saturating_sub(4));
        let popup_height = POPUP_HEIGHT.min(area.height.saturating_sub(4));

        let popup_area = centered_fixed_rect(popup_width, popup_height, area);

        let inner = Rect {
            x: popup_area.x + 1,
            y: popup_area.y + 1,
            width: popup_area.width.saturating_sub(2),
            height: popup_area.height.saturating_sub(2),
        };

        // Reserve 2 lines if query input (query + legend), 1 for scroll indicator row
        let reserved_lines = if has_query { 2 } else { 1 };

        let content_area = Rect {
            x: inner.x + 1,
            y: inner.y,
            width: inner.width.saturating_sub(2),
            height: inner.height.saturating_sub(reserved_lines),
        };

        let query_area = Rect {
            x: inner.x + 1,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width.saturating_sub(2),
            height: 1,
        };

        // Scroll indicator: above query if has_query, at bottom row otherwise
        let scroll_indicator_area = Rect {
            x: inner.x + 1,
            y: if has_query {
                query_area.y.saturating_sub(1)
            } else {
                query_area.y
            },
            width: inner.width.saturating_sub(2),
            height: 1,
        };

        Self {
            popup_area,
            content_area,
            query_area,
            scroll_indicator_area,
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
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(theme::POPUP_TITLE),
        ))
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme::POPUP_BORDER));

    f.render_widget(block, layout.popup_area);
}

pub fn render_query_input(
    f: &mut Frame,
    layout: &PopupLayout,
    query: &CursorBuffer,
    focused: bool,
) {
    let style = if focused {
        Style::new().fg(theme::POPUP_QUERY)
    } else {
        Style::new().fg(theme::POPUP_QUERY_DIM).dim()
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

pub fn render_scroll_indicators(
    f: &mut Frame,
    layout: &PopupLayout,
    scroll_offset: usize,
    visible_height: usize,
    total_items: usize,
) {
    let can_scroll_up = scroll_offset > 0;
    let can_scroll_down = scroll_offset + visible_height < total_items;

    if let Some(arrows) =
        scroll_indicator_text(can_scroll_up, can_scroll_down, ScrollIndicatorStyle::Arrows)
    {
        let indicator = Paragraph::new(Span::styled(
            arrows,
            Style::new().fg(theme::POPUP_SCROLL).dim(),
        ))
        .alignment(Alignment::Right);
        f.render_widget(indicator, layout.scroll_indicator_area);
    }
}

#[must_use]
pub fn build_list_item_line(
    prefix: &str,
    content: &str,
    suffix: &str,
    content_width: usize,
    content_style: Style,
    suffix_style: Style,
) -> Line<'static> {
    let prefix_width = prefix.width();
    let suffix_width = suffix.width();

    let available_for_content = content_width.saturating_sub(prefix_width + suffix_width);
    let content_display = truncate_text(content, available_for_content);
    let content_display_width = content_display.width();

    let padding_width =
        content_width.saturating_sub(prefix_width + content_display_width + suffix_width);
    let padding = " ".repeat(padding_width);

    let mut spans = vec![
        Span::styled(prefix.to_string(), content_style),
        Span::styled(content_display, content_style),
        Span::styled(padding, content_style),
    ];

    if !suffix.is_empty() {
        spans.push(Span::styled(suffix.to_string(), suffix_style));
    }

    Line::from(spans)
}
