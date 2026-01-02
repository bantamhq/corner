use std::io;

use crossterm::event::KeyCode;

use crate::app::{App, InputMode, InsertPosition, ViewMode};
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

pub fn handle_command_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
        KeyCode::Enter => app.execute_command()?,
        KeyCode::Esc => {
            app.command_buffer.clear();
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            if app.command_buffer.is_empty() {
                app.input_mode = InputMode::Normal;
            } else {
                app.command_buffer.pop();
            }
        }
        KeyCode::Char(c) => app.command_buffer.push(c),
        _ => {}
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
        KeyCode::Char('e') => {
            app.edit_current_entry();
            return Ok(());
        }
        KeyCode::Char('x') => {
            app.toggle_current_entry()?;
            return Ok(());
        }
        KeyCode::Char('d') => {
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
            KeyCode::Char('m') => app.enter_order_mode(),
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

pub fn handle_edit_key(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::BackTab => app.cycle_edit_entry_type(),
        KeyCode::Tab => app.commit_and_add_new(),
        KeyCode::Enter => app.exit_edit(),
        KeyCode::Esc => app.cancel_edit(),
        KeyCode::Backspace => {
            if let Some(ref mut buffer) = app.edit_buffer
                && !buffer.delete_char_before()
                && buffer.is_empty()
            {
                // Empty buffer backspace = cancel and delete the empty entry
                // Delegate to exit_edit which handles this correctly
                app.exit_edit();
            }
        }
        KeyCode::Left => {
            if let Some(ref mut buffer) = app.edit_buffer {
                buffer.move_left();
            }
        }
        KeyCode::Right => {
            if let Some(ref mut buffer) = app.edit_buffer {
                buffer.move_right();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut buffer) = app.edit_buffer {
                buffer.insert_char(c);
            }
        }
        _ => {}
    }
}

pub fn handle_query_input_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    // Get query buffer from filter state or use command buffer as temp storage
    let query_buffer = match &app.view {
        ViewMode::Filter(state) => state.query_buffer.clone(),
        ViewMode::Daily(_) => app.command_buffer.clone(),
    };

    match key {
        KeyCode::Enter => {
            if query_buffer.is_empty() {
                app.cancel_filter_input();
            } else {
                app.execute_filter()?;
            }
        }
        KeyCode::Esc => {
            if query_buffer.is_empty() {
                app.cancel_filter_input();
            } else {
                match &mut app.view {
                    ViewMode::Filter(state) => state.query_buffer.clear(),
                    ViewMode::Daily(_) => app.command_buffer.clear(),
                }
            }
        }
        KeyCode::Backspace => {
            if query_buffer.is_empty() {
                app.cancel_filter_input();
            } else {
                match &mut app.view {
                    ViewMode::Filter(state) => {
                        state.query_buffer.pop();
                    }
                    ViewMode::Daily(_) => {
                        app.command_buffer.pop();
                    }
                }
            }
        }
        KeyCode::Char(c) => match &mut app.view {
            ViewMode::Filter(state) => state.query_buffer.push(c),
            ViewMode::Daily(_) => app.command_buffer.push(c),
        },
        _ => {}
    }
    Ok(())
}

pub fn handle_order_key(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('m') | KeyCode::Enter => app.exit_order_mode(true),
        KeyCode::Esc => app.exit_order_mode(false),
        KeyCode::Up | KeyCode::Char('k') => app.order_move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.order_move_down(),
        _ => {}
    }
}
