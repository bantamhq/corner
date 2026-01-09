use std::collections::{HashMap, HashSet};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::registry::{DEFAULT_KEYMAP, KeyActionId, KeyContext, get_keys_for_action};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySpec {
    pub key: Key,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Esc,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    F(u8),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Debug)]
pub enum KeyParseError {
    Empty,
    UnknownKey(String),
    InvalidModifier(String),
}

impl KeySpec {
    pub fn parse(s: &str) -> Result<Self, KeyParseError> {
        if s.is_empty() {
            return Err(KeyParseError::Empty);
        }

        let mut modifiers = Modifiers::default();
        let mut remaining = s;

        loop {
            if let Some(rest) = remaining.strip_prefix("C-") {
                modifiers.ctrl = true;
                remaining = rest;
            } else if let Some(rest) = remaining.strip_prefix("A-") {
                modifiers.alt = true;
                remaining = rest;
            } else if let Some(rest) = remaining.strip_prefix("S-") {
                modifiers.shift = true;
                remaining = rest;
            } else {
                break;
            }
        }

        // S-tab normalizes to BackTab (what crossterm sends for Shift+Tab)
        if modifiers.shift && remaining.eq_ignore_ascii_case("tab") {
            return Ok(KeySpec {
                key: Key::BackTab,
                modifiers: Modifiers {
                    shift: false,
                    ..modifiers
                },
            });
        }

        // S-<char> normalizes to uppercase/symbol (what crossterm sends for shifted chars)
        if modifiers.shift && remaining.len() == 1 {
            let c = remaining.chars().next().unwrap();
            if let Some(shifted) = shift_char(c) {
                return Ok(KeySpec {
                    key: Key::Char(shifted),
                    modifiers: Modifiers {
                        shift: false,
                        ..modifiers
                    },
                });
            }
        }

        let key = match remaining.to_lowercase().as_str() {
            "ret" | "enter" => Key::Enter,
            "esc" | "escape" => Key::Esc,
            "tab" => Key::Tab,
            "backtab" | "btab" => Key::BackTab,
            "backspace" | "bs" => Key::Backspace,
            "del" | "delete" => Key::Delete,
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            "home" => Key::Home,
            "end" => Key::End,
            "space" => Key::Char(' '),
            s if s.starts_with('f') && s.len() > 1 => {
                if let Ok(n) = s[1..].parse::<u8>() {
                    Key::F(n)
                } else {
                    return Err(KeyParseError::UnknownKey(remaining.to_string()));
                }
            }
            s if s.len() == 1 => Key::Char(remaining.chars().next().unwrap()),
            _ => return Err(KeyParseError::UnknownKey(remaining.to_string())),
        };

        Ok(KeySpec { key, modifiers })
    }

    pub fn from_event(event: &KeyEvent) -> Self {
        let key = match event.code {
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Enter => Key::Enter,
            KeyCode::Esc => Key::Esc,
            KeyCode::Tab => Key::Tab,
            KeyCode::BackTab => Key::BackTab,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::F(n) => Key::F(n),
            _ => Key::Unknown,
        };

        // For printable characters, shift is implicit in the character (O vs o, ! vs 1)
        // so we don't include it in modifiers. This makes "O" in registry match Shift+O.
        let shift = if matches!(key, Key::Char(_)) {
            false
        } else {
            event.modifiers.contains(KeyModifiers::SHIFT)
        };

        let modifiers = Modifiers {
            ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
            alt: event.modifiers.contains(KeyModifiers::ALT),
            shift,
        };

        KeySpec { key, modifiers }
    }

    #[must_use]
    pub fn to_key_string(&self) -> String {
        let mut s = String::new();

        if self.modifiers.ctrl {
            s.push_str("C-");
        }
        if self.modifiers.alt {
            s.push_str("A-");
        }
        if self.modifiers.shift {
            s.push_str("S-");
        }

        match &self.key {
            Key::Char(c) => s.push(*c),
            Key::Enter => s.push_str("ret"),
            Key::Esc => s.push_str("esc"),
            Key::Tab => s.push_str("tab"),
            Key::BackTab => s.push_str("backtab"),
            Key::Backspace => s.push_str("backspace"),
            Key::Delete => s.push_str("del"),
            Key::Up => s.push_str("up"),
            Key::Down => s.push_str("down"),
            Key::Left => s.push_str("left"),
            Key::Right => s.push_str("right"),
            Key::Home => s.push_str("home"),
            Key::End => s.push_str("end"),
            Key::F(n) => s.push_str(&format!("f{}", n)),
            Key::Unknown => s.push_str("unknown"),
        }

        s
    }
}

