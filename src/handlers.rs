use std::io;

use crossterm::event::KeyCode;

use crate::app::{App, Mode};
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
            app.mode = Mode::Daily;
        }
        KeyCode::Backspace => {
            if app.command_buffer.is_empty() {
                app.mode = Mode::Daily;
            } else {
                app.command_buffer.pop();
            }
        }
        KeyCode::Char(c) => app.command_buffer.push(c),
        _ => {}
    }
    Ok(())
}

pub fn handle_daily_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Char(':') => app.mode = Mode::Command,
        KeyCode::Char('/') => app.enter_filter_input(),
        KeyCode::Enter => app.new_task(true),
        KeyCode::Char('o') => app.new_task(false),
        KeyCode::Char('e') => app.edit_selected(),
        KeyCode::Char('x') => app.toggle_task(),
        KeyCode::Char('d') => {
            app.delete_selected();
            app.save();
        }
        KeyCode::Char('u') => app.undo(),
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Char('g') => app.jump_to_first(),
        KeyCode::Char('G') => app.jump_to_last(),
        KeyCode::Char('h' | '[') => app.prev_day()?,
        KeyCode::Char('l' | ']') => app.next_day()?,
        KeyCode::Char('t') => app.goto_today()?,
        KeyCode::Char('s') => app.gather_completed_tasks(),
        KeyCode::Char('m') => app.enter_order_mode(),
        _ => {}
    }
    Ok(())
}

pub fn handle_editing_key(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::BackTab => app.cycle_entry_type(),
        KeyCode::Tab => app.commit_and_add_new(),
        KeyCode::Enter => app.exit_edit(),
        KeyCode::Esc => app.cancel_edit(),
        KeyCode::Backspace => {
            if let Some(ref mut buffer) = app.edit_buffer
                && !buffer.delete_char_before()
                && buffer.is_empty()
            {
                app.delete_selected();
                app.edit_buffer = None;
                app.mode = Mode::Daily;
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

pub fn handle_filter_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Char('/') => app.enter_filter_input(),
        KeyCode::Esc => app.exit_filter(),
        KeyCode::Up | KeyCode::Char('k') => app.filter_move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.filter_move_down(),
        KeyCode::Char('g') => {
            app.filter_selected = 0;
        }
        KeyCode::Char('G') => {
            if !app.filter_items.is_empty() {
                app.filter_selected = app.filter_items.len() - 1;
            }
        }
        KeyCode::Enter => app.filter_jump_to_day()?,
        KeyCode::Char('e') => app.filter_edit(),
        KeyCode::Char('x') => app.filter_toggle()?,
        KeyCode::Char('r') => app.refresh_filter()?,
        KeyCode::Char(':') => app.mode = Mode::Command,
        _ => {}
    }
    Ok(())
}

pub fn handle_filter_input_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
        KeyCode::Enter => app.execute_filter()?,
        KeyCode::Esc => app.cancel_filter_input(),
        KeyCode::Backspace => {
            app.filter_buffer.pop();
        }
        KeyCode::Char(c) => app.filter_buffer.push(c),
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
