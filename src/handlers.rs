use std::io;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, ConfirmContext, InputMode, InsertPosition, SelectedItem, ViewMode};
use crate::config::Config;
use crate::cursor::CursorBuffer;
use crate::dispatch::KeySpec;
use crate::registry::{KeyActionId, KeyContext};
use crate::storage;

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

fn dispatch_entry_op<S, C>(app: &mut App, for_selected: S, for_current: C) -> io::Result<()>
where
    S: FnOnce(&mut App) -> io::Result<()>,
    C: FnOnce(&mut App) -> io::Result<()>,
{
    if matches!(app.input_mode, InputMode::Selection(_)) {
        for_selected(app)
    } else {
        for_current(app)
    }
}

fn handle_hint_navigation(app: &mut App, code: KeyCode) -> bool {
    if matches!(code, KeyCode::Up | KeyCode::Down) && app.hint_state.is_active() {
        if code == KeyCode::Down {
            app.hint_state.select_next();
        } else {
            app.hint_state.select_prev();
        }
        return true;
    }
    false
}

fn dispatch_date_navigation(app: &mut App, action: KeyActionId) -> io::Result<()> {
    if !app.is_daily_view() {
        return Ok(());
    }
    use KeyActionId::*;
    match action {
        MoveLeft => app.prev_day()?,
        MoveRight => app.next_day()?,
        PrevWeek => app.prev_week()?,
        NextWeek => app.next_week()?,
        PrevMonth => app.prev_month()?,
        NextMonth => app.next_month()?,
        PrevYear => app.prev_year()?,
        NextYear => app.next_year()?,
        GotoToday => app.goto_today()?,
        DatePicker => app.open_date_picker(),
        _ => {}
    }
    Ok(())
}

fn dispatch_entry_operation(app: &mut App, action: KeyActionId) -> io::Result<()> {
    use KeyActionId::*;
    match action {
        ToggleComplete => {
            dispatch_entry_op(app, App::toggle_selected, App::toggle_current_entry)?;
        }
        Delete => {
            dispatch_entry_op(app, App::delete_selected, App::delete_current_entry)?;
        }
        MoveToToday => {
            dispatch_entry_op(
                app,
                App::move_selected_to_today,
                App::move_current_entry_to_today,
            )?;
        }
        MoveToTomorrow => {
            dispatch_entry_op(
                app,
                App::move_selected_to_tomorrow,
                App::move_current_entry_to_tomorrow,
            )?;
        }
        Yank => {
            dispatch_entry_op(
                app,
                |a| {
                    a.yank_selected();
                    Ok(())
                },
                |a| {
                    a.yank_current_entry();
                    Ok(())
                },
            )?;
        }
        RemoveLastTag => {
            dispatch_entry_op(
                app,
                App::remove_last_tag_from_selected,
                App::remove_last_tag_from_current_entry,
            )?;
        }
        RemoveAllTags => {
            dispatch_entry_op(
                app,
                App::remove_all_tags_from_selected,
                App::remove_all_tags_from_current_entry,
            )?;
        }
        CycleEntryType => match &app.input_mode {
            InputMode::Edit(_) => app.cycle_edit_entry_type(),
            InputMode::Selection(_) => {
                app.cycle_selected_entry_types()?;
            }
            _ => {
                app.cycle_current_entry_type()?;
            }
        },
        _ => {}
    }
    Ok(())
}

