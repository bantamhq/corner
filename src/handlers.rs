use std::io;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
    App, ConfirmContext, HintContext, InputMode, InsertPosition, InterfaceContext, PromptContext,
    SelectedItem, ViewMode,
};
use crate::cursor::CursorBuffer;
use crate::dispatch::KeySpec;
use crate::registry::{KeyActionId, KeyContext};
use crate::storage::add_caliber_to_gitignore;
use crate::ui;

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

fn dispatch_action(app: &mut App, action: KeyActionId) -> io::Result<bool> {
    use KeyActionId::*;
    match action {
        MoveDown => app.move_down(),
        MoveUp => app.move_up(),
        JumpToFirst => app.jump_to_first(),
        JumpToLast => app.jump_to_last(),
        EditEntry => app.edit_current_entry(),
        ToggleEntry => {
            app.toggle_current_entry()?;
        }
        DeleteEntry => {
            app.delete_current_entry()?;
        }
        YankEntry => app.yank_current_entry(),
        Paste => {
            app.paste_from_clipboard()?;
        }
        Undo => app.undo(),
        Redo => {
            app.redo()?;
        }
        RemoveLastTag => {
            app.remove_last_tag_from_current_entry()?;
        }
        RemoveAllTags => {
            app.remove_all_tags_from_current_entry()?;
        }
        ToggleJournal => {
            app.toggle_journal()?;
        }
        EnterSelectionMode => app.enter_selection_mode(),
        CycleEntryTypeNormal => {
            app.cycle_current_entry_type()?;
        }
        ShowHelp => {
            app.show_help = true;
        }
        EnterFilterMode => app.enter_filter_input(),
        EnterCommandMode => {
            app.enter_command_mode();
            app.update_hints();
        }
        OpenDateInterface => app.open_date_interface(),
        OpenProjectInterface => app.open_project_interface(),
        NewEntryBottom => app.new_task(InsertPosition::Bottom),
        NewEntryBelow => {
            if let SelectedItem::Projected { entry, .. } = app.get_selected_item() {
                app.go_to_source(entry.source_date, entry.line_index)?;
            } else {
                app.new_task(InsertPosition::Below);
            }
        }
        NewEntryAbove => app.new_task(InsertPosition::Above),
        PrevDay => {
            app.prev_day()?;
        }
        NextDay => {
            app.next_day()?;
        }
        GotoToday => {
            app.goto_today()?;
        }
        TidyEntries => app.tidy_entries(),
        EnterReorderMode => app.enter_reorder_mode(),
        ToggleHideCompleted => app.toggle_hide_completed(),
        ReturnToFilter => {
            app.return_to_filter()?;
        }
        FilterQuickAdd => app.filter_quick_add(),
        RefreshFilter => {
            app.refresh_filter()?;
        }
        ExitFilter => app.exit_filter(),
        SaveEdit => {
            app.clear_hints();
            app.exit_edit();
        }
        SaveAndNew => {
            app.clear_hints();
            app.commit_and_add_new();
        }
        CycleEntryType => app.cycle_edit_entry_type(),
        CancelEdit => {
            app.clear_hints();
            app.cancel_edit();
        }
        ReorderMoveDown => app.reorder_move_down(),
        ReorderMoveUp => app.reorder_move_up(),
        ReorderSave => app.exit_reorder_mode(true),
        ReorderCancel => app.exit_reorder_mode(false),
        SelectionToggle => app.selection_toggle_current(),
        SelectionExtendRange => app.selection_extend_to_cursor(),
        SelectionMoveDown => app.selection_move_down(),
        SelectionMoveUp => app.selection_move_up(),
        SelectionJumpToFirst => app.selection_jump_to_first(),
        SelectionJumpToLast => app.selection_jump_to_last(),
        SelectionDelete => {
            app.delete_selected()?;
        }
        SelectionToggleComplete => {
            app.toggle_selected()?;
        }
        SelectionYank => app.yank_selected(),
        SelectionRemoveLastTag => {
            app.remove_last_tag_from_selected()?;
        }
        SelectionRemoveAllTags => {
            app.remove_all_tags_from_selected()?;
        }
        SelectionCycleType => {
            app.cycle_selected_entry_types()?;
        }
        SelectionExit => app.exit_selection_mode(),
        HelpScrollDown => {
            let total_lines = ui::get_help_total_lines(&app.keymap);
            let max_scroll = total_lines.saturating_sub(app.help_visible_height);
            if app.help_scroll < max_scroll {
                app.help_scroll += 1;
            }
        }
        HelpScrollUp => {
            app.help_scroll = app.help_scroll.saturating_sub(1);
        }
        CloseHelp => {
            app.show_help = false;
            app.help_scroll = 0;
        }
        DateInterfaceMoveLeft => app.date_interface_move(-1, 0),
        DateInterfaceMoveRight => app.date_interface_move(1, 0),
        DateInterfaceMoveUp => app.date_interface_move(0, -1),
        DateInterfaceMoveDown => app.date_interface_move(0, 1),
        DateInterfacePrevMonth => app.date_interface_prev_month(),
        DateInterfaceNextMonth => app.date_interface_next_month(),
        DateInterfacePrevYear => app.date_interface_prev_year(),
        DateInterfaceNextYear => app.date_interface_next_year(),
        DateInterfaceToday => app.date_interface_goto_today(),
        DateInterfaceConfirm => {
            app.confirm_date_interface()?;
        }
        DateInterfaceCancel => app.close_date_interface(),
        ProjectInterfaceMoveUp => app.project_interface_move_up(),
        ProjectInterfaceMoveDown => app.project_interface_move_down(),
        ProjectInterfaceSelect => {
            app.project_interface_select()?;
        }
        ProjectInterfaceCancel => app.close_interface(),
        NoOp
        | QuickFilterTag
        | AppendFavoriteTag
        | SelectionAppendTag
        | DateInterfaceFooterNavMonth
        | DateInterfaceFooterNavYear => {}
    }
    Ok(true)
}

