use crate::app::{App, DATE_SUFFIX_WIDTH, EditContext, InputMode, ViewMode};
use crate::cursor::{CursorBuffer, cursor_position_in_wrap};
use crate::storage::Line;
use unicode_width::UnicodeWidthStr;

use super::context::RenderContext;
use super::scroll::{CursorContext, ensure_line_visible, ensure_selected_visible};
use super::views::{
    list_content_height_for_daily, list_content_height_for_filter, list_content_width_for_daily,
    list_content_width_for_filter,
};

pub struct RenderPrep {
    pub edit_cursor: Option<CursorContext>,
}

fn build_cursor_context(
    buffer: &CursorBuffer,
    prefix_width: usize,
    available_width: usize,
    entry_start_line: usize,
) -> CursorContext {
    let text_width = available_width.saturating_sub(prefix_width);
    let wrap_width = text_width.saturating_sub(1).max(1);
    let (cursor_row, cursor_col) =
        cursor_position_in_wrap(buffer.content(), buffer.cursor_display_pos(), wrap_width);
    CursorContext {
        prefix_width,
        cursor_row,
        cursor_col,
        entry_start_line,
    }
}

pub fn prepare_render(app: &mut App, layout: &RenderContext) -> RenderPrep {
    let filter_visual_line = app.filter_visual_line();
    let filter_total_lines = app.filter_total_lines();
    let visible_entry_count = app.visible_entry_count();
    let calendar_event_count = app.calendar_event_count();

    match &mut app.view {
        ViewMode::Filter(state) => {
            let scroll_height = list_content_height_for_filter(layout);
            ensure_selected_visible(
                &mut state.scroll_offset,
                filter_visual_line,
                filter_total_lines,
                scroll_height,
            );
            if state.selected == 0 {
                state.scroll_offset = 0;
            }
        }
        ViewMode::Daily(state) => {
            let scroll_height = list_content_height_for_daily(layout);
            ensure_selected_visible(
                &mut state.scroll_offset,
                state.selected + calendar_event_count,
                visible_entry_count + calendar_event_count,
                scroll_height,
            );
            if state.selected == 0 {
                state.scroll_offset = 0;
            }
        }
    }

    let edit_cursor = if let InputMode::Edit(ref ctx) = app.input_mode
        && let Some(ref buffer) = app.edit_buffer
    {
        match ctx {
            EditContext::FilterQuickAdd { entry_type, .. } => {
                let ViewMode::Filter(state) = &app.view else {
                    unreachable!()
                };
                let prefix_width = entry_type.prefix().len();
                let available_width = list_content_width_for_filter(layout);
                Some(build_cursor_context(
                    buffer,
                    prefix_width,
                    available_width,
                    state.entries.len(),
                ))
            }
            EditContext::FilterEdit { filter_index, .. } => {
                let ViewMode::Filter(state) = &app.view else {
                    unreachable!()
                };
                state.entries.get(*filter_index).map(|filter_entry| {
                    let prefix_width = filter_entry.entry_type.prefix().len();
                    let available_width =
                        list_content_width_for_filter(layout).saturating_sub(DATE_SUFFIX_WIDTH);
                    build_cursor_context(buffer, prefix_width, available_width, *filter_index)
                })
            }
            EditContext::Daily { entry_index } => app
                .entry_indices
                .get(*entry_index)
                .and_then(|&i| {
                    if let Line::Entry(entry) = &app.lines[i] {
                        Some(&entry.entry_type)
                    } else {
                        None
                    }
                })
                .map(|entry_type| {
                    let prefix_width = entry_type.prefix().width();
                    let available_width = list_content_width_for_daily(layout);
                    let entry_start_line = calendar_event_count
                        + app.visible_projected_count()
                        + app.visible_entries_before(*entry_index);
                    build_cursor_context(buffer, prefix_width, available_width, entry_start_line)
                }),
            EditContext::LaterEdit { .. } => {
                if let ViewMode::Daily(state) = &app.view {
                    state.projected_entries.get(state.selected).map(|entry| {
                        let prefix_width = entry.entry_type.prefix().len();
                        let available_width = list_content_width_for_daily(layout);
                        let entry_start_line = calendar_event_count + state.selected;
                        build_cursor_context(
                            buffer,
                            prefix_width,
                            available_width,
                            entry_start_line,
                        )
                    })
                } else {
                    None
                }
            }
        }
    } else {
        None
    };

    if let Some(ref cursor) = edit_cursor {
        let cursor_line = cursor.entry_start_line + cursor.cursor_row;
        match &mut app.view {
            ViewMode::Filter(state) => {
                let scroll_height = list_content_height_for_filter(layout);
                ensure_line_visible(&mut state.scroll_offset, cursor_line, scroll_height);
            }
            ViewMode::Daily(state) => {
                let scroll_height = list_content_height_for_daily(layout);
                ensure_line_visible(&mut state.scroll_offset, cursor_line, scroll_height);
            }
        }
    }

    RenderPrep { edit_cursor }
}
