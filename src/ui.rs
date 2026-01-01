use once_cell::sync::Lazy;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line as RatatuiLine, Span},
};
use regex::Regex;

use crate::app::{App, Mode};
use crate::storage::{EntryType, Line};

static TAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"#([a-zA-Z][a-zA-Z0-9_-]*)").unwrap());
static LATER_DATE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"@(\d{1,2}/\d{1,2})").unwrap());

fn style_content(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut last_end = 0;

    let mut matches: Vec<(usize, usize, Color)> = Vec::new();

    for cap in TAG_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), Color::Yellow));
        }
    }

    for cap in LATER_DATE_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), Color::Red));
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

    let mut lines = Vec::new();

    let header = format!("Filter: {}", app.filter_query);
    lines.push(RatatuiLine::from(Span::styled(
        header,
        Style::default().fg(Color::Cyan),
    )));

    let is_editing = app.mode == Mode::Edit;

    for (idx, item) in app.filter_items.iter().enumerate() {
        let is_selected = idx == app.filter_selected;
        let is_editing_this = is_selected && is_editing;

        let content_style = if item.completed {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        let text = if is_editing_this {
            if let Some(ref buffer) = app.edit_buffer {
                buffer.content().to_string()
            } else {
                item.content.clone()
            }
        } else {
            item.content.clone()
        };

        let prefix = match &item.entry_type {
            EntryType::Task { completed: false } => "- [ ] ",
            EntryType::Task { completed: true } => "- [x] ",
            EntryType::Note => "- ",
            EntryType::Event => "* ",
        };
        let prefix_width = prefix.width();

        let date_suffix = format!(" ({})", item.source_date.format("%m/%d"));
        let date_suffix_width = date_suffix.width();

        if is_selected {
            if is_editing_this {
                let available = width.saturating_sub(prefix_width + date_suffix_width);
                let wrapped = wrap_text(&text, available);
                for (i, line_text) in wrapped.iter().enumerate() {
                    if i == 0 {
                        let mut spans = vec![Span::styled(prefix.to_string(), content_style)];
                        spans.push(Span::styled(line_text.clone(), content_style));
                        spans.push(Span::styled(date_suffix.clone(), Style::default().fg(Color::DarkGray)));
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
                let sel_prefix = match &item.entry_type {
                    EntryType::Task { completed: false } => " [ ] ",
                    EntryType::Task { completed: true } => " [x] ",
                    EntryType::Note => " ",
                    EntryType::Event => " ",
                };
                let available = width.saturating_sub(prefix_width + date_suffix_width);
                let display_text = truncate_text(&text, available);
                let mut spans = vec![Span::styled("→", Style::default().fg(Color::Cyan))];
                spans.push(Span::styled(sel_prefix.to_string(), content_style));
                spans.extend(style_content(&display_text, content_style));
                spans.push(Span::styled(date_suffix, Style::default().fg(Color::DarkGray)));
                lines.push(RatatuiLine::from(spans));
            }
        } else {
            let available = width.saturating_sub(prefix_width + date_suffix_width);
            let display_text = truncate_text(&text, available);
            let mut spans = vec![Span::styled(prefix.to_string(), content_style)];
            spans.extend(style_content(&display_text, content_style));
            spans.push(Span::styled(date_suffix, Style::default().fg(Color::DarkGray)));
            lines.push(RatatuiLine::from(spans));
        }
    }

    if app.filter_items.is_empty() {
        lines.push(RatatuiLine::from(Span::styled(
            "(no matches)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines
}

pub fn render_daily_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
    use unicode_width::UnicodeWidthStr;

    let mut lines = Vec::new();

    let date_header = app.current_date.format("%m/%d/%y").to_string();
    lines.push(RatatuiLine::from(Span::styled(
        date_header,
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

            let prefix = entry.prefix();
            let prefix_width = prefix.width();

            if is_editing {
                // Wrap editing entry to multiple lines
                let wrapped = wrap_text(&text, width.saturating_sub(prefix_width));
                for (i, line_text) in wrapped.iter().enumerate() {
                    if i == 0 {
                        lines.push(RatatuiLine::from(Span::styled(
                            format!("{prefix}{line_text}"),
                            content_style,
                        )));
                    } else {
                        // Continuation lines: indent to align with content
                        let indent = " ".repeat(prefix_width);
                        lines.push(RatatuiLine::from(Span::styled(
                            format!("{indent}{line_text}"),
                            content_style,
                        )));
                    }
                }
            } else if is_selected {
                let rest_of_prefix = prefix.chars().skip(1).collect::<String>();
                let indicator = if app.mode == Mode::Order {
                    Span::styled("↕", Style::default().fg(Color::Yellow))
                } else {
                    Span::styled("→", Style::default().fg(Color::Cyan))
                };
                let available = width.saturating_sub(prefix_width);
                let display_text = truncate_text(&text, available);
                let mut spans = vec![indicator, Span::styled(rest_of_prefix, content_style)];
                spans.extend(style_content(&display_text, content_style));
                lines.push(RatatuiLine::from(spans));
            } else {
                let available = width.saturating_sub(prefix_width);
                let display_text = truncate_text(&text, available);
                let mut spans = vec![Span::styled(prefix.to_string(), content_style)];
                spans.extend(style_content(&display_text, content_style));
                lines.push(RatatuiLine::from(spans));
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
            // Start new line with this word
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
    match app.mode {
        Mode::Command => RatatuiLine::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow)),
            Span::raw(app.command_buffer.clone()),
            Span::styled("█", Style::default().fg(Color::White)),
        ]),
        Mode::FilterInput => RatatuiLine::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(app.filter_buffer.clone()),
            Span::styled("█", Style::default().fg(Color::White)),
        ]),
        Mode::Edit => RatatuiLine::from(vec![
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
        Mode::Daily => RatatuiLine::from(vec![
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
        Mode::Filter => RatatuiLine::from(vec![
            Span::styled(
                " FILTER ",
                Style::default().fg(Color::Black).bg(Color::Magenta),
            ),
            Span::styled("  x", Style::default().fg(Color::Gray)),
            Span::styled(" Toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled("r", Style::default().fg(Color::Gray)),
            Span::styled(" Refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Gray)),
            Span::styled(" Go to day  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Gray)),
            Span::styled(" Exit  ", Style::default().fg(Color::DarkGray)),
            Span::styled("?", Style::default().fg(Color::Gray)),
            Span::styled(" Help", Style::default().fg(Color::DarkGray)),
        ]),
        Mode::Order => RatatuiLine::from(vec![
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
    lines.push(help_line(
        "s",
        "Sort completed to top",
        key_style,
        desc_style,
    ));
    lines.push(help_line("m", "Move mode", key_style, desc_style));
    lines.push(help_line("/", "Filter mode", key_style, desc_style));
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
    lines.push(help_line("e", "Edit entry", key_style, desc_style));
    lines.push(help_line("x", "Toggle task", key_style, desc_style));
    lines.push(help_line("r", "Refresh results", key_style, desc_style));
    lines.push(help_line("Enter", "Go to day", key_style, desc_style));
    lines.push(help_line("/", "Edit filter", key_style, desc_style));
    lines.push(help_line("Esc", "Exit to daily", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Filter syntax
    lines.push(
        RatatuiLine::from(Span::styled("[Filter Syntax]", header_style))
            .alignment(Alignment::Center),
    );
    lines.push(help_line("!tasks", "Incomplete tasks", key_style, desc_style));
    lines.push(help_line(
        "!tasks/done",
        "Completed tasks",
        key_style,
        desc_style,
    ));
    lines.push(help_line("!notes", "Notes only", key_style, desc_style));
    lines.push(help_line("!events", "Events only", key_style, desc_style));
    lines.push(help_line("#tag", "Filter by tag", key_style, desc_style));
    lines.push(RatatuiLine::from(""));

    // Commands
    lines.push(
        RatatuiLine::from(Span::styled("[Commands]", header_style)).alignment(Alignment::Center),
    );
    lines.push(help_line(
        ":goto|:gt",
        "Go to date (YYYY/MM/DD or MM/DD)",
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
