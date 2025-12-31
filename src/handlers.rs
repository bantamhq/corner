use std::io;

use crossterm::event::KeyCode;

use crate::app::{App, Mode};

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

pub fn handle_normal_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
        KeyCode::Char(':') => app.mode = Mode::Command,
        KeyCode::Tab => app.enter_tasks_mode()?,
        KeyCode::Enter => app.new_task(true),
        KeyCode::Char('i') => app.new_task(false),
        KeyCode::Char('e') => app.edit_selected(),
        KeyCode::Char('x') => app.toggle_task(),
        KeyCode::Char('d') => {
            app.delete_selected();
            app.save();
        }
        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Char('h' | '[') => app.prev_day()?,
        KeyCode::Char('l' | ']') => app.next_day()?,
        KeyCode::Char('t') => app.goto_today()?,
        _ => {}
    }
    Ok(())
}

pub fn handle_editing_key(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Tab => app.cycle_entry_type(),
        KeyCode::Enter => app.commit_and_add_new(),
        KeyCode::Esc => app.exit_edit(),
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

pub fn handle_tasks_key(app: &mut App, key: KeyCode) -> io::Result<()> {
    match key {
        KeyCode::Tab => app.exit_tasks_mode(),
        KeyCode::Up | KeyCode::Char('k') => app.task_move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.task_move_down(),
        KeyCode::Enter => app.task_jump_to_day()?,
        KeyCode::Char('x') => app.task_toggle()?,
        KeyCode::Char(':') => app.mode = Mode::Command,
        _ => {}
    }
    Ok(())
}
