use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::theme;

use crate::app::TagInterfaceState;

use super::interface_modal::{
    PopupLayout, build_list_item_line, render_popup_frame, render_scroll_indicators,
};

pub fn render_tag_interface(f: &mut Frame, state: &TagInterfaceState, area: Rect) {
    let layout = PopupLayout::without_query(area);

    if layout.is_too_small() {
        return;
    }

    render_popup_frame(f, &layout, "Manage Tags");

    let visible_height = layout.content_area.height as usize;
    let total_items = state.tags.len();

    render_scroll_indicators(f, &layout, state.scroll_offset, visible_height, total_items);

    let content_width = layout.content_area.width as usize;

    let lines: Vec<Line> = if state.tags.is_empty() {
        vec![Line::from(Span::styled(
            "No tags in journal",
            Style::new().dim(),
        ))]
    } else {
        state
            .tags
            .iter()
            .enumerate()
            .skip(state.scroll_offset)
            .take(visible_height)
            .map(|(i, tag)| {
                let is_selected = i == state.selected;

                let style = if is_selected {
                    Style::new()
                        .fg(theme::TAG_SELECTED_FG)
                        .bg(theme::TAG_SELECTED_BG)
                } else {
                    Style::new().fg(theme::TAG_NORMAL_FG)
                };

                let count_style = if is_selected {
                    style.dim()
                } else {
                    Style::new().dim()
                };

                let count_text = format!("({}) ", tag.count);
                build_list_item_line(
                    " #",
                    &tag.name,
                    &count_text,
                    content_width,
                    style,
                    count_style,
                )
            })
            .collect()
    };

    f.render_widget(Paragraph::new(lines), layout.content_area);
}
