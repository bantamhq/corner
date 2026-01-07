use std::io;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
    App, ConfirmContext, HintContext, InputMode, InsertPosition, SelectedItem, ViewMode,
};
use crate::cursor::CursorBuffer;
use crate::storage::{add_caliber_to_gitignore, create_project_journal};
use crate::ui;

/// Maps shifted number characters to their digit equivalent for favorite tag shortcuts.
/// Shift+1='!', Shift+2='@', ..., Shift+0=')'
fn shifted_char_to_digit(c: char) -> Option<char> {
    match c {
        '!' => Some('1'),
        '@' => Some('2'),
        '#' => Some('3'),
        '$' => Some('4'),
        '%' => Some('5'),
        '^' => Some('6'),
        '&' => Some('7'),
        '*' => Some('8'),
        '(' => Some('9'),
        ')' => Some('0'),
        _ => None,
    }
}

pub fn handle_help_key(app: &mut App, key: KeyCode) {
    let total_lines = ui::get_help_total_lines();
    let max_scroll = total_lines.saturating_sub(app.help_visible_height);

    match key {
        KeyCode::Char('?') | KeyCode::Esc => {
            app.show_help = false;
            app.help_scroll = 0;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.help_scroll < max_scroll {
                app.help_scroll += 1;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.help_scroll = app.help_scroll.saturating_sub(1);
        }
        _ => {}
    }
}

pub fn handle_command_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Enter => {
            let did_autocomplete =
                !matches!(app.hint_state, HintContext::Inactive) && app.accept_hint();

            if did_autocomplete && !app.command_is_complete() {
                if !app.input_needs_continuation() {
                    app.command_buffer.insert_char(' ');
                }
                app.update_hints();
            } else {
                app.clear_hints();
                app.execute_command()?;
            }
        }
        KeyCode::Esc => {
            app.clear_hints();
            app.command_buffer.clear();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace if app.command_buffer.is_empty() => {
            app.clear_hints();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Right if !matches!(app.hint_state, HintContext::Inactive) => {
            if app.accept_hint() {
                if !app.input_needs_continuation() {
                    app.command_buffer.insert_char(' ');
                }
                app.update_hints();
            } else {
                handle_text_input(&mut app.command_buffer, key);
                app.update_hints();
            }
        }
        _ => {
            handle_text_input(&mut app.command_buffer, key);
            app.update_hints();
        }
    }
    Ok(())
}

pub fn handle_normal_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let KeyEvent { code, .. } = key;

    // Shared keys (work in both Daily and Filter views)
    match code {
        KeyCode::Char('?') => {
            app.show_help = true;
            return Ok(());
        }
        KeyCode::Char(':') => {
            app.input_mode = InputMode::Command;
            app.update_hints(); // Show all commands immediately
            return Ok(());
        }
        KeyCode::Char('/') => {
            app.enter_filter_input();
            return Ok(());
        }
        KeyCode::Char('\\') => {
            app.open_datepicker();
            return Ok(());
        }
        KeyCode::Char('i') => {
            app.edit_current_entry();
            return Ok(());
        }
        KeyCode::Char('c') => {
            app.toggle_current_entry()?;
            return Ok(());
        }
        KeyCode::Char('d') => {
            app.delete_current_entry()?;
            return Ok(());
        }
        KeyCode::Char('x') => {
            app.remove_last_tag_from_current_entry()?;
            return Ok(());
        }
        KeyCode::Char('X') => {
            app.remove_all_tags_from_current_entry()?;
            return Ok(());
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_up();
            return Ok(());
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_down();
            return Ok(());
        }
        KeyCode::Char('g') => {
            app.jump_to_first();
            return Ok(());
        }
        KeyCode::Char('G') => {
            app.jump_to_last();
            return Ok(());
        }
        KeyCode::Char('u') => {
            app.undo();
            return Ok(());
        }
        KeyCode::Char('U') => {
            app.redo()?;
            return Ok(());
        }
        KeyCode::Char('`') => {
            app.toggle_journal()?;
            return Ok(());
        }
        KeyCode::Char('v') => {
            app.enter_selection_mode();
            return Ok(());
        }
        KeyCode::Char('y') => {
            app.yank_current_entry();
            return Ok(());
        }
        KeyCode::Char('p') => {
            app.paste_from_clipboard()?;
            return Ok(());
        }
        KeyCode::Char(c @ ('1'..='9' | '0')) => {
            if let Some(tag) = app.config.get_favorite_tag(c) {
                app.quick_filter(&format!("#{tag}"))?;
            }
            return Ok(());
        }
        // Shift+number appends favorite tag to current entry
        KeyCode::Char(c) if shifted_char_to_digit(c).is_some() => {
            let digit = shifted_char_to_digit(c).unwrap();
            if let Some(tag) = app.config.get_favorite_tag(digit).map(str::to_string) {
                app.append_tag_to_current_entry(&tag)?;
            }
            return Ok(());
        }
        KeyCode::BackTab => {
            app.cycle_current_entry_type()?;
            return Ok(());
        }
        _ => {}
    }

    // View-specific keys
    match &app.view {
        ViewMode::Daily(_) => match code {
            KeyCode::Enter => app.new_task(InsertPosition::Bottom),
            KeyCode::Char('o') => {
                // On projected entries, go to source; otherwise insert new task below
                if let SelectedItem::Projected { entry, .. } = app.get_selected_item() {
                    app.go_to_source(entry.source_date, entry.line_index)?;
                } else {
                    app.new_task(InsertPosition::Below);
                }
            }
            KeyCode::Char('O') => app.new_task(InsertPosition::Above),
            KeyCode::Char('h' | '[') => app.prev_day()?,
            KeyCode::Char('l' | ']') => app.next_day()?,
            KeyCode::Char('t') => app.goto_today()?,
            KeyCode::Char('T') => app.tidy_entries(),
            KeyCode::Char('r') => app.enter_reorder_mode(),
            KeyCode::Char('z') => app.toggle_hide_completed(),
            KeyCode::Tab => app.return_to_filter()?,
            _ => {}
        },
        ViewMode::Filter(_) => match code {
            KeyCode::Esc | KeyCode::Tab => app.exit_filter(),
            KeyCode::Char('r') => app.refresh_filter()?,
            KeyCode::Enter => app.filter_quick_add(),
            _ => {}
        },
    }
    Ok(())
}