#[derive(Debug)]
pub enum KeymapError {
    UnknownAction {
        context: String,
        key: String,
        action: String,
    },
    InvalidKey {
        context: String,
        key: String,
        error: KeyParseError,
    },
    DuplicateKey {
        context: String,
        key: String,
    },
}

impl std::fmt::Display for KeymapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeymapError::UnknownAction {
                context,
                key,
                action,
            } => {
                write!(
                    f,
                    "Unknown action '{}' for key '{}' in context '{}'",
                    action, key, context
                )
            }
            KeymapError::InvalidKey { context, key, .. } => {
                write!(f, "Invalid key '{}' in context '{}'", key, context)
            }
            KeymapError::DuplicateKey { context, key } => {
                write!(f, "Duplicate key '{}' in context '{}'", key, context)
            }
        }
    }
}

pub struct Keymap {
    maps: HashMap<KeyContext, HashMap<KeySpec, KeyActionId>>,
    /// Actions that are overridden by config in each context.
    /// If an action appears here, its default keys were not applied.
    overrides: HashMap<KeyContext, HashSet<KeyActionId>>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new(&HashMap::new()).expect("default keymap should be valid")
    }
}

impl Keymap {
    pub fn new(
        config_keys: &HashMap<String, HashMap<String, String>>,
    ) -> Result<Self, KeymapError> {
        let mut maps: HashMap<KeyContext, HashMap<KeySpec, KeyActionId>> = HashMap::new();
        let mut overrides: HashMap<KeyContext, HashSet<KeyActionId>> = HashMap::new();

        for (context_str, key_actions) in config_keys {
            let contexts = parse_contexts(context_str);
            for action_str in key_actions.values() {
                if action_str == "no_op" || action_str.is_empty() {
                    continue;
                }
                if let Some(action_id) = parse_action_id(action_str) {
                    for context in &contexts {
                        overrides.entry(*context).or_default().insert(action_id);
                    }
                }
            }
        }

        for (context, entries) in DEFAULT_KEYMAP {
            let context_map = maps.entry(*context).or_default();
            let context_overrides = overrides.get(context);
            for (key_str, action_id) in *entries {
                if context_overrides.is_some_and(|o| o.contains(action_id)) {
                    continue;
                }
                if let Ok(spec) = KeySpec::parse(key_str) {
                    context_map.insert(spec, *action_id);
                }
            }
        }
        let mut config_keys_added: HashMap<KeyContext, HashMap<KeySpec, String>> = HashMap::new();

        for (context_str, key_actions) in config_keys {
            let contexts = parse_contexts(context_str);
            if contexts.is_empty() {
                continue;
            }

            for (key_str, action_str) in key_actions {
                let spec = KeySpec::parse(key_str).map_err(|e| KeymapError::InvalidKey {
                    context: context_str.clone(),
                    key: key_str.clone(),
                    error: e,
                })?;

                let action_id = if action_str == "no_op" || action_str.is_empty() {
                    KeyActionId::NoOp
                } else {
                    parse_action_id(action_str).ok_or_else(|| KeymapError::UnknownAction {
                        context: context_str.clone(),
                        key: key_str.clone(),
                        action: action_str.clone(),
                    })?
                };

                for context in &contexts {
                    let context_config = config_keys_added.entry(*context).or_default();
                    if context_config.contains_key(&spec) {
                        return Err(KeymapError::DuplicateKey {
                            context: context_str.clone(),
                            key: key_str.clone(),
                        });
                    }
                    context_config.insert(spec.clone(), key_str.clone());
                    maps.entry(*context)
                        .or_default()
                        .insert(spec.clone(), action_id);
                }
            }
        }

        Ok(Keymap { maps, overrides })
    }

    #[must_use]
    pub fn get(&self, context: KeyContext, key: &KeySpec) -> Option<KeyActionId> {
        self.maps.get(&context).and_then(|m| m.get(key).copied())
    }

