use ratatui::layout::Rect;

/// Ensures the selected item is visible within the scroll viewport.
pub fn ensure_selected_visible(
    scroll_offset: &mut usize,
    selected: usize,
    entry_count: usize,
    visible_height: usize,
) {
    if entry_count == 0 {
        *scroll_offset = 0;
        return;
    }
    if selected < *scroll_offset {
        *scroll_offset = selected;
    }
    if selected >= *scroll_offset + visible_height {
        *scroll_offset = selected - visible_height + 1;
    }

    let max_scroll = entry_count.saturating_sub(visible_height);
    if *scroll_offset > max_scroll {
        *scroll_offset = max_scroll;
    }
}

/// Ensures a specific line (e.g., cursor position) is visible within the scroll viewport.
pub fn ensure_line_visible(scroll_offset: &mut usize, line: usize, visible_height: usize) {
    if line >= *scroll_offset + visible_height {
        *scroll_offset = line - visible_height + 1;
    }
    let min_scroll = line.saturating_sub(visible_height.saturating_sub(1));
    if *scroll_offset > min_scroll {
        *scroll_offset = min_scroll;
    }
}

/// Context for positioning the cursor during text editing.
pub struct CursorContext {
    pub prefix_width: usize,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub entry_start_line: usize,
}

/// Sets the terminal cursor position for edit mode.
/// Note: Scroll adjustment must be done before rendering via `ensure_line_visible`.
pub fn set_edit_cursor(
    f: &mut ratatui::Frame<'_>,
    ctx: &CursorContext,
    scroll_offset: usize,
    content_area: Rect,
) {
    let cursor_line = ctx.entry_start_line + ctx.cursor_row;

    if cursor_line >= scroll_offset {
        let screen_row = cursor_line - scroll_offset;

        #[allow(clippy::cast_possible_truncation)]
        let cursor_x = content_area.x + (ctx.prefix_width + ctx.cursor_col) as u16;
        #[allow(clippy::cast_possible_truncation)]
        let cursor_y = content_area.y + screen_row as u16;

        if cursor_x < content_area.x + content_area.width
            && cursor_y < content_area.y + content_area.height
        {
            f.set_cursor_position((cursor_x, cursor_y));
        }
    }
}
