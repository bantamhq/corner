use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::ProjectInterfaceState;

use super::interface_popup::{PopupLayout, render_popup_frame, render_scroll_indicators};

pub fn render_project_interface(
    f: &mut Frame,
    state: &ProjectInterfaceState,
    area: Rect,
    current_project_id: Option<&str>,
) {
    let layout = PopupLayout::without_query(area);

    if layout.is_too_small() {
        return;
    }

    render_popup_frame(f, &layout, "Projects");

    let visible_height = layout.content_area.height as usize;
    let total_items = state.projects.len();

    render_scroll_indicators(f, &layout, state.scroll_offset, visible_height, total_items);

    let content_width = layout.content_area.width as usize;

    let mut lines = Vec::new();
    for (i, project) in state
        .projects
        .iter()
        .enumerate()
        .skip(state.scroll_offset)
        .take(visible_height)
    {
        let is_selected = i == state.selected;
        let is_current = current_project_id.is_some_and(|id| project.id.eq_ignore_ascii_case(id));

        let current_prefix = if is_current { "◆ " } else { "  " };

        let style = if !project.available {
            if is_selected {
                Style::new().dim().reversed()
            } else {
                Style::new().dim()
            }
        } else if is_selected {
            Style::new().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::new().fg(Color::Yellow)
        };

        // Use 2 for prefix display width (not .len()) since ◆ is multi-byte UTF-8
        let text_len = 2 + project.name.len();
        let padding = " ".repeat(content_width.saturating_sub(text_len));

        let spans = vec![
            Span::styled(current_prefix, style),
            Span::styled(project.name.clone(), style),
            Span::styled(padding, style),
        ];

        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        let message = "No projects registered";
        lines.push(Line::from(Span::styled(message, Style::new().dim())));
    }

    f.render_widget(Paragraph::new(lines), layout.content_area);
}
