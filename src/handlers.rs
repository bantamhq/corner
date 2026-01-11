use std::io;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
    App, ConfirmContext, InputMode, InsertPosition, InterfaceContext, PromptContext, SelectedItem,
    ViewMode,
};
use crate::config::Config;
use crate::cursor::CursorBuffer;
use crate::dispatch::KeySpec;
use crate::registry::{KeyActionId, KeyContext};
use crate::storage;
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
        ToggleHelp => {
            if app.help_visible {
                app.help_visible = false;
                app.help_scroll = 0;
            } else {
                app.help_visible = true;
            }
        }
        ToggleFilterPrompt => {
            if matches!(
                app.input_mode,
                InputMode::Prompt(PromptContext::Filter { .. })
            ) {
                if app.prompt_is_empty() {
                    app.clear_hints();
                    app.cancel_prompt();
                }
            } else {
                app.enter_filter_input();
            }
        }
        ToggleCommandPrompt => {
            if matches!(
                app.input_mode,
                InputMode::Prompt(PromptContext::Command { .. })
            ) {
                if app.prompt_is_empty() {
                    app.clear_hints();
                    app.cancel_prompt();
                }
            } else {
                app.enter_command_mode();
                app.update_hints();
            }
        }
        ToggleDateInterface => {
            if matches!(
                app.input_mode,
                InputMode::Interface(InterfaceContext::Date(_))
            ) {
                app.cancel_interface();
            } else {
                app.open_date_interface();
            }
        }
        ToggleProjectInterface => {
            if matches!(
                app.input_mode,
                InputMode::Interface(InterfaceContext::Project(_))
            ) {
                app.cancel_interface();
            } else {
                app.open_project_interface();
            }
        }
        ToggleTagInterface => {
            if matches!(
                app.input_mode,
                InputMode::Interface(InterfaceContext::Tag(_))
            ) {
                app.cancel_interface();
            } else {
                app.open_tag_interface();
            }
        }
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
        ToggleFilter => {
            if matches!(app.view, ViewMode::Filter(_)) {
                app.cancel_filter();
            } else {
                app.return_to_filter()?;
            }
        }
        FilterQuickAdd => app.filter_quick_add(),
        RefreshFilter => {
            app.refresh_filter()?;
        }
        SaveEdit => {
            app.accept_hint();
            app.clear_hints();
            app.exit_edit();
        }
        SaveAndNew => {
            app.accept_hint();
            app.clear_hints();
            app.commit_and_add_new();
        }
        CycleEntryType => app.cycle_edit_entry_type(),
        Autocomplete => {
            app.accept_hint();
            if let Some(ref mut buffer) = app.edit_buffer {
                buffer.insert_char(' ');
            }
            app.update_hints();
        }
        ReorderMoveDown => app.reorder_move_down(),
        ReorderMoveUp => app.reorder_move_up(),
        ReorderSave => app.save_reorder_mode(),
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
        InterfaceMoveUp => app.interface_move_up(),
        InterfaceMoveDown => app.interface_move_down(),
        InterfaceMoveLeft => app.interface_move_left(),
        InterfaceMoveRight => app.interface_move_right(),
        InterfaceSubmit => {
            app.interface_submit()?;
        }
        InterfaceDelete => app.interface_delete(),
        InterfaceRename => app.interface_rename(),
        InterfaceHide => app.interface_hide(),
        InterfaceNextMonth => app.date_interface_next_month(),
        InterfacePrevMonth => app.date_interface_prev_month(),
        InterfaceNextYear => app.date_interface_next_year(),
        InterfacePrevYear => app.date_interface_prev_year(),
        InterfaceToday => app.date_interface_goto_today(),
        Cancel => match &app.input_mode {
            InputMode::Edit(_) => {
                app.clear_hints();
                app.cancel_edit_mode();
            }
            InputMode::Reorder => app.cancel_reorder_mode(),
            InputMode::Selection(_) => app.cancel_selection_mode(),
            InputMode::Prompt(_) => {
                app.clear_hints();
                app.cancel_prompt();
            }
            InputMode::Interface(InterfaceContext::Date(_))
            | InputMode::Interface(InterfaceContext::Project(_))
            | InputMode::Interface(InterfaceContext::Tag(_)) => app.cancel_interface(),
            InputMode::Normal | InputMode::Confirm(_) => {
                if app.help_visible {
                    app.help_visible = false;
                    app.help_scroll = 0;
                } else if matches!(app.view, ViewMode::Filter(_)) {
                    app.cancel_filter();
                }
            }
        },
        NoOp | QuickFilterTag | AppendFavoriteTag | SelectionAppendTag => {}
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
/// - Tab: autocomplete, then add space (unless input ends with `:`)
pub fn handle_prompt_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    let is_command = matches!(
        app.input_mode,
        InputMode::Prompt(PromptContext::Command { .. })
    );

    // Check keymap for cancel action (always) and toggle action (only when empty)
    let context = if is_command {
        KeyContext::CommandPrompt
    } else {
        KeyContext::FilterPrompt
    };
    let spec = KeySpec::from_event(&key);
    if let Some(action) = app.keymap.get(context, &spec) {
        // Cancel always works, toggle only when empty
        let should_dispatch = matches!(action, KeyActionId::Cancel)
            || (app.prompt_is_empty()
                && matches!(
                    action,
                    KeyActionId::ToggleFilterPrompt | KeyActionId::ToggleCommandPrompt
                ));
        if should_dispatch {
            let _ = dispatch_action(app, action);
            return Ok(());
        }
    }

    match key.code {
        KeyCode::Enter => {
            app.accept_hint();
            if app.input_needs_continuation() {
                app.update_hints();
            } else {
                app.clear_hints();
                match &app.input_mode {
                    InputMode::Prompt(PromptContext::Command { .. }) => {
                        app.execute_command()?;
                    }
                    InputMode::Prompt(PromptContext::Filter { .. }) => {
                        app.execute_filter()?;
                    }
                    InputMode::Prompt(PromptContext::RenameTag { old_tag, buffer }) => {
                        let new_tag = buffer.content().trim().to_string();
                        let old_tag = old_tag.clone();
                        app.execute_rename_tag(&old_tag, &new_tag)?;
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Tab => {
            app.accept_hint();
            if !app.input_needs_continuation()
                && let Some(buffer) = app.prompt_buffer_mut()
            {
                buffer.insert_char(' ');
            }
            app.update_hints();
        }
        KeyCode::Backspace if app.prompt_is_empty() => {
            app.clear_hints();
            app.cancel_prompt();
        }
        _ => {
            let is_rename_tag = matches!(
                app.input_mode,
                InputMode::Prompt(PromptContext::RenameTag { .. })
            );

            if let Some(buffer) = app.prompt_buffer_mut() {
                let should_handle = if is_rename_tag {
                    match key.code {
                        KeyCode::Char(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {
                            !buffer.is_empty() || c.is_ascii_alphabetic()
                        }
                        KeyCode::Char(_) => false,
                        _ => true,
                    }
                } else {
                    true
                };

                if should_handle {
                    handle_text_input(buffer, key);
                }
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
                let root = if app.in_git_repo {
                    storage::find_git_root()
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
                } else {
                    std::env::current_dir()?
                };

                let caliber_dir = root.join(".caliber");
                if let Err(e) = std::fs::create_dir_all(&caliber_dir) {
                    app.set_status(format!("Failed to create project: {e}"));
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
                        app.set_status(format!("Failed to create journal directory: {e}"));
                        app.input_mode = InputMode::Normal;
                        return Ok(());
                    }
                    if let Err(e) = std::fs::write(&journal_path, "") {
                        app.set_status(format!("Failed to create journal: {e}"));
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

pub fn handle_interface_key(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match &app.input_mode {
        InputMode::Interface(InterfaceContext::Date(_)) => handle_date_interface(app, key),
        InputMode::Interface(InterfaceContext::Project(_)) => handle_list_interface(
            app,
            key,
            '.',
            KeyContext::ProjectInterface,
            App::project_interface_select,
        ),
        InputMode::Interface(InterfaceContext::Tag(_)) => handle_list_interface(
            app,
            key,
            ',',
            KeyContext::TagInterface,
            App::tag_interface_select,
        ),
        _ => Ok(()),
    }
}

fn handle_date_interface(app: &mut App, key: KeyEvent) -> io::Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('\\') => {
            app.cancel_interface();
        }
        KeyCode::Char(c) if c.is_ascii_digit() || c == '/' => {
            app.date_interface_input_char(c);
        }
        KeyCode::Backspace => {
            if app.date_interface_input_is_empty() {
                app.cancel_interface();
            } else {
                app.date_interface_input_backspace();
            }
        }
        KeyCode::Delete => {
            app.date_interface_input_delete();
        }
        KeyCode::Enter => {
            app.interface_submit()?;
        }
        _ => {
            let spec = KeySpec::from_event(&key);
            if let Some(action) = app.keymap.get(KeyContext::DateInterface, &spec) {
                dispatch_action(app, action)?;
            }
        }
    }
    Ok(())
}

fn handle_list_interface(
    app: &mut App,
    key: KeyEvent,
    toggle_key: char,
    context: KeyContext,
    select_fn: fn(&mut App) -> io::Result<()>,
) -> io::Result<()> {
    match key.code {
        KeyCode::Esc => app.cancel_interface(),
        KeyCode::Char(c) if c == toggle_key => app.cancel_interface(),
        KeyCode::Enter => select_fn(app)?,
        _ => {
            let spec = KeySpec::from_event(&key);
            if let Some(action) = app.keymap.get(context, &spec) {
                dispatch_action(app, action)?;
            }
        }
    }
    Ok(())
}
