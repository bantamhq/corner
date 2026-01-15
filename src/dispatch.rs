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
    PageUp,
    PageDown,
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
            "pageup" => Key::PageUp,
            "pagedown" => Key::PageDown,
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
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::F(n) => Key::F(n),
            _ => Key::Unknown,
        };

        // For printable characters, shift is implicit in the character (O vs o, ! vs 1)
        // so we don't include it in modifiers. This makes "O" in registry match Shift+O.
        // For BackTab, crossterm sends SHIFT+BackTab but we normalize to just BackTab.
        let shift = if matches!(key, Key::Char(_) | Key::BackTab) {
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
            Key::PageUp => s.push_str("pageup"),
            Key::PageDown => s.push_str("pagedown"),
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
    pub fn keys_for_action_ordered(&self, context: KeyContext, action: KeyActionId) -> Vec<String> {
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
        "submit" => Some(KeyActionId::Submit),
        "cancel" => Some(KeyActionId::Cancel),
        "move_down" => Some(KeyActionId::MoveDown),
        "move_up" => Some(KeyActionId::MoveUp),
        "move_left" => Some(KeyActionId::MoveLeft),
        "move_right" => Some(KeyActionId::MoveRight),
        "jump_to_first" => Some(KeyActionId::JumpToFirst),
        "jump_to_last" => Some(KeyActionId::JumpToLast),
        "prev_week" => Some(KeyActionId::PrevWeek),
        "next_week" => Some(KeyActionId::NextWeek),
        "prev_month" => Some(KeyActionId::PrevMonth),
        "next_month" => Some(KeyActionId::NextMonth),
        "prev_year" => Some(KeyActionId::PrevYear),
        "next_year" => Some(KeyActionId::NextYear),
        "goto_today" => Some(KeyActionId::GotoToday),
        "new_entry_below" => Some(KeyActionId::NewEntryBelow),
        "new_entry_above" => Some(KeyActionId::NewEntryAbove),
        "edit" => Some(KeyActionId::Edit),
        "toggle_complete" => Some(KeyActionId::ToggleComplete),
        "delete" => Some(KeyActionId::Delete),
        "defer_date" => Some(KeyActionId::DeferDate),
        "remove_date" => Some(KeyActionId::RemoveDate),
        "yank" => Some(KeyActionId::Yank),
        "paste" => Some(KeyActionId::Paste),
        "undo" => Some(KeyActionId::Undo),
        "redo" => Some(KeyActionId::Redo),
        "remove_last_tag" => Some(KeyActionId::RemoveLastTag),
        "remove_all_tags" => Some(KeyActionId::RemoveAllTags),
        "cycle_entry_type" => Some(KeyActionId::CycleEntryType),
        "selection" => Some(KeyActionId::Selection),
        "selection_extend_range" => Some(KeyActionId::SelectionExtendRange),
        "toggle_filter_view" => Some(KeyActionId::ToggleFilterView),
        "filter_prompt" => Some(KeyActionId::FilterPrompt),
        "toggle_journal" => Some(KeyActionId::ToggleJournal),
        "filter_quick_add" => Some(KeyActionId::FilterQuickAdd),
        "refresh" => Some(KeyActionId::Refresh),
        "save_and_new" => Some(KeyActionId::SaveAndNew),
        "reorder_mode" => Some(KeyActionId::ReorderMode),
        "tidy_entries" => Some(KeyActionId::TidyEntries),
        "hide" => Some(KeyActionId::Hide),
        "autocomplete" => Some(KeyActionId::Autocomplete),
        "toggle_calendar_sidebar" => Some(KeyActionId::ToggleCalendarSidebar),
        "toggle_agenda" => Some(KeyActionId::ToggleAgenda),
        "date_picker" => Some(KeyActionId::DatePicker),
        "quit" => Some(KeyActionId::Quit),
        "no_op" => Some(KeyActionId::NoOp),
        _ => None,
    }
}
