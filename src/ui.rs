use std::sync::LazyLock;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line as RatatuiLine, Span},
};

use crate::app::{App, EditContext, InputMode, ViewMode};
use crate::storage::{EntryType, LATER_DATE_REGEX, Line, NATURAL_DATE_REGEX, TAG_REGEX};

fn style_content(text: &str, base_style: Style, muted: bool) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut last_end = 0;

    let tag_color = if muted {
        Color::Rgb(140, 140, 100)
    } else {
        Color::Yellow
    };
    let date_color = if muted {
        Color::Rgb(140, 100, 100)
    } else {
        Color::Red
    };

    let mut matches: Vec<(usize, usize, Color)> = Vec::new();

    for cap in TAG_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), tag_color));
        }
    }

    for cap in LATER_DATE_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), date_color));
        }
    }

    for cap in NATURAL_DATE_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), date_color));
        }
    }

    matches.sort_by_key(|(start, _, _)| *start);

    for (start, end, color) in matches {
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            Style::default().fg(color),
        ));
        last_end = end;
    }

    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }

    spans
}

pub fn render_filter_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
    use unicode_width::UnicodeWidthStr;

    let ViewMode::Filter(state) = &app.view else {
        return vec![];
    };

    let mut lines = Vec::new();

    let header = format!("Filter: {}", state.query);
    lines.push(RatatuiLine::from(Span::styled(
        header,
        Style::default().fg(Color::Magenta),
    )));

    let is_quick_adding = matches!(
        app.input_mode,
        InputMode::Edit(EditContext::FilterQuickAdd { .. })
    );
    let is_editing = matches!(
        app.input_mode,
        InputMode::Edit(EditContext::FilterEdit { .. })
    );

    for (idx, filter_entry) in state.entries.iter().enumerate() {
        let is_selected = idx == state.selected && !is_quick_adding;
        let is_editing_this = is_selected && is_editing;

        let content_style = if filter_entry.completed {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        let text = if is_editing_this {
            if let Some(ref buffer) = app.edit_buffer {
                buffer.content().to_string()
            } else {
                filter_entry.content.clone()
            }
        } else {
            filter_entry.content.clone()
        };

        let prefix = filter_entry.entry_type.prefix();
        let prefix_width = prefix.width();

        let date_suffix = format!(" ({})", filter_entry.source_date.format("%m/%d"));
        let date_suffix_width = date_suffix.width();

        if is_selected {
            if is_editing_this {
                let available = width.saturating_sub(prefix_width + date_suffix_width);
                let wrapped = wrap_text(&text, available);
                for (i, line_text) in wrapped.iter().enumerate() {
                    if i == 0 {
                        let mut spans = vec![Span::styled(prefix.to_string(), content_style)];
                        spans.push(Span::styled(line_text.clone(), content_style));
                        spans.push(Span::styled(
                            date_suffix.clone(),
                            Style::default().fg(Color::DarkGray),
                        ));
                        lines.push(RatatuiLine::from(spans));
                    } else {
                        let indent = " ".repeat(prefix_width);
                        lines.push(RatatuiLine::from(Span::styled(
                            format!("{indent}{line_text}"),
                            content_style,
                        )));
                    }
                }
            } else {
                let sel_prefix = match &filter_entry.entry_type {
                    EntryType::Task { completed: false } => " [ ] ",
                    EntryType::Task { completed: true } => " [x] ",
                    EntryType::Note => " ",
                    EntryType::Event => " ",
                };
                let available = width.saturating_sub(prefix_width + date_suffix_width);
                let display_text = truncate_text(&text, available);
                let mut spans = vec![Span::styled("→", Style::default().fg(Color::Cyan))];
                spans.push(Span::styled(sel_prefix.to_string(), content_style));
                spans.extend(style_content(
                    &display_text,
                    content_style,
                    filter_entry.completed,
                ));
                spans.push(Span::styled(
                    date_suffix,
                    Style::default().fg(Color::DarkGray),
                ));
                lines.push(RatatuiLine::from(spans));
            }
        } else {
            let available = width.saturating_sub(prefix_width + date_suffix_width);
            let display_text = truncate_text(&text, available);
            let mut spans = vec![Span::styled(prefix.to_string(), content_style)];
            spans.extend(style_content(
                &display_text,
                content_style,
                filter_entry.completed,
            ));
            spans.push(Span::styled(
                date_suffix,
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(RatatuiLine::from(spans));
        }
    }

    if let InputMode::Edit(EditContext::FilterQuickAdd { entry_type, .. }) = &app.input_mode {
        let text = if let Some(ref buffer) = app.edit_buffer {
            buffer.content().to_string()
        } else {
            String::new()
        };
        let prefix = entry_type.prefix();
        let prefix_width = prefix.width();
        let available = width.saturating_sub(prefix_width);
        let wrapped = wrap_text(&text, available);

        if wrapped.is_empty() {
            lines.push(RatatuiLine::from(Span::raw(prefix.to_string())));
        } else {
            for (i, line_text) in wrapped.iter().enumerate() {
                if i == 0 {
                    lines.push(RatatuiLine::from(format!("{prefix}{line_text}")));
                } else {
                    let indent = " ".repeat(prefix_width);
                    lines.push(RatatuiLine::from(format!("{indent}{line_text}")));
                }
            }
        }
    }

    if state.entries.is_empty() && !is_quick_adding {
        lines.push(RatatuiLine::from(Span::styled(
            "(no matches)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}

pub fn render_daily_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
    use unicode_width::UnicodeWidthStr;

    let ViewMode::Daily(state) = &app.view else {
        return vec![];
    };

    let mut lines = Vec::new();

    let date_header = app.current_date.format("%m/%d/%y").to_string();
    lines.push(RatatuiLine::from(Span::styled(
        date_header,
        Style::default().fg(Color::Cyan),
    )));

    let later_count = state.later_entries.len();

    // === Later entries section (at top) ===
    for (later_idx, later_entry) in state.later_entries.iter().enumerate() {
        let is_selected = later_idx == state.selected;
        let is_editing = is_selected
            && matches!(
                app.input_mode,
                InputMode::Edit(EditContext::LaterEdit { .. })
            );

        let content_style = if later_entry.completed {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        let text = if is_editing {
            if let Some(ref buffer) = app.edit_buffer {
                buffer.content().to_string()
            } else {
                later_entry.content.clone()
            }
        } else {
            later_entry.content.clone()
        };

        let prefix = later_entry.entry_type.prefix();
        let prefix_width = prefix.width();
        let source_suffix = format!(" ({})", later_entry.source_date.format("%m/%d"));
        let source_suffix_width = source_suffix.width();
        let later_prefix_style = Style::default().fg(Color::Red);

        if is_editing {
            let available = width.saturating_sub(prefix_width + source_suffix_width);
            let wrapped = wrap_text(&text, available);
            for (i, line_text) in wrapped.iter().enumerate() {
                if i == 0 {
                    let first_char = prefix.chars().next().unwrap_or('-').to_string();
                    let rest_of_prefix: String = prefix.chars().skip(1).collect();
                    let spans = vec![
                        Span::styled(first_char, later_prefix_style),
                        Span::styled(rest_of_prefix, content_style),
                        Span::styled(line_text.clone(), content_style),
                        Span::styled(source_suffix.clone(), Style::default().fg(Color::DarkGray)),
                    ];
                    lines.push(RatatuiLine::from(spans));
                } else {
                    let indent = " ".repeat(prefix_width);
                    lines.push(RatatuiLine::from(Span::styled(
                        format!("{indent}{line_text}"),
                        content_style,
                    )));
                }
            }
        } else if is_selected {
            let available = width.saturating_sub(prefix_width + source_suffix_width);
            let display_text = truncate_text(&text, available);
            let rest_of_prefix: String = prefix.chars().skip(1).collect();
            let mut spans = vec![
                Span::styled("→", Style::default().fg(Color::Red)),
                Span::styled(rest_of_prefix, content_style),
            ];
            spans.extend(style_content(
                &display_text,
                content_style,
                later_entry.completed,
            ));
            spans.push(Span::styled(
                source_suffix,
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(RatatuiLine::from(spans));
        } else {
            let available = width.saturating_sub(prefix_width + source_suffix_width);
            let display_text = truncate_text(&text, available);
            let first_char = prefix.chars().next().unwrap_or('-').to_string();
            let rest_of_prefix: String = prefix.chars().skip(1).collect();
            let mut spans = vec![
                Span::styled(first_char, later_prefix_style),
                Span::styled(rest_of_prefix, content_style),
            ];
            spans.extend(style_content(
                &display_text,
                content_style,
                later_entry.completed,
            ));
            spans.push(Span::styled(
                source_suffix,
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(RatatuiLine::from(spans));
        }
    }

    // === Regular entries section ===
    for (entry_idx, &line_idx) in app.entry_indices.iter().enumerate() {
        if let Line::Entry(entry) = &app.lines[line_idx] {
            let selection_idx = later_count + entry_idx;
            let is_selected = selection_idx == state.selected;
            let is_editing =
                is_selected && matches!(app.input_mode, InputMode::Edit(EditContext::Daily { .. }));

            let is_completed = matches!(entry.entry_type, EntryType::Task { completed: true });
            let content_style = if is_completed {
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

            let prefix = entry.prefix();
            let prefix_width = prefix.width();

            if is_editing {
                let wrapped = wrap_text(&text, width.saturating_sub(prefix_width));
                for (i, line_text) in wrapped.iter().enumerate() {
                    if i == 0 {
                        lines.push(RatatuiLine::from(Span::styled(
                            format!("{prefix}{line_text}"),
                            content_style,
                        )));
                    } else {
                        let indent = " ".repeat(prefix_width);
                        lines.push(RatatuiLine::from(Span::styled(
                            format!("{indent}{line_text}"),
                            content_style,
                        )));
                    }
                }
            } else if is_selected {
                let rest_of_prefix = prefix.chars().skip(1).collect::<String>();
                let indicator = if app.input_mode == InputMode::Order {
                    Span::styled("↕", Style::default().fg(Color::Yellow))
                } else {
                    Span::styled("→", Style::default().fg(Color::Cyan))
                };
                let available = width.saturating_sub(prefix_width);
                let display_text = truncate_text(&text, available);
                let mut spans = vec![indicator, Span::styled(rest_of_prefix, content_style)];
                spans.extend(style_content(&display_text, content_style, is_completed));
                lines.push(RatatuiLine::from(spans));
            } else {
                let available = width.saturating_sub(prefix_width);
                let display_text = truncate_text(&text, available);
                let mut spans = vec![Span::styled(prefix.to_string(), content_style)];
                spans.extend(style_content(&display_text, content_style, is_completed));
                lines.push(RatatuiLine::from(spans));
            }
        }
    }

    // Empty state only if both later and regular entries are empty
    if state.later_entries.is_empty() && app.entry_indices.is_empty() {
        lines.push(RatatuiLine::from(Span::styled(
            "(No entries - press Enter to add)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}

fn truncate_text(text: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthStr;

    if text.width() <= max_width {
        return text.to_string();
    }

    let ellipsis = "…";
    let target_width = max_width.saturating_sub(1); // Room for ellipsis

    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let ch_width = ch.to_string().width();
        if current_width + ch_width > target_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }

    format!("{result}{ellipsis}")
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthStr;

    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_inclusive(' ') {
        let word_width = word.width();

        if current_width + word_width <= max_width {
            current_line.push_str(word);
            current_width += word_width;
        } else if current_line.is_empty() {
            // Word is longer than max_width, must break it by character
            for ch in word.chars() {
                let ch_width = ch.to_string().width();
                if current_width + ch_width > max_width && !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::new();
                    current_width = 0;
                }
                current_line.push(ch);
                current_width += ch_width;
            }
        } else {
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() || lines.is_empty() {
        lines.push(current_line);
    }

    lines
}

pub fn render_footer(app: &App) -> RatatuiLine<'static> {
    match (&app.view, &app.input_mode) {
        (_, InputMode::Command) => RatatuiLine::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow)),
            Span::raw(app.command_buffer.clone()),
            Span::styled("█", Style::default().fg(Color::White)),
        ]),
        (_, InputMode::QueryInput) => {
            let buffer = match &app.view {
                ViewMode::Filter(state) => state.query_buffer.clone(),
                ViewMode::Daily(_) => app.command_buffer.clone(),
            };
            RatatuiLine::from(vec![
                Span::styled("/", Style::default().fg(Color::Magenta)),
                Span::raw(buffer),
                Span::styled("█", Style::default().fg(Color::White)),
            ])
        }
        (_, InputMode::Edit(_)) => RatatuiLine::from(vec![
            Span::styled(" EDIT ", Style::default().fg(Color::Black).bg(Color::Green)),
            Span::styled("  Enter", Style::default().fg(Color::Gray)),
            Span::styled(" Save  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(Color::Gray)),
            Span::styled(" Save and new  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Shift+Tab", Style::default().fg(Color::Gray)),
            Span::styled(" Toggle entry type  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Gray)),
            Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
        ]),
        (_, InputMode::Order) => RatatuiLine::from(vec![
            Span::styled(
                " MOVE ",
                Style::default().fg(Color::Black).bg(Color::Yellow),
            ),
            Span::styled("  j/k|↕", Style::default().fg(Color::Gray)),
            Span::styled(" Move down/up  ", Style::default().fg(Color::DarkGray)),
            Span::styled("m/Enter", Style::default().fg(Color::Gray)),
            Span::styled(" Save  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Gray)),
            Span::styled(" Cancel", Style::default().fg(Color::DarkGray)),
        ]),
        (ViewMode::Daily(_), InputMode::Normal) => RatatuiLine::from(vec![
            Span::styled(" DAILY ", Style::default().fg(Color::Black).bg(Color::Cyan)),
            Span::styled("  Enter", Style::default().fg(Color::Gray)),
            Span::styled(" New entry  ", Style::default().fg(Color::DarkGray)),
            Span::styled("e", Style::default().fg(Color::Gray)),
            Span::styled(" Edit entry  ", Style::default().fg(Color::DarkGray)),
            Span::styled("x", Style::default().fg(Color::Gray)),
            Span::styled(" Toggle task  ", Style::default().fg(Color::DarkGray)),
            Span::styled("/", Style::default().fg(Color::Gray)),
            Span::styled(" Filter  ", Style::default().fg(Color::DarkGray)),
            Span::styled("?", Style::default().fg(Color::Gray)),
            Span::styled(" Help", Style::default().fg(Color::DarkGray)),
        ]),
        (ViewMode::Filter(_), InputMode::Normal) => RatatuiLine::from(vec![
            Span::styled(
                " FILTER ",
                Style::default().fg(Color::Black).bg(Color::Magenta),
            ),
            Span::styled("  x", Style::default().fg(Color::Gray)),
            Span::styled(" Toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled("d", Style::default().fg(Color::Gray)),
            Span::styled(" Delete  ", Style::default().fg(Color::DarkGray)),
            Span::styled("r", Style::default().fg(Color::Gray)),
            Span::styled(" Refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("v", Style::default().fg(Color::Gray)),
            Span::styled(" View day  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Gray)),
            Span::styled(" Exit  ", Style::default().fg(Color::DarkGray)),
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

static HELP_LINES: LazyLock<Vec<RatatuiLine<'static>>> = LazyLock::new(build_help_lines);

#[allow(clippy::vec_init_then_push)]
fn build_help_lines() -> Vec<RatatuiLine<'static>> {
    let header_style = Style::default().fg(Color::Cyan);
    let key_style = Style::default().fg(Color::Yellow);
    let desc_style = Style::default().fg(Color::White);

    let mut lines = Vec::new();

    // Daily mode
    lines.push(
        RatatuiLine::from(Span::styled("[Daily]", header_style)).alignment(Alignment::Center),
    );
    lines.push(help_line(
        "Enter",
        "New entry at end",
        key_style,
        desc_style,
    ));
    lines.push(help_line("o", "New entry below", key_style, desc_style));
    lines.push(help_line("e", "Edit selected", key_style, desc_style));
    lines.push(help_line(
        "x",
        "Toggle task complete",
        key_style,
        desc_style,
    ));
    lines.push(help_line("d", "Delete entry", key_style, desc_style));
    lines.push(help_line("y", "Yank to clipboard", key_style, desc_style));
    lines.push(help_line("u", "Undo delete", key_style, desc_style));
    lines.push(help_line("j/k", "Navigate down/up", key_style, desc_style));
    lines.push(help_line(
        "g/G",
        "Jump to first/last",
        key_style,
        desc_style,
    ));
    lines.push(help_line(
        "h/l|[]",
        "Previous/next day",
        key_style,
        desc_style,
    ));
    lines.push(help_line("t", "Go to today", key_style, desc_style));
    lines.push(help_line("s", "Sort entries", key_style, desc_style));
    lines.push(help_line("m", "Move mode", key_style, desc_style));
    lines.push(help_line("/", "Filter mode", key_style, desc_style));
    lines.push(help_line(
        "0-9",
        "Filter favorite tag",
        key_style,
        desc_style,
    ));
    lines.push(help_line(":", "Command mode", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Move mode
    lines
        .push(RatatuiLine::from(Span::styled("[Move]", header_style)).alignment(Alignment::Center));
    lines.push(help_line(
        "j/k|↕",
        "Move entry down/up",
        key_style,
        desc_style,
    ));
    lines.push(help_line("m/Enter", "Save", key_style, desc_style));
    lines.push(help_line("Esc", "Cancel", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Edit mode
    lines
        .push(RatatuiLine::from(Span::styled("[Edit]", header_style)).alignment(Alignment::Center));
    lines.push(help_line("Enter", "Save and exit", key_style, desc_style));
    lines.push(help_line("Tab", "Save and new", key_style, desc_style));
    lines.push(help_line(
        "Shift+Tab",
        "Toggle entry type",
        key_style,
        desc_style,
    ));
    lines.push(help_line("←/→", "Move cursor", key_style, desc_style));
    lines.push(help_line("Esc", "Cancel", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Filter mode
    lines.push(
        RatatuiLine::from(Span::styled("[Filter]", header_style)).alignment(Alignment::Center),
    );
    lines.push(help_line(
        "j/k|↕",
        "Navigate down/up",
        key_style,
        desc_style,
    ));
    lines.push(help_line("g/G", "Jump first/last", key_style, desc_style));
    lines.push(help_line(
        "Enter",
        "Quick add to today",
        key_style,
        desc_style,
    ));
    lines.push(help_line("e", "Edit entry", key_style, desc_style));
    lines.push(help_line("x", "Toggle task", key_style, desc_style));
    lines.push(help_line("d", "Delete entry", key_style, desc_style));
    lines.push(help_line("y", "Yank to clipboard", key_style, desc_style));
    lines.push(help_line("r", "Refresh results", key_style, desc_style));
    lines.push(help_line("v", "View day", key_style, desc_style));
    lines.push(help_line("/", "Edit filter", key_style, desc_style));
    lines.push(help_line("Esc", "Exit to daily", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Filter syntax
    lines.push(
        RatatuiLine::from(Span::styled("[Filter Syntax]", header_style))
            .alignment(Alignment::Center),
    );
    lines.push(help_line(
        "!tasks",
        "Incomplete tasks",
        key_style,
        desc_style,
    ));
    lines.push(help_line(
        "!tasks/done",
        "Completed tasks",
        key_style,
        desc_style,
    ));
    lines.push(help_line("!notes", "Notes only", key_style, desc_style));
    lines.push(help_line("!events", "Events only", key_style, desc_style));
    lines.push(help_line("#tag", "Filter by tag", key_style, desc_style));
    lines.push(help_line("$name", "Saved filter", key_style, desc_style));
    lines.push(help_line(
        "@before:DATE",
        "Before date",
        key_style,
        desc_style,
    ));
    lines.push(help_line(
        "@after:DATE",
        "After date",
        key_style,
        desc_style,
    ));
    lines.push(help_line(
        "@overdue",
        "Has past @date",
        key_style,
        desc_style,
    ));
    lines.push(RatatuiLine::from(Span::styled(
        "  DATE: MM/DD, tomorrow, yesterday, next-mon, last-fri, 3d, -3d",
        desc_style,
    )));
    lines.push(RatatuiLine::from(""));

    // Commands
    lines.push(
        RatatuiLine::from(Span::styled("[Commands]", header_style)).alignment(Alignment::Center),
    );
    lines.push(help_line(
        ":[g]oto",
        "Go to date (MM/DD, MM/DD/YY, etc.)",
        key_style,
        desc_style,
    ));
    lines.push(help_line(
        ":[o]pen",
        "Open journal file",
        key_style,
        desc_style,
    ));
    lines.push(help_line(
        ":config-reload",
        "Reload config file",
        key_style,
        desc_style,
    ));
    lines.push(help_line(":[q]uit", "Quit", key_style, desc_style));

    lines
}

fn help_line(key: &str, desc: &str, key_style: Style, desc_style: Style) -> RatatuiLine<'static> {
    RatatuiLine::from(vec![
        Span::styled(format!("{key:>8}  "), key_style),
        Span::styled(desc.to_string(), desc_style),
    ])
}

pub fn get_help_total_lines() -> usize {
    HELP_LINES.len()
}

pub fn render_help_content(scroll: usize, visible_height: usize) -> Vec<RatatuiLine<'static>> {
    HELP_LINES
        .iter()
        .skip(scroll)
        .take(visible_height)
        .cloned()
        .collect()
}