pub fn handle_help_key(app: &mut App, key: KeyEvent) {
    let spec = KeySpec::from_event(&key);
    if let Some(action) = app.keymap.get(KeyContext::Help, &spec) {
        let _ = dispatch_action(app, action);
    }
}

/// Handle keyboard input in prompt mode (command `:` or filter `/`).
///
/// Autocomplete behavior differs by key:
/// - Enter: autocomplete, then submit (unless input ends with `:`)
/// - Right: autocomplete, then add space (unless input ends with `:`)
pub fn handle_prompt_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let is_command = matches!(app.input_mode, InputMode::Prompt(PromptContext::Command { .. }));

    match key.code {
        KeyCode::Enter => {
            app.accept_hint();
            if app.input_needs_continuation() {
                app.update_hints();
            } else {
                app.clear_hints();
                if is_command {
                    app.execute_command()?;
                } else {
                    app.execute_filter()?;
                }
            }
        }
        KeyCode::Esc => {
            app.clear_hints();
            app.exit_prompt();
        }
        KeyCode::Backspace if app.prompt_is_empty() => {
            app.clear_hints();
            app.exit_prompt();
        }
        KeyCode::Right
            if !matches!(
                app.hint_state,
                HintContext::Inactive | HintContext::Commands { .. }
            ) =>
        {
            if app.accept_hint() {
                if !app.input_needs_continuation()
                    && let Some(buffer) = app.prompt_buffer_mut()
                {
                    buffer.insert_char(' ');
                }
                app.update_hints();
            } else if let Some(buffer) = app.prompt_buffer_mut() {
                handle_text_input(buffer, key);
                app.update_hints();
            }
        }
        _ => {
            if let Some(buffer) = app.prompt_buffer_mut() {
                handle_text_input(buffer, key);
            }
            app.update_hints();
        }
    }
    Ok(())
}

pub fn handle_normal_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let KeyEvent { code, .. } = key;

    if let KeyCode::Char(c @ ('1'..='9' | '0')) = code {
        if let Some(tag) = app.config.get_favorite_tag(c) {
            app.quick_filter(&format!("#{tag}"))?;
        }
        return Ok(());
    }

    if let KeyCode::Char(c) = code
        && let Some(digit) = shifted_char_to_digit(c)
    {
        if let Some(tag) = app.config.get_favorite_tag(digit).map(str::to_string) {
            app.append_tag_to_current_entry(&tag)?;
        }
        return Ok(());
    }

    let spec = KeySpec::from_event(&key);
    let context = match &app.view {
        ViewMode::Daily(_) => KeyContext::DailyNormal,
        ViewMode::Filter(_) => KeyContext::FilterNormal,
    };

    if let Some(action) = app.keymap.get(context, &spec) {
        dispatch_action(app, action)?;
    }

    Ok(())
}

