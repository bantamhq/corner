use chrono::Timelike;
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line as RatatuiLine, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, EditContext, InputMode, ViewMode};
use crate::calendar::CalendarEvent;
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

    let calendar_events = app.calendar_store.events_for_date(app.current_date);
    let show_calendar_name = app.calendar_store.visible_calendar_count > 1;
    let calendar_event_count = calendar_events.len();

    for event in calendar_events {
        let line = render_calendar_event(event, width, show_calendar_name);
        lines.push(line);
    }

    let mut visible_projected_idx = 0;

    for projected_entry in &state.projected_entries {
        let is_completed = matches!(
            projected_entry.entry_type,
            EntryType::Task { completed: true }
        );
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
        let indicator = get_projected_entry_indicator(
            app,
            is_selected,
            visible_idx,
            &projected_entry.source_type,
        );

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
                    let mut spans = if i == 0 {
                        vec![Span::styled(prefix.to_string(), content_style)]
                    } else {
                        vec![Span::styled(" ".repeat(prefix_width), content_style)]
                    };
                    spans.extend(style_content(line_text, content_style));
                    lines.push(RatatuiLine::from(spans));
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

    if calendar_event_count == 0 && visible_projected_idx == 0 && visible_entry_idx == 0 {
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

fn render_calendar_event(
    event: &CalendarEvent,
    width: usize,
    show_calendar_name: bool,
) -> RatatuiLine<'static> {
    let prefix = "* ";
    let prefix_width = prefix.width();
    let indicator = "○";

    let content = format_calendar_event(event, show_calendar_name);
    let available = width.saturating_sub(prefix_width);
    let display_text = truncate_with_tags(&content, available);

    let content_style = if event.is_cancelled || event.is_declined {
        Style::default().italic().crossed_out()
    } else {
        Style::default().italic()
    };
    let rest_of_prefix: String = prefix.chars().skip(1).collect();

    let mut spans = vec![
        Span::styled(indicator.to_string(), Style::default().fg(Color::Blue)),
        Span::styled(rest_of_prefix, content_style),
    ];
    spans.extend(style_content(&display_text, content_style));

    RatatuiLine::from(spans)
}

#[must_use]
fn format_calendar_event(event: &CalendarEvent, show_calendar_name: bool) -> String {
    let mut parts = vec![event.title.clone()];

    if let Some((day, total)) = event.multi_day_info {
        parts.push(format!("{day}/{total}"));
    }

    if !event.is_all_day {
        let start_hour = event.start.hour();
        let end_hour = event.end.hour();
        let same_period = (start_hour < 12) == (end_hour < 12);

        let time_str = if same_period {
            let start_time = event.start.format("%-I:%M").to_string();
            let end_time = event.end.format("%-I:%M%P").to_string();
            format!("{start_time}-{end_time}")
        } else {
            let start_time = event.start.format("%-I:%M%P").to_string();
            let end_time = event.end.format("%-I:%M%P").to_string();
            format!("{start_time}-{end_time}")
        };
        parts.push(time_str);
    }

    if show_calendar_name {
        parts.push(format!("({})", event.calendar_name));
    }

    if parts.len() == 1 {
        parts.into_iter().next().unwrap()
    } else if show_calendar_name && parts.len() > 1 {
        let last = parts.pop().unwrap();
        format!("{} {last}", parts.join(" - "))
    } else {
        parts.join(" - ")
    }
}

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
        SourceType::Calendar { .. } => "○",
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
            Span::styled("↕", Style::default().fg(Color::Green))
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
