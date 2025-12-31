use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
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
            Span::styled(" Tasks  ", Style::default().fg(Color::DarkGray)),
            Span::styled("?", Style::default().fg(Color::Gray)),
            Span::styled(" Help", Style::default().fg(Color::DarkGray)),
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
            Span::styled(" Daily mode  ", Style::default().fg(Color::DarkGray)),
            Span::styled("?", Style::default().fg(Color::Gray)),
            Span::styled(" Help", Style::default().fg(Color::DarkGray)),
        ]),
    }
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

#[allow(clippy::vec_init_then_push)]
pub fn get_help_lines() -> Vec<RatatuiLine<'static>> {
    let header_style = Style::default().fg(Color::Cyan);
    let key_style = Style::default().fg(Color::Yellow);
    let desc_style = Style::default().fg(Color::White);

    let mut lines = Vec::new();

    // Daily mode
    lines.push(
        RatatuiLine::from(Span::styled("--- Daily ---", header_style))
            .alignment(Alignment::Center),
    );
    lines.push(help_line(
        "Enter",
        "New entry at end",
        key_style,
        desc_style,
    ));
    lines.push(help_line("i", "Insert entry below", key_style, desc_style));
    lines.push(help_line("e", "Edit selected", key_style, desc_style));
    lines.push(help_line(
        "x",
        "Toggle task complete",
        key_style,
        desc_style,
    ));
    lines.push(help_line("d", "Delete entry", key_style, desc_style));
    lines.push(help_line("j/k", "Navigate up/down", key_style, desc_style));
    lines.push(help_line("h/l", "Previous/next day", key_style, desc_style));
    lines.push(help_line("[/]", "Previous/next day", key_style, desc_style));
    lines.push(help_line("t", "Go to today", key_style, desc_style));
    lines.push(help_line("Tab", "Tasks view", key_style, desc_style));
    lines.push(help_line(":", "Command mode", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Edit mode
    lines.push(
        RatatuiLine::from(Span::styled("--- Edit ---", header_style))
            .alignment(Alignment::Center),
    );
    lines.push(help_line(
        "Enter",
        "Save and add new",
        key_style,
        desc_style,
    ));
    lines.push(help_line("Tab", "Toggle entry type", key_style, desc_style));
    lines.push(help_line("Esc", "Save and exit", key_style, desc_style));
    lines.push(help_line("←/→", "Move cursor", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Tasks mode
    lines.push(
        RatatuiLine::from(Span::styled("--- Tasks ---", header_style))
            .alignment(Alignment::Center),
    );
    lines.push(help_line("j/k", "Navigate up/down", key_style, desc_style));
    lines.push(help_line("x", "Toggle task", key_style, desc_style));
    lines.push(help_line("Enter", "Go to day", key_style, desc_style));
    lines.push(help_line("Tab", "Daily view", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Commands
    lines.push(
        RatatuiLine::from(Span::styled("--- Commands ---", header_style))
            .alignment(Alignment::Center),
    );
    lines.push(help_line(
        ":goto",
        "Go to date (YYYY/MM/DD or MM/DD)",
        key_style,
        desc_style,
    ));
    lines.push(help_line(
        ":gt",
        "Go to date (shorthand)",
        key_style,
        desc_style,
    ));
    lines.push(help_line(":q", "Quit", key_style, desc_style));

    lines
}

fn help_line(key: &str, desc: &str, key_style: Style, desc_style: Style) -> RatatuiLine<'static> {
    RatatuiLine::from(vec![
        Span::styled(format!("{key:>8}  "), key_style),
        Span::styled(desc.to_string(), desc_style),
    ])
}

pub fn get_help_total_lines() -> usize {
    get_help_lines().len()
}

pub fn render_help_content(scroll: usize, visible_height: usize) -> Vec<RatatuiLine<'static>> {
    get_help_lines()
        .into_iter()
        .skip(scroll)
        .take(visible_height)
        .collect()
}