pub fn handle_edit_key(app: &mut App, key: KeyEvent) {
    // Edit-specific keys first
    match key.code {
        KeyCode::BackTab => {
            app.cycle_edit_entry_type();
            return;
        }
        KeyCode::Tab => {
            app.clear_hints();
            app.commit_and_add_new();
            return;
        }
        KeyCode::Enter => {
            app.clear_hints();
            app.exit_edit();
            return;
        }
        KeyCode::Esc => {
            app.clear_hints();
            app.cancel_edit();
            return;
        }
        KeyCode::Right if !matches!(app.hint_state, HintContext::Inactive) => {
            if !app.accept_hint()
                && let Some(ref mut buffer) = app.edit_buffer
            {
                handle_text_input(buffer, key);
                app.update_hints();
            }
            return;
        }
        _ => {}
    }

    if let Some(ref mut buffer) = app.edit_buffer {
        // Special case: empty buffer backspace cancels edit
        if key.code == KeyCode::Backspace
            && key.modifiers.is_empty()
            && !buffer.delete_char_before()
            && buffer.is_empty()
        {
            app.clear_hints();
            app.exit_edit();
            return;
        }

        if key.code != KeyCode::Backspace {
            handle_text_input(buffer, key);
        }
        app.update_hints();
    }
}

pub fn handle_query_input_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Enter => {
            if !matches!(app.hint_state, HintContext::Inactive) {
                app.accept_hint();
            }

            if app.input_needs_continuation() {
                app.update_hints();
            } else {
                app.clear_hints();
                if app.query_is_empty() {
                    app.cancel_filter_input();
                } else {
                    app.execute_filter()?;
                }
            }
        }
        KeyCode::Esc => {
            app.clear_hints();
            app.cancel_filter_input();
        }
        KeyCode::Backspace if app.query_is_empty() && key.modifiers.is_empty() => {
            app.clear_hints();
            app.cancel_filter_input();
        }
        KeyCode::Right if !matches!(app.hint_state, HintContext::Inactive) => {
            if app.accept_hint() {
                if !app.input_needs_continuation() {
                    app.query_insert_char(' ');
                }
                app.update_hints();
            } else {
                handle_text_input(app.query_buffer_mut(), key);
                app.update_hints();
            }
        }
        _ => {
            handle_text_input(app.query_buffer_mut(), key);
            app.update_hints();
        }
    }
    Ok(())
}

pub fn handle_reorder_key(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('r') | KeyCode::Enter => app.exit_reorder_mode(true),
        KeyCode::Esc => app.exit_reorder_mode(false),
        KeyCode::Up | KeyCode::Char('k') => app.reorder_move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.reorder_move_down(),
        _ => {}
    }
}

/// Shared text input handler for CursorBuffer
/// Returns true if the key was handled
fn handle_text_input(buffer: &mut CursorBuffer, key: KeyEvent) -> bool {
    let KeyEvent {
        code, modifiers, ..
    } = key;

    // Ctrl modifiers
    if modifiers.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('a') => buffer.move_to_start(),
            KeyCode::Char('e') => buffer.move_to_end(),
            KeyCode::Char('w') => buffer.delete_word_before(),
            KeyCode::Char('u') => buffer.delete_to_start(),
            KeyCode::Char('k') => buffer.delete_to_end(),
            KeyCode::Left => buffer.move_word_left(),
            KeyCode::Right => buffer.move_word_right(),
            _ => return false,
        }
        return true;
    }

    // Alt modifiers
    if modifiers.contains(KeyModifiers::ALT) {
        match code {
            KeyCode::Char('b') => buffer.move_word_left(),
            KeyCode::Char('f') => buffer.move_word_right(),
            KeyCode::Char('d') => buffer.delete_word_after(),
            KeyCode::Backspace => buffer.delete_word_before(),
            _ => return false,
        }
        return true;
    }

    // No modifiers (or shift only for chars)
    match code {
        KeyCode::Left => buffer.move_left(),
        KeyCode::Right => buffer.move_right(),
        KeyCode::Home => buffer.move_to_start(),
        KeyCode::End => buffer.move_to_end(),
        KeyCode::Delete => {
            buffer.delete_char_after();
        }
        KeyCode::Backspace => {
            buffer.delete_char_before();
        }
        KeyCode::Char(c) => buffer.insert_char(c),
        _ => return false,
    }
    true
}

