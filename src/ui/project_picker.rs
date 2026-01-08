use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::ProjectPickerState;

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

pub fn render_project_picker(f: &mut Frame, state: &ProjectPickerState, area: Rect) {
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 12.min(area.height.saturating_sub(4));

    let popup_area = centered_fixed_rect(popup_width, popup_height, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(" Projects ", Style::new().fg(Color::Cyan)))
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::Cyan));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if inner.height < 3 {
        return;
    }

    let query_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let query_line = Line::from(vec![
        Span::styled("> ", Style::new().fg(Color::Cyan)),
        Span::raw(state.query.content().to_string()),
    ]);
    f.render_widget(Paragraph::new(query_line), query_area);

    // List area (leave 1 line for query, 1 for separator, 1 for footer)
    let list_area = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(3),
    };

    let mut lines = Vec::new();
    for (i, &project_idx) in state.filtered_indices.iter().enumerate() {
        if i >= list_area.height as usize {
            break;
        }

        let project = &state.projects[project_idx];
        let is_selected = i == state.selected;

        let indicator = if is_selected { "→" } else { " " };

        let name_style = if !project.available {
            Style::new().dim()
        } else if is_selected {
            Style::new().fg(Color::Cyan)
        } else {
            Style::new()
        };

        let mut spans = vec![
            Span::styled(format!("{} ", indicator), Style::new().fg(Color::Cyan)),
            Span::styled(project.name.clone(), name_style),
        ];

        if !project.available {
            spans.push(Span::styled(" (unavailable)", Style::new().fg(Color::Red).dim()));
        }

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

    f.render_widget(Paragraph::new(lines), list_area);

    let cursor_x = query_area.x + 2 + state.query.cursor_display_pos() as u16;
    f.set_cursor_position((cursor_x, query_area.y));
}
