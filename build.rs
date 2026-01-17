use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct ActionsFile {
    action: Vec<ActionDef>,
}

#[derive(Debug, Deserialize)]
struct ActionDef {
    key_action_id: String,
    default_keys: Vec<String>,
    contexts: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CommandsFile {
    command: Vec<CommandDef>,
}

#[derive(Debug, Deserialize)]
struct SubArgDef {
    options: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CommandDef {
    name: String,
    #[serde(default)]
    group: Option<String>,
    args: Option<String>,
    subargs: Option<Vec<SubArgDef>>,
    help: String,
}

#[derive(Debug, Deserialize)]
struct FiltersFile {
    filter: Vec<FilterDef>,
}

#[derive(Debug, Deserialize)]
struct FilterDef {
    syntax: String,
    display: Option<String>,
    category: String,
    help: String,
}

#[derive(Debug, Deserialize)]
struct DateValuesFile {
    date_value: Vec<DateValueDef>,
}

#[derive(Debug, Deserialize)]
struct DateValueDef {
    syntax: String,
    display: String,
    scopes: Vec<String>,
    #[serde(default)]
    values: Option<Vec<String>>,
    #[serde(default)]
    pattern: Option<String>,
    help: String,
    readme: String,
    completion_hint: String,
}

#[derive(Debug, Deserialize)]
struct HelpFile {
    help: Vec<HelpDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    Single(String),
    Multiple(Vec<String>),
}

impl StringOrVec {
    fn to_vec(&self) -> Vec<String> {
        match self {
            Self::Single(s) => vec![s.clone()],
            Self::Multiple(v) => v.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct HelpDef {
    actions: StringOrVec,
    description: String,
    category: String,
}

#[derive(Debug, Deserialize)]
struct HelpEntriesFile {
    help_entry: Vec<HelpEntryDef>,
}

#[derive(Debug, Deserialize)]
struct HelpEntryDef {
    section: String,
    key: String,
    description: String,
}

const VALID_CONTEXTS: &[&str] = &[
    "shared_normal",
    "daily_normal",
    "filter_normal",
    "edit",
    "reorder",
    "selection",
    "command_palette",
];

const VALID_DATE_SCOPES: &[&str] = &["entry", "filter"];

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect()
}

/// Format a key spec for README display (e.g., "down" -> "↓", "ret" -> "Enter")
fn format_key_for_readme(key: &str) -> String {
    // Handle modifier prefixes
    let mut modifiers = Vec::new();
    let mut remaining = key;

    loop {
        if let Some(rest) = remaining.strip_prefix("C-") {
            modifiers.push("Ctrl");
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("A-") {
            modifiers.push("Alt");
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("S-") {
            modifiers.push("Shift");
            remaining = rest;
        } else {
            break;
        }
    }

    // Map the base key (preserve original case for single chars)
    let base = match remaining.to_lowercase().as_str() {
        "ret" | "enter" => "Enter".to_string(),
        "esc" | "escape" => "Esc".to_string(),
        "tab" => "Tab".to_string(),
        "backtab" | "btab" => "Shift+Tab".to_string(),
        "backspace" | "bs" => "Backspace".to_string(),
        "del" | "delete" => "Delete".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "PgUp".to_string(),
        "pagedown" => "PgDn".to_string(),
        "space" => "Space".to_string(),
        _ => {
            // For single characters and other keys, preserve original case
            if modifiers.is_empty() {
                format!("`{remaining}`")
            } else {
                return format!("{}+`{}`", modifiers.join("+"), remaining);
            }
        }
    };

    if modifiers.is_empty() {
        base
    } else {
        format!("{}+{}", modifiers.join("+"), base)
    }
}

/// Format keys for multiple related actions (e.g., move_up + move_down)
/// For paired actions, formats as "key1 / key2" showing one key per action
fn format_action_keys_for_readme(action_ids: &[String], actions: &[ActionDef]) -> String {
    if action_ids.len() == 1 {
        // Single action: show all its keys
        if let Some(action) = actions.iter().find(|a| a.key_action_id == action_ids[0]) {
            return format_keys_grouped(&action.default_keys);
        }
        return String::new();
    }

    // Multiple actions (paired like move_up/move_down): show first key of each action
    let mut primary_keys: Vec<String> = Vec::new();
    for action_id in action_ids {
        if let Some(action) = actions.iter().find(|a| a.key_action_id == *action_id)
            && let Some(first_key) = action.default_keys.first()
        {
            primary_keys.push(format_key_for_readme(first_key));
        }
    }

    // Group arrows together, letters together
    let arrows: Vec<_> = primary_keys
        .iter()
        .filter(|k| k.contains('↑') || k.contains('↓') || k.contains('←') || k.contains('→'))
        .cloned()
        .collect();
    let others: Vec<_> = primary_keys
        .iter()
        .filter(|k| !k.contains('↑') && !k.contains('↓') && !k.contains('←') && !k.contains('→'))
        .cloned()
        .collect();

    match (arrows.is_empty(), others.is_empty()) {
        (true, true) => String::new(),
        (true, false) => others.join(" / "),
        (false, true) => arrows.join("/"),
        (false, false) => format!("{} / {}", arrows.join("/"), others.join("/")),
    }
}

/// Format a list of keys, grouping arrows and letters
fn format_keys_grouped(keys: &[String]) -> String {
    let formatted: Vec<String> = keys.iter().map(|k| format_key_for_readme(k)).collect();

    let arrows: Vec<_> = formatted
        .iter()
        .filter(|k| k.contains('↑') || k.contains('↓') || k.contains('←') || k.contains('→'))
        .cloned()
        .collect();
    let others: Vec<_> = formatted
        .iter()
        .filter(|k| !k.contains('↑') && !k.contains('↓') && !k.contains('←') && !k.contains('→'))
        .cloned()
        .collect();

    match (arrows.is_empty(), others.is_empty()) {
        (true, true) => String::new(),
        (true, false) => others.join(" / "),
        (false, true) => arrows.join("/"),
        (false, false) => format!("{} / {}", arrows.join("/"), others.join("/")),
    }
}

fn expand_contexts(contexts: &[String]) -> Vec<String> {
    let mut expanded = Vec::new();
    for ctx in contexts {
        if ctx == "shared_normal" {
            expanded.push("daily_normal".to_string());
            expanded.push("filter_normal".to_string());
        } else {
            expanded.push(ctx.clone());
        }
    }
    expanded
}

fn is_valid_key_spec(s: &str) -> bool {
    // Documentation-only patterns (not actually parseable key specs)
    // These are display-only keys for help/footer that represent key ranges or combos
    const DOC_ONLY_KEYS: &[&str] = &["0-9", "S-0-9", "[]", "y/Y"];
    if DOC_ONLY_KEYS.contains(&s) {
        return true;
    }

    // Simple key spec validation
    let mut remaining = s;
    loop {
        if let Some(rest) = remaining.strip_prefix("C-") {
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("A-") {
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("S-") {
            remaining = rest;
        } else {
            break;
        }
    }

    matches!(
        remaining.to_lowercase().as_str(),
        "ret"
            | "enter"
            | "esc"
            | "escape"
            | "tab"
            | "backtab"
            | "btab"
            | "backspace"
            | "bs"
            | "del"
            | "delete"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
            | "space"
    ) || remaining.len() == 1
        || (remaining.starts_with('f')
            && remaining.len() > 1
            && remaining[1..].parse::<u8>().is_ok())
}

fn validate_actions(actions: &[ActionDef]) {
    let mut seen_ids = HashSet::new();

    for action in actions {
        if !seen_ids.insert(&action.key_action_id) {
            panic!("Duplicate action ID: {}", action.key_action_id);
        }

        for key in &action.default_keys {
            if !is_valid_key_spec(key) {
                panic!(
                    "Invalid key '{}' for action '{}'. Key must be a valid key spec.",
                    key, action.key_action_id
                );
            }
        }

        for ctx in &action.contexts {
            if !VALID_CONTEXTS.contains(&ctx.as_str()) {
                panic!(
                    "Invalid context '{}' for action '{}'. Valid values: {:?}",
                    ctx, action.key_action_id, VALID_CONTEXTS
                );
            }
        }
    }
}

fn validate_date_values(date_values: &[DateValueDef]) {
    for dv in date_values {
        for scope in &dv.scopes {
            if !VALID_DATE_SCOPES.contains(&scope.as_str()) {
                panic!(
                    "Invalid scope '{}' for date_value '{}'. Valid values: {:?}",
                    scope, dv.syntax, VALID_DATE_SCOPES
                );
            }
        }
    }
}

fn generate_actions_code(actions: &[ActionDef]) -> String {
    let mut code = String::new();

    let expanded_contexts: Vec<String> = actions
        .iter()
        .flat_map(|a| expand_contexts(&a.contexts))
        .collect();
    let contexts: HashSet<&str> = expanded_contexts.iter().map(|s| s.as_str()).collect();
    let mut contexts: Vec<_> = contexts.into_iter().collect();
    contexts.sort();

    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum KeyContext {\n");
    for ctx in &contexts {
        code.push_str(&format!("    {},\n", to_pascal_case(ctx)));
    }
    code.push_str("}\n\n");

    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum KeyActionId {\n");
    for action in actions {
        code.push_str(&format!("    {},\n", to_pascal_case(&action.key_action_id)));
    }
    code.push_str("    NoOp,\n");
    code.push_str("}\n\n");

    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct KeyAction {\n");
    code.push_str("    pub id: KeyActionId,\n");
    code.push_str("    pub default_keys: &'static [&'static str],\n");
    code.push_str("    pub contexts: &'static [KeyContext],\n");
    code.push_str("}\n\n");

    code.push_str("pub static KEY_ACTIONS: &[KeyAction] = &[\n");
    for action in actions {
        let keys_str: Vec<String> = action
            .default_keys
            .iter()
            .map(|k| format!("r#\"{}\"#", k))
            .collect();

        let expanded = expand_contexts(&action.contexts);
        let contexts_str: Vec<String> = expanded
            .iter()
            .map(|c| format!("KeyContext::{}", to_pascal_case(c)))
            .collect();

        code.push_str(&format!(
            "    KeyAction {{\n        id: KeyActionId::{},\n        default_keys: &[{}],\n        contexts: &[{}],\n    }},\n",
            to_pascal_case(&action.key_action_id),
            keys_str.join(", "),
            contexts_str.join(", ")
        ));
    }
    code.push_str("];\n\n");

    code.push_str("#[must_use]\n");
    code.push_str("pub fn get_key_action(id: KeyActionId) -> &'static KeyAction {\n");
    code.push_str("    KEY_ACTIONS.iter().find(|a| a.id == id).expect(\"action exists\")\n");
    code.push_str("}\n\n");

    code.push_str("pub fn key_actions_for_context(context: KeyContext) -> impl Iterator<Item = &'static KeyAction> {\n");
    code.push_str("    KEY_ACTIONS.iter().filter(move |a| a.contexts.contains(&context))\n");
    code.push_str("}\n\n");

    let mut keymap: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for action in actions {
        let expanded = expand_contexts(&action.contexts);
        for ctx in expanded {
            for key in &action.default_keys {
                keymap
                    .entry(ctx.clone())
                    .or_default()
                    .push((key.clone(), action.key_action_id.clone()));
            }
        }
    }

    code.push_str("pub static DEFAULT_KEYMAP: &[(KeyContext, &[(&str, KeyActionId)])] = &[\n");
    let mut sorted_contexts: Vec<_> = keymap.keys().collect();
    sorted_contexts.sort();
    for ctx in sorted_contexts {
        let entries = keymap.get(ctx).unwrap();
        let entries_str: Vec<String> = entries
            .iter()
            .map(|(k, a)| format!("(r#\"{}\"#, KeyActionId::{})", k, to_pascal_case(a)))
            .collect();
        code.push_str(&format!(
            "    (KeyContext::{}, &[{}]),\n",
            to_pascal_case(ctx),
            entries_str.join(", ")
        ));
    }
    code.push_str("];\n\n");

    let mut action_to_keys: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    for action in actions {
        let expanded = expand_contexts(&action.contexts);
        for ctx in expanded {
            action_to_keys
                .entry(ctx.clone())
                .or_default()
                .entry(action.key_action_id.clone())
                .or_default()
                .extend(action.default_keys.iter().cloned());
        }
    }

    code.push_str("#[allow(clippy::type_complexity)]\n");
    code.push_str("pub static ACTION_TO_KEYS: &[(KeyContext, &[(KeyActionId, &[&str])])] = &[\n");
    let mut sorted_contexts: Vec<_> = action_to_keys.keys().collect();
    sorted_contexts.sort();
    for ctx in sorted_contexts {
        let actions_map = action_to_keys.get(ctx).unwrap();
        let mut entries: Vec<String> = Vec::new();
        let mut sorted_actions: Vec<_> = actions_map.keys().collect();
        sorted_actions.sort();
        for action_id in sorted_actions {
            let keys = actions_map.get(action_id).unwrap();
            let keys_str: Vec<String> = keys.iter().map(|k| format!("r#\"{}\"#", k)).collect();
            entries.push(format!(
                "(KeyActionId::{}, &[{}])",
                to_pascal_case(action_id),
                keys_str.join(", ")
            ));
        }
        code.push_str(&format!(
            "    (KeyContext::{}, &[{}]),\n",
            to_pascal_case(ctx),
            entries.join(", ")
        ));
    }
    code.push_str("];\n\n");

    code.push_str("#[must_use]\n");
    code.push_str(
        "pub fn get_keys_for_action(context: KeyContext, action: KeyActionId) -> &'static [&'static str] {\n",
    );
    code.push_str("    ACTION_TO_KEYS\n");
    code.push_str("        .iter()\n");
    code.push_str("        .find(|(ctx, _)| *ctx == context)\n");
    code.push_str("        .and_then(|(_, actions)| actions.iter().find(|(a, _)| *a == action))\n");
    code.push_str("        .map(|(_, keys)| *keys)\n");
    code.push_str("        .unwrap_or(&[])\n");
    code.push_str("}\n\n");

    code.push_str("#[must_use]\n");
    code.push_str(
        "pub fn get_default_action(context: KeyContext, key: &str) -> Option<KeyActionId> {\n",
    );
    code.push_str("    DEFAULT_KEYMAP\n");
    code.push_str("        .iter()\n");
    code.push_str("        .find(|(ctx, _)| *ctx == context)\n");
    code.push_str("        .and_then(|(_, keys)| keys.iter().find(|(k, _)| *k == key))\n");
    code.push_str("        .map(|(_, action)| *action)\n");
    code.push_str("}\n");

    code
}

fn generate_commands_code(commands: &[CommandDef]) -> String {
    let mut code = String::new();

    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct SubArg {\n");
    code.push_str("    pub options: &'static [&'static str],\n");
    code.push_str("}\n\n");

    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct Command {\n");
    code.push_str("    pub name: &'static str,\n");
    code.push_str("    pub group: &'static str,\n");
    code.push_str("    pub args: Option<&'static str>,\n");
    code.push_str("    pub subargs: &'static [SubArg],\n");
    code.push_str("    pub help: &'static str,\n");
    code.push_str("}\n\n");

    code.push_str("pub static COMMANDS: &[Command] = &[\n");
    for cmd in commands {
        let args = match &cmd.args {
            Some(a) => format!("Some(r#\"{}\"#)", a),
            None => "None".to_string(),
        };
        let subargs = match &cmd.subargs {
            Some(subs) => {
                let subargs_str: Vec<String> = subs
                    .iter()
                    .map(|s| {
                        let opts: Vec<String> =
                            s.options.iter().map(|o| format!("r#\"{}\"#", o)).collect();
                        format!("SubArg {{ options: &[{}] }}", opts.join(", "))
                    })
                    .collect();
                format!("&[{}]", subargs_str.join(", "))
            }
            None => "&[]".to_string(),
        };
        let group = cmd.group.as_deref().unwrap_or("Commands");
        code.push_str(&format!(
            "    Command {{\n        name: r#\"{}\"#,\n        group: r#\"{}\"#,\n        args: {},\n        subargs: {},\n        help: r#\"{}\"#,\n    }},\n",
            cmd.name, group, args, subargs, cmd.help
        ));
    }
    code.push_str("];\n\n");

    code.push_str("#[must_use]\n");
    code.push_str("pub fn find_command(input: &str) -> Option<&'static Command> {\n");
    code.push_str("    COMMANDS.iter().find(|c| c.name == input)\n");
    code.push_str("}\n\n");

    code.push_str(
        "pub fn commands_matching(prefix: &str) -> impl Iterator<Item = &'static Command> {\n",
    );
    code.push_str("    COMMANDS.iter().filter(move |c| c.name.starts_with(prefix))\n");
    code.push_str("}\n");

    code
}

fn generate_filters_code(filters: &[FilterDef]) -> String {
    let mut code = String::new();

    let categories: HashSet<&str> = filters.iter().map(|f| f.category.as_str()).collect();
    let mut categories: Vec<_> = categories.into_iter().collect();
    categories.sort();

    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum FilterCategory {\n");
    for cat in &categories {
        code.push_str(&format!("    {},\n", to_pascal_case(cat)));
    }
    code.push_str("}\n\n");

    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct FilterSyntax {\n");
    code.push_str("    pub syntax: &'static str,\n");
    code.push_str("    pub display: &'static str,\n");
    code.push_str("    pub category: FilterCategory,\n");
    code.push_str("    pub help: &'static str,\n");
    code.push_str("}\n\n");

    code.push_str("pub static FILTER_SYNTAX: &[FilterSyntax] = &[\n");
    for filter in filters {
        let display = filter.display.as_deref().unwrap_or(&filter.syntax);
        code.push_str(&format!(
            "    FilterSyntax {{\n        syntax: r#\"{}\"#,\n        display: r#\"{}\"#,\n        category: FilterCategory::{},\n        help: r#\"{}\"#,\n    }},\n",
            filter.syntax,
            display,
            to_pascal_case(&filter.category),
            filter.help
        ));
    }
    code.push_str("];\n\n");

    code.push_str("pub fn filter_syntax_for_category(category: FilterCategory) -> impl Iterator<Item = &'static FilterSyntax> {\n");
    code.push_str("    FILTER_SYNTAX.iter().filter(move |f| f.category == category)\n");
    code.push_str("}\n\n");

    code.push_str("pub fn filter_syntax_matching(prefix: &str) -> impl Iterator<Item = &'static FilterSyntax> {\n");
    code.push_str("    FILTER_SYNTAX.iter().filter(move |f| f.syntax.starts_with(prefix))\n");
    code.push_str("}\n");

    code
}

fn generate_date_values_code(date_values: &[DateValueDef]) -> String {
    let mut code = String::new();

    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum DateScope {\n");
    code.push_str("    Entry,\n");
    code.push_str("    Filter,\n");
    code.push_str("}\n\n");

    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct DateValue {\n");
    code.push_str("    pub syntax: &'static str,\n");
    code.push_str("    pub display: &'static str,\n");
    code.push_str("    pub scopes: &'static [DateScope],\n");
    code.push_str("    /// Enumerated valid values (for grouped hints like weekdays)\n");
    code.push_str("    pub values: Option<&'static [&'static str]>,\n");
    code.push_str("    /// Regex pattern for validation (for pattern-based hints like d[1-999])\n");
    code.push_str("    pub pattern: Option<&'static str>,\n");
    code.push_str("    pub help: &'static str,\n");
    code.push_str("    pub readme: &'static str,\n");
    code.push_str("    pub completion_hint: &'static str,\n");
    code.push_str("}\n\n");

    code.push_str("pub static DATE_VALUES: &[DateValue] = &[\n");
    for dv in date_values {
        let scopes_str: Vec<String> = dv
            .scopes
            .iter()
            .map(|s| format!("DateScope::{}", to_pascal_case(s)))
            .collect();
        let values_str = match &dv.values {
            Some(vals) => {
                let quoted: Vec<String> = vals.iter().map(|v| format!(r#""{v}""#)).collect();
                format!("Some(&[{}])", quoted.join(", "))
            }
            None => "None".to_string(),
        };
        let pattern_str = match &dv.pattern {
            Some(p) => format!("Some(r#\"{}\"#)", p),
            None => "None".to_string(),
        };
        code.push_str(&format!(
            "    DateValue {{\n        syntax: r#\"{}\"#,\n        display: r#\"{}\"#,\n        scopes: &[{}],\n        values: {},\n        pattern: {},\n        help: r#\"{}\"#,\n        readme: r#\"{}\"#,\n        completion_hint: r#\"{}\"#,\n    }},\n",
            dv.syntax,
            dv.display,
            scopes_str.join(", "),
            values_str,
            pattern_str,
            dv.help,
            dv.readme,
            dv.completion_hint
        ));
    }
    code.push_str("];\n\n");

    code.push_str("pub fn date_values_for_scope(scope: DateScope) -> impl Iterator<Item = &'static DateValue> {\n");
    code.push_str("    DATE_VALUES.iter().filter(move |d| d.scopes.contains(&scope))\n");
    code.push_str("}\n");

    code
}

/// Generate a markdown table for a help category
fn generate_help_table(helps: &[HelpDef], actions: &[ActionDef], category: &str) -> String {
    let mut table = String::from("| Key | Action |\n|-----|--------|\n");

    for help in helps.iter().filter(|h| h.category == category) {
        let action_ids = help.actions.to_vec();
        let keys = format_action_keys_for_readme(&action_ids, actions);
        if !keys.is_empty() {
            table.push_str(&format!("| {} | {} |\n", keys, help.description));
        }
    }

    table
}

/// Generate a markdown table for help_entries (documentation-only)
fn generate_help_entries_table(entries: &[HelpEntryDef], section: &str) -> String {
    let mut table = String::from("| Pattern | Matches |\n|---------|---------|");

    for entry in entries.iter().filter(|e| e.section == section) {
        table.push_str(&format!("\n| {} | {} |", entry.key, entry.description));
    }

    table
}

fn generate_readme(
    manifest_dir: &Path,
    actions: &[ActionDef],
    helps: &[HelpDef],
    help_entries: &[HelpEntryDef],
) {
    let template_path = manifest_dir.join("docs/templates/README.template.md");
    let readme_path = manifest_dir.join("README.md");

    if !template_path.exists() {
        return;
    }

    println!("cargo:rerun-if-changed=docs/templates/README.template.md");

    let template = fs::read_to_string(&template_path).expect("Failed to read README.template.md");

    // Generate tables from help.toml categories
    let navigation_table = generate_help_table(helps, actions, "navigation");
    let entries_table = generate_help_table(helps, actions, "entries");
    let clipboard_table = generate_help_table(helps, actions, "clipboard");
    let tags_table = generate_help_table(helps, actions, "tags");
    let views_table = generate_help_table(helps, actions, "views");
    let daily_table = generate_help_table(helps, actions, "daily");
    let filter_table = generate_help_table(helps, actions, "filter");
    let edit_table = generate_help_table(helps, actions, "edit");
    let selection_table = generate_help_table(helps, actions, "selection");
    let general_table = generate_help_table(helps, actions, "general");

    // Generate tables from help_entries.toml sections
    let filter_syntax_table = generate_help_entries_table(help_entries, "filter_syntax");
    let date_syntax_table = generate_help_entries_table(help_entries, "date_syntax");

    let readme = template
        .replace("<!-- GENERATED:NAVIGATION -->", &navigation_table)
        .replace("<!-- GENERATED:ENTRIES -->", &entries_table)
        .replace("<!-- GENERATED:CLIPBOARD -->", &clipboard_table)
        .replace("<!-- GENERATED:TAGS -->", &tags_table)
        .replace("<!-- GENERATED:VIEWS -->", &views_table)
        .replace("<!-- GENERATED:DAILY -->", &daily_table)
        .replace("<!-- GENERATED:FILTER -->", &filter_table)
        .replace("<!-- GENERATED:EDIT -->", &edit_table)
        .replace("<!-- GENERATED:SELECTION -->", &selection_table)
        .replace("<!-- GENERATED:GENERAL -->", &general_table)
        .replace("<!-- GENERATED:FILTER_SYNTAX -->", &filter_syntax_table)
        .replace("<!-- GENERATED:DATE_SYNTAX -->", &date_syntax_table);

    let readme = format!(
        "<!-- AUTO-GENERATED FILE. DO NOT EDIT DIRECTLY. Edit /docs/templates/README.template.md instead. -->\n\n{}",
        readme
    );

    fs::write(&readme_path, readme).expect("Failed to write README.md");
}

fn main() {
    let manifest_dir_str = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir_str);
    let registry_dir = manifest_dir.join("src/registry");
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("registry_generated.rs");

    println!("cargo:rerun-if-changed=src/registry/actions.toml");
    println!("cargo:rerun-if-changed=src/registry/commands.toml");
    println!("cargo:rerun-if-changed=src/registry/filters.toml");
    println!("cargo:rerun-if-changed=src/registry/date_values.toml");
    println!("cargo:rerun-if-changed=src/registry/help.toml");
    println!("cargo:rerun-if-changed=src/registry/help_entries.toml");

    let actions_toml =
        fs::read_to_string(registry_dir.join("actions.toml")).expect("Failed to read actions.toml");
    let commands_toml = fs::read_to_string(registry_dir.join("commands.toml"))
        .expect("Failed to read commands.toml");
    let filters_toml =
        fs::read_to_string(registry_dir.join("filters.toml")).expect("Failed to read filters.toml");
    let date_values_toml = fs::read_to_string(registry_dir.join("date_values.toml"))
        .expect("Failed to read date_values.toml");
    let help_toml =
        fs::read_to_string(registry_dir.join("help.toml")).expect("Failed to read help.toml");
    let help_entries_toml = fs::read_to_string(registry_dir.join("help_entries.toml"))
        .expect("Failed to read help_entries.toml");

    let actions: ActionsFile = toml::from_str(&actions_toml).expect("Failed to parse actions.toml");
    let commands: CommandsFile =
        toml::from_str(&commands_toml).expect("Failed to parse commands.toml");
    let filters: FiltersFile = toml::from_str(&filters_toml).expect("Failed to parse filters.toml");
    let date_values: DateValuesFile =
        toml::from_str(&date_values_toml).expect("Failed to parse date_values.toml");
    let helps: HelpFile = toml::from_str(&help_toml).expect("Failed to parse help.toml");
    let help_entries: HelpEntriesFile =
        toml::from_str(&help_entries_toml).expect("Failed to parse help_entries.toml");

    validate_actions(&actions.action);
    validate_date_values(&date_values.date_value);

    let mut code = String::new();

    code.push_str(&generate_actions_code(&actions.action));
    code.push('\n');
    code.push_str(&generate_commands_code(&commands.command));
    code.push('\n');
    code.push_str(&generate_filters_code(&filters.filter));
    code.push('\n');
    code.push_str(&generate_date_values_code(&date_values.date_value));

    fs::write(&out_path, code).expect("Failed to write generated code");

    generate_readme(
        manifest_dir,
        &actions.action,
        &helps.help,
        &help_entries.help_entry,
    );
}
