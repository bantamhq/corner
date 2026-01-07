use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line as RatatuiLine, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, EditContext, InputMode, ViewMode};
use crate::storage::EntryType;

use super::shared::{
    date_suffix_style, entry_style, format_date_suffix, style_content, truncate_with_tags,
    wrap_text,
};

pub fn render_filter_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
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

        let content_style = entry_style(&filter_entry.entry_type);

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

        let (date_suffix, date_suffix_width) = format_date_suffix(filter_entry.source_date);

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
                            date_suffix_style(content_style),
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
                let display_text = truncate_with_tags(&text, available);

                let is_cursor_selected = if let InputMode::Selection(ref sel_state) = app.input_mode
                {
                    sel_state.is_selected(idx)
                } else {
                    false
                };
                let cursor_indicator = if is_cursor_selected {
                    Span::styled("◉", Style::default().fg(Color::Green))
                } else {
                    Span::styled("→", Style::default().fg(Color::Cyan))
                };
                let mut spans = vec![cursor_indicator];
                spans.push(Span::styled(sel_prefix.to_string(), content_style));
                spans.extend(style_content(&display_text, content_style));
                spans.push(Span::styled(date_suffix, date_suffix_style(content_style)));
                lines.push(RatatuiLine::from(spans));
            }
        } else {
            let is_selected_in_selection =
                if let InputMode::Selection(ref sel_state) = app.input_mode {
                    sel_state.is_selected(idx)
                } else {
                    false
                };

            let available = width.saturating_sub(prefix_width + date_suffix_width);
            let display_text = truncate_with_tags(&text, available);

            let first_char = if is_selected_in_selection {
                Span::styled("○", Style::default().fg(Color::Green))
            } else {
                Span::styled(
                    prefix.chars().next().unwrap_or('-').to_string(),
                    content_style,
                )
            };
            let rest_of_prefix: String = prefix.chars().skip(1).collect();

            let mut spans = vec![first_char, Span::styled(rest_of_prefix, content_style)];
            spans.extend(style_content(&display_text, content_style));
            spans.push(Span::styled(date_suffix, date_suffix_style(content_style)));
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
            Style::default().dim(),
        )));
    }

    lines
}
