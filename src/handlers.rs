use std::io;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, ConfirmContext, InputMode, InsertPosition, ViewMode};
use crate::cursor::CursorBuffer;
use crate::storage;
use crate::ui;

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
        KeyCode::Enter => app.execute_command()?,
        KeyCode::Esc => {
            app.command_buffer.clear();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace if app.command_buffer.is_empty() => {
            app.input_mode = InputMode::Normal;
        }
        _ => {
            handle_text_input(&mut app.command_buffer, key);
        }
    }
    Ok(())
}

pub fn handle_normal_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    // Shared keys (work in both Daily and Filter views)
    match key {
        KeyCode::Char('?') => {
            app.show_help = true;
            return Ok(());
        }
        KeyCode::Char(':') => {
            app.input_mode = InputMode::Command;
            return Ok(());
        }
        KeyCode::Char('/') => {
            app.enter_filter_input();
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
        KeyCode::Char('x') => {
            app.delete_current_entry()?;
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
        KeyCode::Char('`') => {
            app.toggle_journal()?;
            return Ok(());
        }
        KeyCode::Char('v') => {
            app.view_entry_source()?;
            return Ok(());
        }
        KeyCode::Char('y') => {
            app.yank_current_entry();
            return Ok(());
        }
        KeyCode::Char(c @ ('1'..='9' | '0')) => {
            if let Some(tag) = app.config.get_favorite_tag(c) {
                app.quick_filter(&format!("#{tag}"))?;
            }
            return Ok(());
        }
        _ => {}
    }

    // View-specific keys
    match &app.view {
        ViewMode::Daily(_) => match key {
            KeyCode::Enter => app.new_task(InsertPosition::Bottom),
            KeyCode::Char('o') => app.new_task(InsertPosition::Below),
            KeyCode::Char('O') => app.new_task(InsertPosition::Above),
            KeyCode::Char('h' | '[') => app.prev_day()?,
            KeyCode::Char('l' | ']') => app.next_day()?,
            KeyCode::Char('t') => app.goto_today()?,
            KeyCode::Char('s') => app.sort_entries(),
            KeyCode::Char('r') => app.enter_reorder_mode(),
            KeyCode::Tab => app.return_to_filter()?,
            _ => {}
        },
        ViewMode::Filter(_) => match key {
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
            app.commit_and_add_new();
            return;
        }
        KeyCode::Enter => {
            app.exit_edit();
            return;
        }
        KeyCode::Esc => {
            app.cancel_edit();
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
            app.exit_edit();
            return;
        }

        if key.code != KeyCode::Backspace {
            handle_text_input(buffer, key);
        }
    }
}

pub fn handle_query_input_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let is_empty = match &app.view {
        ViewMode::Filter(state) => state.query_buffer.is_empty(),
        ViewMode::Daily(_) => app.command_buffer.is_empty(),
    };

    match key.code {
        KeyCode::Enter => {
            if is_empty {
                app.cancel_filter_input();
            } else {
                app.execute_filter()?;
            }
        }
        KeyCode::Esc => {
            if is_empty {
                app.cancel_filter_input();
            } else {
                match &mut app.view {
                    ViewMode::Filter(state) => state.query_buffer.clear(),
                    ViewMode::Daily(_) => app.command_buffer.clear(),
                }
            }
        }
        KeyCode::Backspace if is_empty && key.modifiers.is_empty() => {
            app.cancel_filter_input();
        }
        _ => match &mut app.view {
            ViewMode::Filter(state) => {
                handle_text_input(&mut state.query_buffer, key);
            }
            ViewMode::Daily(_) => {
                handle_text_input(&mut app.command_buffer, key);
            }
        },
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
                match storage::create_project_journal() {
                    Ok(path) => {
                        storage::set_project_path(path);
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
                if let Err(e) = storage::add_caliber_to_gitignore() {
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
                app.set_status("Staying on Global journal");
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
