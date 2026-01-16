use ratatui::style::{Style, Stylize};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, EditContext, InputMode, ViewMode};
use crate::storage::{EntryType, Line};

use super::helpers::edit_text;
use super::model::ListModel;
use super::rows;
use super::rows::build_edit_rows_with_prefix_width;
use super::shared::entry_style;

pub fn build_daily_list(app: &App, width: usize) -> ListModel {
    let ViewMode::Daily(state) = &app.view else {
        return ListModel::from_rows(None, Vec::new(), app.scroll_offset());
    };

    let mut rows = Vec::new();

    let calendar_events = app.calendar_store.events_for_date(app.current_date);
    let show_calendar_name = app.calendar_store.visible_calendar_count > 1;
    let mut calendar_event_count = 0;

    for event in calendar_events {
        let is_past = event.is_past();
        if app.hide_completed && is_past {
            continue;
        }
        calendar_event_count += 1;
        rows.push(rows::build_calendar_row(
            event,
            width,
            show_calendar_name,
            is_past,
        ));
    }

    let hidden_count = app.hidden_completed_count();
    if app.hide_completed && hidden_count > 0 {
        let message = format!("▼ [{hidden_count} hidden entries]");
        rows.push(rows::build_message_row(&message, Style::default().dim()));
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

        let visible_idx = visible_projected_idx - 1;
        rows.push(rows::build_projected_row(
            app,
            projected_entry,
            is_selected,
            visible_idx,
            width,
        ));
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

            let text = edit_text(app, is_editing, &entry.content);

            let prefix = entry.prefix();
            let prefix_width = prefix.width();

            if is_editing {
                let text_width = width.saturating_sub(prefix_width);
                rows.extend(build_edit_rows_with_prefix_width(
                    prefix,
                    prefix_width,
                    content_style,
                    &text,
                    text_width,
                    None,
                ));
            } else {
                rows.push(rows::build_daily_entry_row(
                    app,
                    entry,
                    is_selected,
                    selection_idx,
                    width,
                ));
            }
        }
    }

    // Show empty state only for truly empty days
    if calendar_event_count == 0
        && visible_projected_idx == 0
        && visible_entry_idx == 0
        && hidden_count == 0
    {
        rows.push(rows::build_message_row(
            "(No entries - press Enter to add)",
            Style::default().dim(),
        ));
    }

    ListModel::from_rows(None, rows, app.scroll_offset())
}
