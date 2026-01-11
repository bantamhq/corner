use ratatui::{
    Frame,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::theme;

use crate::app::ProjectInterfaceState;

use super::interface_modal::{
    PopupLayout, build_list_item_line, render_popup_frame, render_scroll_indicators,
};

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

    render_popup_frame(f, &layout, "Manage Projects");

    let visible_height = layout.content_area.height as usize;
    let total_items = state.projects.len();

    render_scroll_indicators(f, &layout, state.scroll_offset, visible_height, total_items);

    let content_width = layout.content_area.width as usize;

    let lines: Vec<Line> = if state.projects.is_empty() {
        vec![Line::from(Span::styled(
            "No projects registered",
            Style::new().dim(),
        ))]
    } else {
        state
            .projects
            .iter()
            .enumerate()
            .skip(state.scroll_offset)
            .take(visible_height)
            .map(|(i, project)| {
                let is_selected = i == state.selected;
                let is_current =
                    current_project_id.is_some_and(|id| project.id.eq_ignore_ascii_case(id));

                let style = if !project.available {
                    if is_selected {
                        Style::new().dim().reversed()
                    } else {
                        Style::new().dim()
                    }
                } else if is_selected {
                    Style::new()
                        .fg(theme::PROJECT_SELECTED_FG)
                        .bg(theme::PROJECT_SELECTED_BG)
                } else {
                    Style::new().fg(theme::PROJECT_NORMAL_FG)
                };

                let indicator_style = if is_selected {
                    style.dim()
                } else {
                    Style::new().dim()
                };

                let indicator = if is_current { " ◆ " } else { "" };
                build_list_item_line(
                    " ",
                    &project.name,
                    indicator,
                    content_width,
                    style,
                    indicator_style,
                )
            })
            .collect()
    };

    f.render_widget(Paragraph::new(lines), layout.content_area);
}
