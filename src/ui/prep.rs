use crate::app::{
    App, DAILY_HEADER_LINES, DATE_SUFFIX_WIDTH, EditContext, FILTER_HEADER_LINES, InputMode,
    InterfaceContext, PromptContext, ViewMode,
};
use crate::cursor::cursor_position_in_wrap;
use crate::storage::Line;
use unicode_width::UnicodeWidthStr;

use super::context::RenderContext;
use super::scroll::{CursorContext, ensure_selected_visible};

pub struct RenderPrep {
    pub edit_cursor: Option<CursorContext>,
    pub prompt_cursor: Option<(u16, u16)>,
}

/// Prepares render state and mutates view scroll offsets for visibility.
pub fn prepare_render(app: &mut App, layout: &RenderContext) -> RenderPrep {
    let filter_visual_line = app.filter_visual_line();
    let filter_total_lines = app.filter_total_lines();
    let visible_entry_count = app.visible_entry_count();
    let calendar_event_count = app.calendar_event_count();

    match &mut app.view {
        ViewMode::Filter(state) => {
            ensure_selected_visible(
                &mut state.scroll_offset,
                filter_visual_line,
                filter_total_lines,
                layout.scroll_height,
            );
            if state.selected == 0 {
                state.scroll_offset = 0;
            }
        }
        ViewMode::Daily(state) => {
            let fixed_lines = DAILY_HEADER_LINES + calendar_event_count;
            ensure_selected_visible(
                &mut state.scroll_offset,
                state.selected + fixed_lines,
                visible_entry_count + fixed_lines,
                layout.scroll_height,
            );
            if state.selected == 0 {
                state.scroll_offset = 0;
            }
        }
    }

    if app.help_visible {
        app.help_visible_height = layout.help_visible_height;
    }

    if let InputMode::Interface(ref mut ctx) = app.input_mode {
        match ctx {
            InterfaceContext::Date(_) => {}
            InterfaceContext::Project(state) => {
                prep_interface_scroll(
                    &mut state.scroll_offset,
                    state.selected,
                    state.projects.len(),
                    layout.interface_visible_height,
                );
            }
            InterfaceContext::Tag(state) => {
                prep_interface_scroll(
                    &mut state.scroll_offset,
                    state.selected,
                    state.tags.len(),
                    layout.interface_visible_height,
                );
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
                let text_width = layout.content_width.saturating_sub(prefix_width);
                let (cursor_row, cursor_col) = cursor_position_in_wrap(
                    buffer.content(),
                    buffer.cursor_display_pos(),
                    text_width,
                );
                Some(CursorContext {
                    prefix_width,
                    cursor_row,
                    cursor_col,
                    entry_start_line: state.entries.len() + FILTER_HEADER_LINES,
                })
            }
            EditContext::FilterEdit { filter_index, .. } => {
                let ViewMode::Filter(state) = &app.view else {
                    unreachable!()
                };
                state.entries.get(*filter_index).map(|filter_entry| {
                    let prefix_width = filter_entry.entry_type.prefix().len();
                    let text_width = layout
                        .content_width
                        .saturating_sub(prefix_width + DATE_SUFFIX_WIDTH);
                    let (cursor_row, cursor_col) = cursor_position_in_wrap(
                        buffer.content(),
                        buffer.cursor_display_pos(),
                        text_width,
                    );
                    CursorContext {
                        prefix_width,
                        cursor_row,
                        cursor_col,
                        entry_start_line: *filter_index + FILTER_HEADER_LINES,
                    }
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
                    let text_width = layout.content_width.saturating_sub(prefix_width);
                    let (cursor_row, cursor_col) = cursor_position_in_wrap(
                        buffer.content(),
                        buffer.cursor_display_pos(),
                        text_width,
                    );
                    CursorContext {
                        prefix_width,
                        cursor_row,
                        cursor_col,
                        entry_start_line: app.visible_projected_count()
                            + app.visible_entries_before(*entry_index)
                            + DAILY_HEADER_LINES,
                    }
                }),
        }
    } else {
        None
    };

    let prompt_cursor = if let InputMode::Prompt(ref ctx) = app.input_mode {
        let (prefix_width, cursor_pos) = prompt_cursor_offsets(ctx);
        let cursor_x = layout.footer_area.x + prefix_width + cursor_pos as u16;
        let cursor_y = layout.footer_area.y;
        Some((cursor_x, cursor_y))
    } else {
        None
    };

    RenderPrep {
        edit_cursor,
        prompt_cursor,
    }
}

fn prompt_cursor_offsets(ctx: &PromptContext) -> (u16, usize) {
    match ctx {
        PromptContext::Command { buffer } | PromptContext::Filter { buffer } => {
            (1, buffer.cursor_display_pos())
        }
        PromptContext::RenameTag { old_tag, buffer } => {
            let rename_prefix_width = "Rename #".len() + old_tag.len() + " to: ".len();
            (rename_prefix_width as u16, buffer.cursor_display_pos())
        }
    }
}

fn prep_interface_scroll(
    scroll_offset: &mut usize,
    selected: usize,
    total: usize,
    visible_height: usize,
) {
    ensure_selected_visible(scroll_offset, selected, total, visible_height);
}
