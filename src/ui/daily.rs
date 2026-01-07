use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line as RatatuiLine, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, EditContext, InputMode, ViewMode};
use crate::storage::{EntryType, Line, SourceType};

use super::shared::{
    date_suffix_style, entry_style, format_date_suffix, style_content, truncate_with_tags,
    wrap_text,
};

pub fn render_daily_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
    let ViewMode::Daily(state) = &app.view else {
        return vec![];
    };

    let mut lines = Vec::new();

    let date_header = app
        .current_date
        .format(&app.config.header_date_format)
        .to_string();
    let hidden_count = app.hidden_completed_count();
    if app.hide_completed && hidden_count > 0 {
        lines.push(RatatuiLine::from(vec![
            Span::styled(date_header, Style::default().fg(Color::Cyan)),
            Span::styled(
                format!(" (Hiding {hidden_count} completed)"),
                Style::default().dim(),
            ),
        ]));
    } else {
        lines.push(RatatuiLine::from(Span::styled(
            date_header,
            Style::default().fg(Color::Cyan),
        )));
    }

    let mut visible_projected_idx = 0;

    for projected_entry in &state.projected_entries {
        let is_completed =
            matches!(projected_entry.entry_type, EntryType::Task { completed: true });
        if app.hide_completed && is_completed {
            continue;
        }

        let is_selected = visible_projected_idx == state.selected;
        visible_projected_idx += 1;

        let content_style = entry_style(&projected_entry.entry_type);
        let text = projected_entry.content.clone();
        let prefix = projected_entry.entry_type.prefix();
        let prefix_width = prefix.width();
        let (source_suffix, source_suffix_width) = format_date_suffix(projected_entry.source_date);

        let available = width.saturating_sub(prefix_width + source_suffix_width);
        let display_text = truncate_with_tags(&text, available);
        let rest_of_prefix: String = prefix.chars().skip(1).collect();

        let visible_idx = visible_projected_idx - 1;
        let indicator = get_projected_entry_indicator(app, is_selected, visible_idx, &projected_entry.source_type);

        let mut spans = vec![indicator, Span::styled(rest_of_prefix, content_style)];
        spans.extend(style_content(&display_text, content_style));
        spans.push(Span::styled(
            source_suffix,
            date_suffix_style(content_style),
        ));
        lines.push(RatatuiLine::from(spans));
    }

    let mut visible_entry_idx = 0;
    for &line_idx in &app.entry_indices {
        if let Line::Entry(entry) = &app.lines[line_idx] {
            let is_completed = matches!(entry.entry_type, EntryType::Task { completed: true });

            if app.hide_completed && is_completed {
                continue;
            }

            let selection_idx = visible_projected_idx + visible_entry_idx;
            visible_entry_idx += 1;
            let is_selected = selection_idx == state.selected;
            let is_editing =
                is_selected && matches!(app.input_mode, InputMode::Edit(EditContext::Daily { .. }));

            let content_style = entry_style(&entry.entry_type);

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
            } else {
                let first_char = prefix.chars().next().unwrap_or('-').to_string();
                let rest_of_prefix = prefix.chars().skip(1).collect::<String>();
                let indicator = get_entry_indicator(
                    app,
                    is_selected,
                    selection_idx,
                    Color::Cyan,
                    &first_char,
                    content_style,
                );
                let available = width.saturating_sub(prefix_width);
                let display_text = truncate_with_tags(&text, available);
                let mut spans = vec![indicator, Span::styled(rest_of_prefix, content_style)];
                spans.extend(style_content(&display_text, content_style));
                lines.push(RatatuiLine::from(spans));
            }
        }
    }

    if visible_projected_idx == 0 && visible_entry_idx == 0 {
        let has_hidden = app.hide_completed && app.hidden_completed_count() > 0;
        let message = if has_hidden {
            "(No visible entries - press z to show completed or Enter to add)"
        } else {
            "(No entries - press Enter to add)"
        };
        lines.push(RatatuiLine::from(Span::styled(
            message,
            Style::default().dim(),
        )));
    }

    lines
}

/// Determine the indicator character for a projected entry (↪ for Later, ↺ for Recurring)
fn get_projected_entry_indicator(
    app: &App,
    is_cursor: bool,
    visible_idx: usize,
    kind: &SourceType,
) -> Span<'static> {
    let is_selected_in_selection = if let InputMode::Selection(ref state) = app.input_mode {
        state.is_selected(visible_idx)
    } else {
        false
    };

    let indicator = match kind {
        SourceType::Later => "↪",
        SourceType::Recurring => "↺",
        SourceType::Local => unreachable!("projected entries are never Local"),
    };

    if is_cursor {
        if matches!(app.input_mode, InputMode::Selection(_)) {
            if is_selected_in_selection {
                Span::styled("◉", Style::default().fg(Color::Green))
            } else {
                Span::styled(indicator, Style::default().fg(Color::Cyan))
            }
        } else {
            Span::styled(indicator, Style::default().fg(Color::Cyan))
        }
    } else if is_selected_in_selection {
        Span::styled("○", Style::default().fg(Color::Green))
    } else {
        Span::styled(indicator, Style::default().fg(Color::Red))
    }
}

/// Determine the indicator character for a regular entry based on mode and selection state
fn get_entry_indicator(
    app: &App,
    is_cursor: bool,
    visible_idx: usize,
    cursor_color: Color,
    default_first_char: &str,
    default_style: Style,
) -> Span<'static> {
    let is_selected_in_selection = if let InputMode::Selection(ref state) = app.input_mode {
        state.is_selected(visible_idx)
    } else {
        false
    };

    if is_cursor {
        if matches!(app.input_mode, InputMode::Reorder) {
            Span::styled("↕", Style::default().fg(Color::Yellow))
        } else if matches!(app.input_mode, InputMode::Selection(_)) {
            if is_selected_in_selection {
                Span::styled("◉", Style::default().fg(Color::Green))
            } else {
                Span::styled("→", Style::default().fg(Color::Cyan))
            }
        } else {
            Span::styled("→", Style::default().fg(cursor_color))
        }
    } else if is_selected_in_selection {
        Span::styled("○", Style::default().fg(Color::Green))
    } else {
        Span::styled(default_first_char.to_string(), default_style)
    }
}
