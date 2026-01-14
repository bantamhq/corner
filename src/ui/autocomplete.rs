use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line as RatatuiLine, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, HintContext, HintItem};

use super::scroll::CursorContext;

const DROPDOWN_TEXT_WIDTH: usize = 15;

pub const MAX_SUGGESTIONS: usize = 5;

pub fn render_autocomplete_dropdown(
    f: &mut Frame<'_>,
    app: &App,
    cursor: &CursorContext,
    content_area: Rect,
) {
    if !app.hint_state.is_active() {
        return;
    }

    let items = app.hint_state.display_items("");
    if items.is_empty() {
        return;
    }

    let selected_index = app.hint_state.selected_index();
    let window_len = items.len().min(MAX_SUGGESTIONS);

    let cursor_line = cursor.entry_start_line + cursor.cursor_row;
    let scroll_offset = app.scroll_offset();
    if cursor_line < scroll_offset {
        return;
    }

    let screen_row = cursor_line - scroll_offset;
    let rows_below = content_area.height.saturating_sub(screen_row as u16 + 1);
    let show_above = rows_below < 6;
    let start_y = if show_above {
        (content_area.y + screen_row as u16).saturating_sub(1)
    } else {
        content_area.y + screen_row as u16 + 1
    };
    if start_y >= content_area.y + content_area.height {
        return;
    }

    let cursor_x = content_area.x + (cursor.prefix_width + cursor.cursor_col) as u16;
    let token_len = token_display_len(&app.hint_state) as u16;
    let start_x = cursor_x.saturating_sub(token_len + 1).max(content_area.x);
    if start_x >= content_area.x + content_area.width {
        return;
    }

    let mut width = (DROPDOWN_TEXT_WIDTH + 2) as u16;
    let available_width = content_area.width - (start_x - content_area.x);
    if width > available_width {
        width = available_width.max(1);
    }

    let mut height = (window_len as u16).saturating_add(2);
    let available_height = if show_above {
        start_y.saturating_sub(content_area.y) + 1
    } else {
        content_area.height - (start_y - content_area.y)
    };
    if height > available_height {
        height = available_height.max(1);
    }

    let start_y = if show_above {
        start_y.saturating_sub(height.saturating_sub(1))
    } else {
        start_y
    };

    let area = Rect {
        x: start_x,
        y: start_y,
        width,
        height,
    };

    let text_width = width.saturating_sub(2).max(1) as usize;
    let lines = build_dropdown_lines(&items, selected_index, app.hint_state.color(), text_width);
    render_dropdown_box(f, area, lines);
}

pub fn token_display_len(hint: &HintContext) -> usize {
    match hint {
        HintContext::Tags { prefix, .. } => 1 + prefix.width(),
        HintContext::Commands { prefix, .. } => 1 + prefix.width(),
        HintContext::FilterTypes { prefix, .. } => 1 + prefix.width(),
        HintContext::DateOps { prefix, .. } => 1 + prefix.width(),
        HintContext::DateValues { prefix, .. } => 1 + prefix.width(),
        HintContext::SavedFilters { prefix, .. } => 1 + prefix.width(),
        HintContext::Negation { inner } => 4 + token_display_len(inner),
        HintContext::Inactive | HintContext::GuidanceMessage { .. } => 1,
    }
}

pub fn truncate_item(item: &str, width: usize) -> String {
    if item.width() <= width {
        return item.to_string();
    }

    let mut result = String::new();
    let mut current_width = 0;
    for ch in item.chars() {
        let ch_width = ch.to_string().width();
        if current_width + ch_width >= width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }
    format!("{result}…")
}

pub fn build_dropdown_lines(
    items: &[HintItem],
    selected_index: usize,
    highlight_color: ratatui::style::Color,
    text_width: usize,
) -> Vec<RatatuiLine<'static>> {
    let window_start = selected_index.saturating_sub(selected_index % MAX_SUGGESTIONS);
    let window_end = (window_start + MAX_SUGGESTIONS).min(items.len());
    let window = &items[window_start..window_end];

    window
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let is_selected = window_start + index == selected_index;
            let mut style = Style::default().fg(highlight_color);
            if !item.selectable {
                style = style.dim();
            }
            if is_selected {
                style = style.reversed();
            }
            let truncated = truncate_item(&item.label, text_width);
            RatatuiLine::from(Span::styled(truncated, style))
        })
        .collect()
}

pub fn render_dropdown_box(f: &mut Frame<'_>, area: Rect, lines: Vec<RatatuiLine<'static>>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);
    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}