pub fn handle_confirm_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    let context = match &app.input_mode {
        InputMode::Confirm(ctx) => ctx.clone(),
        _ => return Ok(()),
    };

    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => match context {
            ConfirmContext::CreateProjectJournal => {
                match create_project_journal() {
                    Ok(path) => {
                        app.journal_context.set_project_path(path);
                        // Ask about .gitignore next
                        app.input_mode = InputMode::Confirm(ConfirmContext::AddToGitignore);
                    }
                    Err(e) => {
                        app.set_status(format!("Failed to create project journal: {e}"));
                        app.input_mode = InputMode::Normal;
                    }
                }
            }
            ConfirmContext::AddToGitignore => {
                if let Err(e) = add_caliber_to_gitignore() {
                    app.set_status(format!("Failed to update .gitignore: {e}"));
                } else {
                    app.set_status("Project journal created and added to .gitignore");
                }
                // Switch to project journal
                app.switch_to_project()?;
                app.input_mode = InputMode::Normal;
            }
        },
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => match context {
            ConfirmContext::CreateProjectJournal => {
                app.set_status("Staying on Hub journal");
                app.input_mode = InputMode::Normal;
            }
            ConfirmContext::AddToGitignore => {
                app.set_status("Project journal created (not added to .gitignore)");
                // Still switch to project journal
                app.switch_to_project()?;
                app.input_mode = InputMode::Normal;
            }
        },
        _ => {}
    }

    Ok(())
}

pub fn handle_selection_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let KeyEvent {
        code, modifiers, ..
    } = key;

    // Shift+V selects range from anchor to cursor
    if modifiers.contains(KeyModifiers::SHIFT) && code == KeyCode::Char('V') {
        app.selection_extend_to_cursor();
        return Ok(());
    }

    match code {
        KeyCode::Esc => {
            app.exit_selection_mode();
        }
        KeyCode::Char('v') => {
            app.selection_toggle_current();
        }
        // Navigation (without extending)
        KeyCode::Down | KeyCode::Char('j') => {
            app.selection_move_down();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.selection_move_up();
        }
        KeyCode::Char('g') => {
            app.selection_jump_to_first();
        }
        KeyCode::Char('G') => {
            app.selection_jump_to_last();
        }
        // Batch operations
        KeyCode::Char('d') => {
            app.delete_selected()?;
        }
        KeyCode::Char('c') => {
            app.toggle_selected()?;
        }
        KeyCode::Char('y') => {
            app.yank_selected();
        }
        KeyCode::Char('x') => {
            app.remove_last_tag_from_selected()?;
        }
        KeyCode::Char('X') => {
            app.remove_all_tags_from_selected()?;
        }
        // Shift+number appends favorite tag to all selected entries
        KeyCode::Char(c) if shifted_char_to_digit(c).is_some() => {
            let digit = shifted_char_to_digit(c).unwrap();
            if let Some(tag) = app.config.get_favorite_tag(digit).map(str::to_string) {
                app.append_tag_to_selected(&tag)?;
            }
        }
        KeyCode::BackTab => {
            app.cycle_selected_entry_types()?;
        }
        _ => {}
    }
    Ok(())
}

pub fn handle_datepicker_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let KeyEvent { code, .. } = key;

    match code {
        // Day navigation
        KeyCode::Left | KeyCode::Char('h') => app.datepicker_move(-1, 0),
        KeyCode::Right | KeyCode::Char('l') => app.datepicker_move(1, 0),
        KeyCode::Up | KeyCode::Char('k') => app.datepicker_move(0, -1),
        KeyCode::Down | KeyCode::Char('j') => app.datepicker_move(0, 1),
        // Month navigation
        KeyCode::Char('[') => app.datepicker_prev_month(),
        KeyCode::Char(']') => app.datepicker_next_month(),
        // Year navigation
        KeyCode::Char('y') => app.datepicker_next_year(),
        KeyCode::Char('Y') => app.datepicker_prev_year(),
        // Jump to today
        KeyCode::Char('t') => app.datepicker_goto_today(),
        // Confirm/Cancel
        KeyCode::Enter => app.confirm_datepicker()?,
        KeyCode::Esc | KeyCode::Char('\\') => app.close_datepicker(),
        _ => {}
    }
    Ok(())
}