fn dispatch_action(app: &mut App, action: KeyActionId) -> io::Result<bool> {
    use KeyActionId::*;

    // Date navigation (daily view only)
    if matches!(
        action,
        MoveLeft
            | MoveRight
            | PrevWeek
            | NextWeek
            | PrevMonth
            | NextMonth
            | PrevYear
            | NextYear
            | GotoToday
            | DatePicker
    ) {
        dispatch_date_navigation(app, action)?;
        return Ok(true);
    }

    // Entry operations (selection-aware)
    if matches!(
        action,
        ToggleComplete
            | Delete
            | MoveToToday
            | MoveToTomorrow
            | Yank
            | RemoveLastTag
            | RemoveAllTags
            | CycleEntryType
    ) {
        dispatch_entry_operation(app, action)?;
        return Ok(true);
    }

    match action {
        Submit => match &app.input_mode {
            InputMode::Edit(_) => {
                app.accept_hint();
                app.clear_hints();
                app.exit_edit();
            }
            InputMode::Reorder => app.save_reorder_mode(),
            InputMode::Normal => match app.view {
                ViewMode::Daily(_) => app.new_task(InsertPosition::Bottom),
                ViewMode::Filter(_) => app.filter_quick_add(),
            },
            _ => {}
        },
        Cancel => match &app.input_mode {
            InputMode::Edit(_) => {
                app.clear_hints();
                app.cancel_edit_mode();
            }
            InputMode::Reorder => app.cancel_reorder_mode(),
            InputMode::Selection(_) => app.cancel_selection_mode(),
            InputMode::CommandPalette(_) => app.close_command_palette(),
            InputMode::FilterPrompt => app.cancel_filter_prompt(),
            InputMode::DatePicker(_) => app.close_date_picker(),
            InputMode::Normal | InputMode::Confirm(_) => {}
        },
        MoveDown => match &app.input_mode {
            InputMode::Reorder => app.reorder_move_down(),
            InputMode::Selection(_) => app.selection_move_down(),
            _ => app.move_down(),
        },
        MoveUp => match &app.input_mode {
            InputMode::Reorder => app.reorder_move_up(),
            InputMode::Selection(_) => app.selection_move_up(),
            _ => app.move_up(),
        },
        JumpToFirst => match &app.input_mode {
            InputMode::Selection(_) => app.selection_jump_to_first(),
            _ => app.jump_to_first(),
        },
        JumpToLast => match &app.input_mode {
            InputMode::Selection(_) => app.selection_jump_to_last(),
            _ => app.jump_to_last(),
        },
        NewEntryBelow => {
            if let SelectedItem::Projected { entry, .. } = app.get_selected_item() {
                app.go_to_source(entry.source_date, entry.line_index)?;
            } else {
                app.new_task(InsertPosition::Below);
            }
        }
        NewEntryAbove => app.new_task(InsertPosition::Above),
        Edit => app.edit_current_entry(),
        Paste => app.paste_from_clipboard()?,
        Undo => app.undo(),
        Redo => app.redo()?,
        Selection => {
            if matches!(app.input_mode, InputMode::Selection(_)) {
                app.selection_toggle_current();
            } else {
                app.enter_selection_mode();
            }
        }
        SelectionExtendRange => app.selection_extend_to_cursor(),
        ToggleFilterView => app.cycle_view()?,
        ToggleJournal => app.toggle_journal()?,
        CommandPalette => app.toggle_command_palette(),
        ToggleCalendarSidebar => app.toggle_calendar_sidebar(),
        ToggleAgenda => app.toggle_agenda(),
        FilterQuickAdd => app.filter_quick_add(),
        Refresh => app.refresh_filter()?,
        SaveAndNew => {
            app.accept_hint();
            app.clear_hints();
            app.commit_and_add_new();
        }
        ReorderMode => app.enter_reorder_mode(),
        TidyEntries => app.tidy_entries(),
        Hide => app.toggle_hide_completed(),
        Autocomplete => {
            app.accept_hint();
            if let Some(ref mut buffer) = app.edit_buffer {
                buffer.insert_char(' ');
            }
            app.update_hints();
        }
        Quit => app.should_quit = true,
        FilterPrompt => app.enter_filter_prompt()?,
        NoOp => {}
        // Already handled above
        _ => {}
    }
    Ok(true)
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
    if handle_hint_navigation(app, key.code) {
        return;
    }

    let spec = KeySpec::from_event(&key);
    if let Some(action) = app.keymap.get(KeyContext::Edit, &spec) {
        let _ = dispatch_action(app, action);
        return;
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

pub fn handle_command_palette_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let spec = KeySpec::from_event(&key);
    if let Some(action) = app.keymap.get(KeyContext::CommandPalette, &spec) {
        match action {
            KeyActionId::Cancel => app.close_command_palette(),
            KeyActionId::CommandPalette => app.toggle_command_palette(),
            KeyActionId::MoveUp => app.command_palette_select_prev(),
            KeyActionId::MoveDown => app.command_palette_select_next(),
            KeyActionId::MoveLeft => app.command_palette_prev_tab(),
            KeyActionId::MoveRight => app.command_palette_next_tab(),
            KeyActionId::Submit => {
                app.execute_selected_palette_item()?;
                app.close_command_palette();
            }
            KeyActionId::Delete => {
                app.palette_delete_selected()?;
            }
            KeyActionId::Hide => {
                app.palette_hide_selected()?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn handle_reorder_key(app: &mut App, key: KeyEvent) {
    let spec = KeySpec::from_event(&key);
    if let Some(action) = app.keymap.get(KeyContext::Reorder, &spec) {
        let _ = dispatch_action(app, action);
    }
}

pub(crate) fn handle_text_input(buffer: &mut CursorBuffer, key: KeyEvent) -> bool {
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
                let root = if app.in_git_repo {
                    storage::find_git_root()
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
                } else {
                    std::env::current_dir()?
                };

                let caliber_dir = root.join(".caliber");
                if let Err(e) = std::fs::create_dir_all(&caliber_dir) {
                    app.set_error(format!("Failed to create project: {e}"));
                    app.input_mode = InputMode::Normal;
                    return Ok(());
                }

                // Load config to get custom journal path if configured
                let config = Config::load_merged_from(&root).unwrap_or_default();
                let journal_path = config.get_project_journal_path(&root);

                if !journal_path.exists() {
                    // Create parent directories if journal is at custom location
                    if let Some(parent) = journal_path.parent()
                        && let Err(e) = std::fs::create_dir_all(parent)
                    {
                        app.set_error(format!("Failed to create journal directory: {e}"));
                        app.input_mode = InputMode::Normal;
                        return Ok(());
                    }
                    if let Err(e) = std::fs::write(&journal_path, "") {
                        app.set_error(format!("Failed to create journal: {e}"));
                        app.input_mode = InputMode::Normal;
                        return Ok(());
                    }
                }

                if std::env::var("CALIBER_SKIP_REGISTRY").is_err() {
                    let mut registry = storage::ProjectRegistry::load();
                    if registry.find_by_path(&caliber_dir).is_none() {
                        let _ = registry.register(caliber_dir);
                        let _ = registry.save();
                    }
                }

                app.journal_context.set_project_path(journal_path);
                app.switch_to_project()?;
                app.set_status("Project initialized");
                app.input_mode = InputMode::Normal;
            }
            ConfirmContext::DeleteTag(tag) => {
                app.confirm_delete_tag(&tag)?;
            }
        },
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.set_status("Staying on Hub journal");
            app.input_mode = InputMode::Normal;
        }
        _ => {}
    }

    Ok(())
}

pub fn handle_selection_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let KeyEvent {
        code, modifiers, ..
    } = key;

    if let KeyCode::Char(c) = code
        && modifiers.contains(KeyModifiers::SHIFT)
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

pub fn handle_filter_prompt_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    if handle_hint_navigation(app, key.code) {
        return Ok(());
    }

    match key.code {
        KeyCode::Enter => {
            app.accept_hint();
            app.submit_filter_prompt()?;
        }
        KeyCode::Esc => {
            app.cancel_filter_prompt();
        }
        KeyCode::Tab => {
            if app.accept_hint()
                && let ViewMode::Filter(state) = &mut app.view
            {
                state.query_buffer.insert_char(' ');
            }
            app.update_hints();
        }
        _ => {
            if let ViewMode::Filter(state) = &mut app.view {
                handle_text_input(&mut state.query_buffer, key);
            }
            app.clear_status();
            app.update_hints();
        }
    }

    Ok(())
}

pub fn handle_date_picker_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Enter => app.submit_date_picker()?,
        KeyCode::Esc => app.close_date_picker(),
        _ => {
            let InputMode::DatePicker(state) = &mut app.input_mode else {
                return Ok(());
            };
            match key.code {
                KeyCode::Char(c) if c.is_ascii_digit() || c == '/' => {
                    if state.buffer.content().len() < 10 {
                        state.buffer.insert_char(c);
                    }
                }
                KeyCode::Backspace => {
                    state.buffer.delete_char_before();
                }
                KeyCode::Left => state.buffer.move_left(),
                KeyCode::Right => state.buffer.move_right(),
                _ => {}
            }
        }
    }
    Ok(())
}
