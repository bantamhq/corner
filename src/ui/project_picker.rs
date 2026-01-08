use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::ProjectPickerState;

use super::popup_interface::{PopupLayout, render_popup_frame, render_query_input};

pub fn render_project_picker(f: &mut Frame, state: &ProjectPickerState, area: Rect) {
    let layout = PopupLayout::new(area);

    if layout.is_too_small() {
        return;
    }

    render_popup_frame(f, &layout, "Projects");
    render_query_input(f, &layout, &state.query, true);

    let mut lines = Vec::new();
    for (i, &project_idx) in state.filtered_indices.iter().enumerate() {
        if i >= layout.content_area.height as usize {
            break;
        }

        let project = &state.projects[project_idx];
        let is_selected = i == state.selected;

        let indicator = if is_selected { "→" } else { " " };

        let name_style = if !project.available {
            Style::new().dim()
        } else if is_selected {
            Style::new().fg(Color::Yellow)
        } else {
            Style::new().fg(Color::Yellow).dim()
        };

        let spans = vec![
            Span::styled(format!("{} ", indicator), Style::new().fg(Color::Cyan)),
            Span::styled(project.name.clone(), name_style),
        ];

        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        if state.projects.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No registered projects",
                Style::new().dim(),
            )));
            lines.push(Line::from(Span::styled(
                "  Use :project init to add",
                Style::new().dim(),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  No matching projects",
                Style::new().dim(),
            )));
        }
    }

    f.render_widget(Paragraph::new(lines), layout.content_area);
}
