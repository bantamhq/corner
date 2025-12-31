use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line as RatatuiLine, Span},
};

use crate::app::{App, Mode};
use crate::storage::{EntryType, Line};

pub fn render_tasks_view(app: &App) -> Vec<RatatuiLine<'static>> {
    use chrono::NaiveDate;

    let mut lines = Vec::new();
    let mut last_date: Option<NaiveDate> = None;

    for (idx, item) in app.task_items.iter().enumerate() {
        if last_date != Some(item.date) {
            let date_str = item.date.format("%m/%d/%y").to_string();
            lines.push(RatatuiLine::from(Span::styled(
                date_str,
                Style::default().fg(Color::Cyan),
            )));
            last_date = Some(item.date);
        }

        let is_selected = idx == app.task_selected;
        let content_style = if item.completed {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        if is_selected {
            let checkbox = if item.completed { " [x] " } else { " [ ] " };
            lines.push(RatatuiLine::from(vec![
                Span::styled("→", Style::default().fg(Color::Cyan)),
                Span::styled(format!("{}{}", checkbox, item.content), content_style),
            ]));
        } else {
            let checkbox = if item.completed { "- [x] " } else { "- [ ] " };
            lines.push(RatatuiLine::from(Span::styled(
                format!("{}{}", checkbox, item.content),
                content_style,
            )));
        }
    }

    if lines.is_empty() {
        lines.push(RatatuiLine::from(Span::styled(
            "(no incomplete tasks)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}

pub fn render_daily_view(app: &App) -> Vec<RatatuiLine<'static>> {
    let mut lines = Vec::new();

    let date_str = app.current_date.format("%m/%d/%y").to_string();
    lines.push(RatatuiLine::from(Span::styled(
        date_str,
        Style::default().fg(Color::Cyan),
    )));

    for (entry_idx, &line_idx) in app.entry_indices.iter().enumerate() {
        if let Line::Entry(entry) = &app.lines[line_idx] {
            let is_selected = entry_idx == app.selected;
            let is_editing = is_selected && app.mode == Mode::Edit;

            let content_style = if matches!(entry.entry_type, EntryType::Task { completed: true }) {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            let text = if is_editing {
                if let Some(ref buffer) = app.edit_buffer {
                    buffer.content().to_string()
                } else {
                    entry.content.clone()
                }
            } else {
                entry.content.clone()
            };

            if is_selected && !is_editing {
                let rest_of_prefix = entry.prefix().chars().skip(1).collect::<String>();
                lines.push(RatatuiLine::from(vec![
                    Span::styled("→", Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{rest_of_prefix}{text}"), content_style),
                ]));
            } else {
                lines.push(RatatuiLine::from(Span::styled(
                    format!("{}{}", entry.prefix(), text),
                    content_style,
                )));
            }
        }
    }

    if app.entry_indices.is_empty() {
        lines.push(RatatuiLine::from(Span::styled(
            "(No entries - press Enter to add)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}

pub fn render_editing_cursor(app: &App, lines: &mut [RatatuiLine<'static>]) {
    if app.mode != Mode::Edit {
        return;
    }

    let Some(ref buffer) = app.edit_buffer else {
        return;
    };
    let Some(entry) = app.get_selected_entry() else {
        return;
    };

    let line_idx = app.selected + 1; // +1 for date header
    if line_idx < lines.len() {
        let before = buffer.text_before_cursor();
        let cursor_char = buffer.char_at_cursor().unwrap_or(' ');
        let after = buffer.text_after_cursor();

        lines[line_idx] = RatatuiLine::from(vec![
            Span::raw(format!("{}{}", entry.prefix(), before)),
            Span::styled(
                cursor_char.to_string(),
                Style::default().add_modifier(Modifier::REVERSED),
            ),
            Span::raw(after.to_string()),
        ]);
    }
}

pub fn render_footer(app: &App) -> RatatuiLine<'static> {
    match app.mode {
        Mode::Command => RatatuiLine::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow)),
            Span::raw(app.command_buffer.clone()),
            Span::styled("█", Style::default().fg(Color::White)),
        ]),
        Mode::Edit => RatatuiLine::from(vec![
            Span::styled(" EDIT ", Style::default().fg(Color::Black).bg(Color::Green)),
            Span::styled("  Enter", Style::default().fg(Color::Gray)),
            Span::styled(" Save and new  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(Color::Gray)),
            Span::styled(" Toggle entry type  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Gray)),
            Span::styled(" Daily mode", Style::default().fg(Color::DarkGray)),
        ]),
        Mode::Daily => RatatuiLine::from(vec![
            Span::styled(" DAILY ", Style::default().fg(Color::Black).bg(Color::Blue)),
            Span::styled("  Enter", Style::default().fg(Color::Gray)),
            Span::styled(" New entry  ", Style::default().fg(Color::DarkGray)),
            Span::styled("e", Style::default().fg(Color::Gray)),
            Span::styled(" Edit  ", Style::default().fg(Color::DarkGray)),
            Span::styled("i", Style::default().fg(Color::Gray)),
            Span::styled(" Insert below  ", Style::default().fg(Color::DarkGray)),
            Span::styled("x", Style::default().fg(Color::Gray)),
            Span::styled(" Toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled("d", Style::default().fg(Color::Gray)),
            Span::styled(" Delete  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(Color::Gray)),
            Span::styled(" Tasks", Style::default().fg(Color::DarkGray)),
        ]),
        Mode::Tasks => RatatuiLine::from(vec![
            Span::styled(
                " TASKS ",
                Style::default().fg(Color::Black).bg(Color::Magenta),
            ),
            Span::styled("  j/k", Style::default().fg(Color::Gray)),
            Span::styled(" Move up/down  ", Style::default().fg(Color::DarkGray)),
            Span::styled("x", Style::default().fg(Color::Gray)),
            Span::styled(" Toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Gray)),
            Span::styled(" Go to day  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(Color::Gray)),
            Span::styled(" Daily mode", Style::default().fg(Color::DarkGray)),
        ]),
    }
}
