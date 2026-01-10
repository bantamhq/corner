use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::TagInterfaceState;

use super::interface_popup::{PopupLayout, render_popup_frame, render_scroll_indicators};

pub fn render_tag_interface(f: &mut Frame, state: &TagInterfaceState, area: Rect) {
    let layout = PopupLayout::without_query(area);

    if layout.is_too_small() {
        return;
    }

    render_popup_frame(f, &layout, "Tags");

    let visible_height = layout.content_area.height as usize;
    let total_items = state.tags.len();

    render_scroll_indicators(f, &layout, state.scroll_offset, visible_height, total_items);

    let content_width = layout.content_area.width as usize;

    let mut lines = Vec::new();
    for (i, tag) in state
        .tags
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(visible_height)
    {
        let is_selected = i == state.selected;

        let style = if is_selected {
            Style::new().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::new().fg(Color::Yellow)
        };

        let count_style = if is_selected {
            Style::new().fg(Color::Black).bg(Color::Yellow).dim()
        } else {
            Style::new().dim()
        };

        let tag_text = format!(" #{}", tag.name);
        let count_text = format!("({}) ", tag.count);
        let text_len = tag_text.len() + count_text.len();
        let padding = " ".repeat(content_width.saturating_sub(text_len));

        let spans = vec![
            Span::styled(tag_text, style),
            Span::styled(padding, style),
            Span::styled(count_text, count_style),
        ];

        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        let message = "No tags in journal";
        lines.push(Line::from(Span::styled(message, Style::new().dim())));
    }

    f.render_widget(Paragraph::new(lines), layout.content_area);
}