    /// Get all keys bound to an action in a given context.
    #[must_use]
    pub fn keys_for_action(&self, context: KeyContext, action: KeyActionId) -> Vec<String> {
        self.maps
            .get(&context)
            .map(|m| {
                m.iter()
                    .filter(|(_, a)| **a == action)
                    .map(|(k, _)| k.to_key_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get keys bound to an action in deterministic order for display.
    ///
    /// Order:
    /// 1. Registry default keys (in TOML order) that are still active
    /// 2. Config-defined keys not in defaults, sorted lexicographically
    ///
    /// If the action is overridden by config, only config keys are returned (sorted).
    #[must_use]
    pub fn keys_for_action_ordered(
        &self,
        context: KeyContext,
        action: KeyActionId,
    ) -> Vec<String> {
        let is_overridden = self
            .overrides
            .get(&context)
            .is_some_and(|o| o.contains(&action));

        let active_keys: HashSet<String> = self
            .maps
            .get(&context)
            .map(|m| {
                m.iter()
                    .filter(|(_, a)| **a == action)
                    .map(|(k, _)| k.to_key_string())
                    .collect()
            })
            .unwrap_or_default();

        if is_overridden {
            let mut keys: Vec<String> = active_keys.into_iter().collect();
            keys.sort();
            keys
        } else {
            let default_keys = get_keys_for_action(context, action);
            let mut result: Vec<String> = Vec::new();

            for default_key in default_keys {
                let key_string = KeySpec::parse(default_key)
                    .map(|s| s.to_key_string())
                    .unwrap_or_else(|_| (*default_key).to_string());
                if active_keys.contains(&key_string) {
                    result.push(key_string);
                }
            }

            let default_set: HashSet<String> = result.iter().cloned().collect();
            let mut extras: Vec<String> = active_keys
                .into_iter()
                .filter(|k| !default_set.contains(k))
                .collect();
            extras.sort();
            result.extend(extras);

            result
        }
    }
}

fn parse_context(s: &str) -> Option<KeyContext> {
    match s {
        "daily_normal" => Some(KeyContext::DailyNormal),
        "filter_normal" => Some(KeyContext::FilterNormal),
        "edit" => Some(KeyContext::Edit),
        "reorder" => Some(KeyContext::Reorder),
        "selection" => Some(KeyContext::Selection),
        "help" => Some(KeyContext::Help),
        "date_interface" => Some(KeyContext::DateInterface),
        "project_interface" => Some(KeyContext::ProjectInterface),
        _ => None,
    }
}

fn parse_contexts(s: &str) -> Vec<KeyContext> {
    if s == "shared_normal" {
        vec![KeyContext::DailyNormal, KeyContext::FilterNormal]
    } else {
        parse_context(s).into_iter().collect()
    }
}

fn shift_char(c: char) -> Option<char> {
    match c {
        'a'..='z' => Some(c.to_ascii_uppercase()),
        '1' => Some('!'),
        '2' => Some('@'),
        '3' => Some('#'),
        '4' => Some('$'),
        '5' => Some('%'),
        '6' => Some('^'),
        '7' => Some('&'),
        '8' => Some('*'),
        '9' => Some('('),
        '0' => Some(')'),
        '-' => Some('_'),
        '=' => Some('+'),
        '[' => Some('{'),
        ']' => Some('}'),
        '\\' => Some('|'),
        ';' => Some(':'),
        '\'' => Some('"'),
        ',' => Some('<'),
        '.' => Some('>'),
        '/' => Some('?'),
        '`' => Some('~'),
        _ => None,
    }
}

fn parse_action_id(s: &str) -> Option<KeyActionId> {
    match s {
        "new_entry_bottom" => Some(KeyActionId::NewEntryBottom),
        "new_entry_below" => Some(KeyActionId::NewEntryBelow),
        "new_entry_above" => Some(KeyActionId::NewEntryAbove),
        "edit_entry" => Some(KeyActionId::EditEntry),
        "toggle_entry" => Some(KeyActionId::ToggleEntry),
        "delete_entry" => Some(KeyActionId::DeleteEntry),
        "yank_entry" => Some(KeyActionId::YankEntry),
        "paste" => Some(KeyActionId::Paste),
        "undo" => Some(KeyActionId::Undo),
        "redo" => Some(KeyActionId::Redo),
        "remove_last_tag" => Some(KeyActionId::RemoveLastTag),
        "remove_all_tags" => Some(KeyActionId::RemoveAllTags),
        "move_down" => Some(KeyActionId::MoveDown),
        "move_up" => Some(KeyActionId::MoveUp),
        "jump_to_first" => Some(KeyActionId::JumpToFirst),
        "jump_to_last" => Some(KeyActionId::JumpToLast),
        "prev_day" => Some(KeyActionId::PrevDay),
        "next_day" => Some(KeyActionId::NextDay),
        "goto_today" => Some(KeyActionId::GotoToday),
        "toggle_hide_completed" => Some(KeyActionId::ToggleHideCompleted),
        "tidy_entries" => Some(KeyActionId::TidyEntries),
        "enter_reorder_mode" => Some(KeyActionId::EnterReorderMode),
        "enter_selection_mode" => Some(KeyActionId::EnterSelectionMode),
        // quick_filter_tag, append_favorite_tag: not remappable (digit determines tag)
        "cycle_entry_type_normal" => Some(KeyActionId::CycleEntryTypeNormal),
        "return_to_filter" => Some(KeyActionId::ReturnToFilter),
        "toggle_journal" => Some(KeyActionId::ToggleJournal),
        "enter_filter_mode" => Some(KeyActionId::EnterFilterMode),
        "enter_command_mode" => Some(KeyActionId::EnterCommandMode),
        "show_help" => Some(KeyActionId::ShowHelp),
        "open_date_interface" => Some(KeyActionId::OpenDateInterface),
        "open_project_interface" => Some(KeyActionId::OpenProjectInterface),
        "filter_quick_add" => Some(KeyActionId::FilterQuickAdd),
        "refresh_filter" => Some(KeyActionId::RefreshFilter),
        "exit_filter" => Some(KeyActionId::ExitFilter),
        "save_edit" => Some(KeyActionId::SaveEdit),
        "save_and_new" => Some(KeyActionId::SaveAndNew),
        "cycle_entry_type" => Some(KeyActionId::CycleEntryType),
        "cancel_edit" => Some(KeyActionId::CancelEdit),
        "reorder_move_down" => Some(KeyActionId::ReorderMoveDown),
        "reorder_move_up" => Some(KeyActionId::ReorderMoveUp),
        "reorder_save" => Some(KeyActionId::ReorderSave),
        "reorder_cancel" => Some(KeyActionId::ReorderCancel),
        "selection_toggle" => Some(KeyActionId::SelectionToggle),
        "selection_extend_range" => Some(KeyActionId::SelectionExtendRange),
        "selection_move_down" => Some(KeyActionId::SelectionMoveDown),
        "selection_move_up" => Some(KeyActionId::SelectionMoveUp),
        "selection_delete" => Some(KeyActionId::SelectionDelete),
        "selection_toggle_complete" => Some(KeyActionId::SelectionToggleComplete),
        "selection_yank" => Some(KeyActionId::SelectionYank),
        "selection_remove_last_tag" => Some(KeyActionId::SelectionRemoveLastTag),
        "selection_remove_all_tags" => Some(KeyActionId::SelectionRemoveAllTags),
        // selection_append_tag: not remappable (digit determines tag)
        "selection_cycle_type" => Some(KeyActionId::SelectionCycleType),
        "selection_exit" => Some(KeyActionId::SelectionExit),
        "help_scroll_down" => Some(KeyActionId::HelpScrollDown),
        "help_scroll_up" => Some(KeyActionId::HelpScrollUp),
        "close_help" => Some(KeyActionId::CloseHelp),
        "date_interface_move_left" => Some(KeyActionId::DateInterfaceMoveLeft),
        "date_interface_move_right" => Some(KeyActionId::DateInterfaceMoveRight),
        "date_interface_move_up" => Some(KeyActionId::DateInterfaceMoveUp),
        "date_interface_move_down" => Some(KeyActionId::DateInterfaceMoveDown),
        "date_interface_prev_month" => Some(KeyActionId::DateInterfacePrevMonth),
        "date_interface_next_month" => Some(KeyActionId::DateInterfaceNextMonth),
        "date_interface_prev_year" => Some(KeyActionId::DateInterfacePrevYear),
        "date_interface_next_year" => Some(KeyActionId::DateInterfaceNextYear),
        "date_interface_today" => Some(KeyActionId::DateInterfaceToday),
        "date_interface_confirm" => Some(KeyActionId::DateInterfaceConfirm),
        "date_interface_cancel" => Some(KeyActionId::DateInterfaceCancel),
        "project_interface_move_up" => Some(KeyActionId::ProjectInterfaceMoveUp),
        "project_interface_move_down" => Some(KeyActionId::ProjectInterfaceMoveDown),
        "project_interface_select" => Some(KeyActionId::ProjectInterfaceSelect),
        "project_interface_cancel" => Some(KeyActionId::ProjectInterfaceCancel),
        "no_op" => Some(KeyActionId::NoOp),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_char() {
        let spec = KeySpec::parse("j").unwrap();
        assert_eq!(spec.key, Key::Char('j'));
        assert!(!spec.modifiers.ctrl);
        assert!(!spec.modifiers.alt);
        assert!(!spec.modifiers.shift);
    }

    #[test]
    fn test_parse_ctrl_modifier() {
        let spec = KeySpec::parse("C-j").unwrap();
        assert_eq!(spec.key, Key::Char('j'));
        assert!(spec.modifiers.ctrl);
    }

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(KeySpec::parse("ret").unwrap().key, Key::Enter);
        assert_eq!(KeySpec::parse("esc").unwrap().key, Key::Esc);
        assert_eq!(KeySpec::parse("tab").unwrap().key, Key::Tab);
        assert_eq!(KeySpec::parse("up").unwrap().key, Key::Up);
        assert_eq!(KeySpec::parse("down").unwrap().key, Key::Down);
    }

    #[test]
    fn test_parse_shift_tab_normalizes_to_backtab() {
        let spec = KeySpec::parse("S-tab").unwrap();
        assert_eq!(spec.key, Key::BackTab);
        assert!(!spec.modifiers.shift);
    }

    #[test]
    fn test_uppercase_char_no_shift_modifier() {
        let spec = KeySpec::parse("O").unwrap();
        assert_eq!(spec.key, Key::Char('O'));
        assert!(!spec.modifiers.shift);

        // Simulate Shift+O from crossterm
        let event = KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT);
        let from_event = KeySpec::from_event(&event);
        assert_eq!(from_event, spec);
    }

    #[test]
    fn test_shifted_symbol_no_shift_modifier() {
        let spec = KeySpec::parse("!").unwrap();
        assert_eq!(spec.key, Key::Char('!'));
        assert!(!spec.modifiers.shift);

        // Simulate Shift+1 → ! from crossterm
        let event = KeyEvent::new(KeyCode::Char('!'), KeyModifiers::SHIFT);
        let from_event = KeySpec::from_event(&event);
        assert_eq!(from_event, spec);
    }

    #[test]
    fn test_keymap_default() {
        let keymap = Keymap::default();
        let spec = KeySpec::parse("j").unwrap();
        let action = keymap.get(KeyContext::DailyNormal, &spec);
        assert_eq!(action, Some(KeyActionId::MoveDown));
    }

    #[test]
    fn test_parse_shift_char_normalizes_to_uppercase() {
        let spec = KeySpec::parse("S-a").unwrap();
        assert_eq!(spec.key, Key::Char('A'));
        assert!(!spec.modifiers.shift);

        let event = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
        let from_event = KeySpec::from_event(&event);
        assert_eq!(from_event, spec);
    }

    #[test]
    fn test_parse_shift_number_normalizes_to_symbol() {
        let spec = KeySpec::parse("S-1").unwrap();
        assert_eq!(spec.key, Key::Char('!'));
        assert!(!spec.modifiers.shift);
    }

    #[test]
    fn test_config_override_replaces_all_defaults() {
        let default_keys = get_keys_for_action(KeyContext::DailyNormal, KeyActionId::MoveDown);
        assert!(!default_keys.is_empty());

        let mut config = HashMap::new();
        let mut daily_keys = HashMap::new();
        daily_keys.insert("n".to_string(), "move_down".to_string());
        config.insert("daily_normal".to_string(), daily_keys);

        let keymap = Keymap::new(&config).unwrap();

        for default_key in default_keys {
            let spec = KeySpec::parse(default_key).unwrap();
            assert_eq!(keymap.get(KeyContext::DailyNormal, &spec), None);
        }

        let n_spec = KeySpec::parse("n").unwrap();
        assert_eq!(
            keymap.get(KeyContext::DailyNormal, &n_spec),
            Some(KeyActionId::MoveDown)
        );
    }

    #[test]
    fn test_config_override_multiple_keys_same_action() {
        let mut config = HashMap::new();
        let mut daily_keys = HashMap::new();
        daily_keys.insert("n".to_string(), "move_down".to_string());
        daily_keys.insert("m".to_string(), "move_down".to_string());
        config.insert("daily_normal".to_string(), daily_keys);

        let keymap = Keymap::new(&config).unwrap();

        let n_spec = KeySpec::parse("n").unwrap();
        let m_spec = KeySpec::parse("m").unwrap();
        assert_eq!(
            keymap.get(KeyContext::DailyNormal, &n_spec),
            Some(KeyActionId::MoveDown)
        );
        assert_eq!(
            keymap.get(KeyContext::DailyNormal, &m_spec),
            Some(KeyActionId::MoveDown)
        );

        let default_keys = get_keys_for_action(KeyContext::DailyNormal, KeyActionId::MoveDown);
        for default_key in default_keys {
            let spec = KeySpec::parse(default_key).unwrap();
            assert_eq!(keymap.get(KeyContext::DailyNormal, &spec), None);
        }
    }

    #[test]
    fn test_ordered_keys_matches_registry_order() {
        let keymap = Keymap::default();

        let registry_keys = get_keys_for_action(KeyContext::DailyNormal, KeyActionId::MoveDown);
        let ordered_keys =
            keymap.keys_for_action_ordered(KeyContext::DailyNormal, KeyActionId::MoveDown);

        let expected: Vec<String> = registry_keys
            .iter()
            .map(|k| {
                KeySpec::parse(k)
                    .map(|s| s.to_key_string())
                    .unwrap_or_else(|_| (*k).to_string())
            })
            .collect();
        assert_eq!(ordered_keys, expected);
    }

    #[test]
    fn test_ordered_keys_config_sorted() {
        let mut config = HashMap::new();
        let mut daily_keys = HashMap::new();
        daily_keys.insert("z".to_string(), "move_down".to_string());
        daily_keys.insert("a".to_string(), "move_down".to_string());
        config.insert("daily_normal".to_string(), daily_keys);

        let keymap = Keymap::new(&config).unwrap();

        let keys = keymap.keys_for_action_ordered(KeyContext::DailyNormal, KeyActionId::MoveDown);
        assert_eq!(keys, vec!["a", "z"]);
    }

    #[test]
    fn test_no_op_does_not_override() {
        let mut config = HashMap::new();
        let mut daily_keys = HashMap::new();
        daily_keys.insert("x".to_string(), "no_op".to_string());
        config.insert("daily_normal".to_string(), daily_keys);

        let keymap = Keymap::new(&config).unwrap();

        let default_keys = get_keys_for_action(KeyContext::DailyNormal, KeyActionId::MoveDown);
        for default_key in default_keys {
            let spec = KeySpec::parse(default_key).unwrap();
            assert_eq!(
                keymap.get(KeyContext::DailyNormal, &spec),
                Some(KeyActionId::MoveDown)
            );
        }
    }

    #[test]
    fn test_unrelated_actions_keep_defaults() {
        let mut config = HashMap::new();
        let mut daily_keys = HashMap::new();
        daily_keys.insert("n".to_string(), "move_down".to_string());
        config.insert("daily_normal".to_string(), daily_keys);

        let keymap = Keymap::new(&config).unwrap();

        let default_keys = get_keys_for_action(KeyContext::DailyNormal, KeyActionId::MoveUp);
        for default_key in default_keys {
            let spec = KeySpec::parse(default_key).unwrap();
            assert_eq!(
                keymap.get(KeyContext::DailyNormal, &spec),
                Some(KeyActionId::MoveUp)
            );
        }
    }
}