pub fn handle_edit_key(app: &mut App, key: KeyEvent) {
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

pub fn handle_reorder_key(app: &mut App, key: KeyEvent) {
    let spec = KeySpec::from_event(&key);
    if let Some(action) = app.keymap.get(KeyContext::Reorder, &spec) {
        let _ = dispatch_action(app, action);
    }
}

fn handle_text_input(buffer: &mut CursorBuffer, key: KeyEvent) -> bool {
    let KeyEvent {
        code, modifiers, ..
    } = key;

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
                if let Err(e) = app.init_project() {
                    app.set_status(format!("Failed to create project: {e}"));
                    app.input_mode = InputMode::Normal;
                }
            }
            ConfirmContext::AddToGitignore => {
                if let Err(e) = add_caliber_to_gitignore() {
                    app.set_status(format!("Failed to update .gitignore: {e}"));
                } else {
                    app.set_status("Project created and added to .gitignore");
                }
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
                app.set_status("Project created (not added to .gitignore)");
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

    if modifiers.contains(KeyModifiers::SHIFT) && code == KeyCode::Char('V') {
        app.selection_extend_to_cursor();
        return Ok(());
    }

    if let KeyCode::Char(c) = code
        && let Some(digit) = shifted_char_to_digit(c)
    {
        if let Some(tag) = app.config.get_favorite_tag(digit).map(str::to_string) {
            app.append_tag_to_selected(&tag)?;
        }
        return Ok(());
    }

    let spec = KeySpec::from_event(&key);
    if let Some(action) = app.keymap.get(KeyContext::Selection, &spec) {
        dispatch_action(app, action)?;
    }
    Ok(())
}

pub fn handle_interface_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    // Toggle key closes interface even with input focus
    match (&app.input_mode, key.code) {
        (InputMode::Interface(InterfaceContext::Date(_)), KeyCode::Char('\\')) => {
            app.close_interface();
            return Ok(());
        }
        (InputMode::Interface(InterfaceContext::Project(_)), KeyCode::Char('+')) => {
            app.close_interface();
            return Ok(());
        }
        _ => {}
    }

    // Dispatch to context-specific handler
    match &app.input_mode {
        InputMode::Interface(InterfaceContext::Date(_)) => handle_date_interface_key(app, key),
        InputMode::Interface(InterfaceContext::Project(_)) => {
            handle_project_interface_key(app, key)
        }
        InputMode::Interface(InterfaceContext::Tag(_)) => Ok(()), // Stub for future implementation
        _ => Ok(()),
    }
}

fn handle_date_interface_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    if app.date_interface_input_focused() {
        // Input is focused - handle text editing
        match key.code {
            KeyCode::Enter => {
                app.date_interface_submit_input()?;
            }
            KeyCode::Esc => {
                app.close_date_interface();
            }
            KeyCode::Tab => {
                app.date_interface_toggle_focus();
            }
            KeyCode::Backspace if app.date_interface_input_is_empty() => {
                app.close_date_interface();
            }
            KeyCode::Backspace => {
                app.date_interface_input_backspace();
            }
            KeyCode::Delete => {
                app.date_interface_input_delete();
            }
            KeyCode::Left => {
                app.date_interface_input_move_left();
            }
            KeyCode::Right => {
                app.date_interface_input_move_right();
            }
            KeyCode::Char(c) => {
                app.date_interface_input_char(c);
            }
            _ => {}
        }
    } else {
        // Calendar is focused - use keymap, but Tab toggles to input
        match key.code {
            KeyCode::Tab => {
                app.date_interface_toggle_focus();
            }
            _ => {
                let spec = KeySpec::from_event(&key);
                if let Some(action) = app.keymap.get(KeyContext::DateInterface, &spec) {
                    dispatch_action(app, action)?;
                }
            }
        }
    }
    Ok(())
}

fn handle_project_interface_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    // Try keymap first for navigation actions
    let spec = KeySpec::from_event(&key);
    if let Some(action) = app.keymap.get(KeyContext::ProjectInterface, &spec) {
        dispatch_action(app, action)?;
        return Ok(());
    }

    // Handle text input for filtering
    match key.code {
        KeyCode::Char(c) => {
            let InputMode::Interface(InterfaceContext::Project(ref mut state)) = app.input_mode
            else {
                return Ok(());
            };
            state.query.insert_char(c);
            state.update_filter();
        }
        KeyCode::Backspace => {
            let is_empty = {
                let InputMode::Interface(InterfaceContext::Project(ref mut state)) = app.input_mode
                else {
                    return Ok(());
                };
                if state.query.delete_char_before() {
                    state.update_filter();
                    false
                } else {
                    state.query.is_empty()
                }
            };
            if is_empty {
                app.close_interface();
            }
        }
        _ => {}
    }
    Ok(())
}
