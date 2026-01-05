use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct KeysFile {
    keys: Vec<KeyDef>,
}

#[derive(Debug, Deserialize)]
struct KeyDef {
    id: String,
    key: String,
    alt_key: Option<String>,
    modes: Vec<String>,
    help_sections: Vec<String>,
    footer: Option<Vec<String>>,
    short_text: String,
    short_description: String,
    long_description: String,
}

#[derive(Debug, Deserialize)]
struct CommandsFile {
    commands: Vec<CommandDef>,
}

#[derive(Debug, Deserialize)]
struct SubArgDef {
    options: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CommandDef {
    name: String,
    aliases: Vec<String>,
    args: Option<String>,
    subargs: Option<Vec<SubArgDef>>,
    key: String,
    help_section: String,
    short_text: String,
    short_description: String,
    long_description: String,
}

#[derive(Debug, Deserialize)]
struct FiltersFile {
    filters: Vec<FilterDef>,
}

#[derive(Debug, Deserialize)]
struct FilterDef {
    syntax: String,
    display: Option<String>,
    aliases: Vec<String>,
    category: String,
    help_section: String,
    short_text: String,
    short_description: String,
    long_description: String,
}

const VALID_HELP_SECTIONS: &[&str] = &[
    "daily",
    "filter",
    "edit",
    "reorder",
    "selection",
    "text_editing",
    "commands",
    "filters",
    "help",
];

const VALID_FOOTER_MODES: &[&str] = &[
    "normal_daily",
    "normal_filter",
    "edit",
    "reorder",
    "selection",
];

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

fn validate_keys(keys: &[KeyDef]) {
    let mut seen_ids = HashSet::new();

    for key in keys {
        // Check for duplicate IDs
        if !seen_ids.insert(&key.id) {
            panic!("Duplicate key ID: {}", key.id);
        }

        // Validate help_sections
        if key.help_sections.is_empty() {
            panic!("Key '{}' must have at least one help_section", key.id);
        }
        for section in &key.help_sections {
            if !VALID_HELP_SECTIONS.contains(&section.as_str()) {
                panic!(
                    "Invalid help_section '{}' for key '{}'. Valid values: {:?}",
                    section, key.id, VALID_HELP_SECTIONS
                );
            }
        }

        // Validate footer modes
        if let Some(footer) = &key.footer {
            for mode in footer {
                if !VALID_FOOTER_MODES.contains(&mode.as_str()) {
                    panic!(
                        "Invalid footer mode '{}' for key '{}'. Valid values: {:?}",
                        mode, key.id, VALID_FOOTER_MODES
                    );
                }
            }
        }
    }
}

fn validate_commands(commands: &[CommandDef]) {
    for cmd in commands {
        if !VALID_HELP_SECTIONS.contains(&cmd.help_section.as_str()) {
            panic!(
                "Invalid help_section '{}' for command '{}'. Valid values: {:?}",
                cmd.help_section, cmd.name, VALID_HELP_SECTIONS
            );
        }
    }
}

fn validate_filters(filters: &[FilterDef]) {
    for filter in filters {
        if !VALID_HELP_SECTIONS.contains(&filter.help_section.as_str()) {
            panic!(
                "Invalid help_section '{}' for filter '{}'. Valid values: {:?}",
                filter.help_section, filter.syntax, VALID_HELP_SECTIONS
            );
        }
    }
}

fn generate_keys_code(keys: &[KeyDef]) -> String {
    let mut code = String::new();

    // Collect unique modes
    let modes: HashSet<&str> = keys
        .iter()
        .flat_map(|k| k.modes.iter().map(|s| s.as_str()))
        .collect();
    let mut modes: Vec<_> = modes.into_iter().collect();
    modes.sort();

    // Generate KeyMode enum
    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum KeyMode {\n");
    for mode in &modes {
        code.push_str(&format!("    {},\n", to_pascal_case(mode)));
    }
    code.push_str("}\n\n");

    // Generate FooterMode enum
    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum FooterMode {\n");
    for mode in VALID_FOOTER_MODES {
        code.push_str(&format!("    {},\n", to_pascal_case(mode)));
    }
    code.push_str("}\n\n");

    // Generate HelpSection enum
    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum HelpSection {\n");
    for section in VALID_HELP_SECTIONS {
        code.push_str(&format!("    {},\n", to_pascal_case(section)));
    }
    code.push_str("}\n\n");

    // Generate KeyActionId enum
    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum KeyActionId {\n");
    for key in keys {
        code.push_str(&format!("    {},\n", to_pascal_case(&key.id)));
    }
    code.push_str("}\n\n");

    // Generate KeyAction struct
    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct KeyAction {\n");
    code.push_str("    pub id: KeyActionId,\n");
    code.push_str("    pub key: &'static str,\n");
    code.push_str("    pub alt_key: Option<&'static str>,\n");
    code.push_str("    pub modes: &'static [KeyMode],\n");
    code.push_str("    pub help_sections: &'static [HelpSection],\n");
    code.push_str("    pub footer_modes: &'static [FooterMode],\n");
    code.push_str("    pub short_text: &'static str,\n");
    code.push_str("    pub short_description: &'static str,\n");
    code.push_str("    pub long_description: &'static str,\n");
    code.push_str("}\n\n");

    // Generate KEY_ACTIONS static
    code.push_str("pub static KEY_ACTIONS: &[KeyAction] = &[\n");
    for key in keys {
        let modes_str: Vec<String> = key
            .modes
            .iter()
            .map(|m| format!("KeyMode::{}", to_pascal_case(m)))
            .collect();
        let alt_key = match &key.alt_key {
            Some(ak) => format!("Some(\"{}\")", ak),
            None => "None".to_string(),
        };
        let help_sections_str: Vec<String> = key
            .help_sections
            .iter()
            .map(|s| format!("HelpSection::{}", to_pascal_case(s)))
            .collect();
        let footer_modes_str: Vec<String> = key
            .footer
            .as_ref()
            .map(|f| {
                f.iter()
                    .map(|m| format!("FooterMode::{}", to_pascal_case(m)))
                    .collect()
            })
            .unwrap_or_default();
        code.push_str(&format!(
            "    KeyAction {{\n        id: KeyActionId::{},\n        key: \"{}\",\n        alt_key: {},\n        modes: &[{}],\n        help_sections: &[{}],\n        footer_modes: &[{}],\n        short_text: \"{}\",\n        short_description: \"{}\",\n        long_description: \"{}\",\n    }},\n",
            to_pascal_case(&key.id),
            key.key,
            alt_key,
            modes_str.join(", "),
            help_sections_str.join(", "),
            footer_modes_str.join(", "),
            key.short_text,
            key.short_description,
            key.long_description
        ));
    }
    code.push_str("];\n\n");

    // Generate helper function to get action by ID
    code.push_str("#[must_use]\n");
    code.push_str("pub fn get_key_action(id: KeyActionId) -> &'static KeyAction {\n");
    code.push_str("    KEY_ACTIONS.iter().find(|a| a.id == id).expect(\"action exists\")\n");
    code.push_str("}\n\n");

    // Generate helper function to get actions for a mode
    code.push_str("pub fn key_actions_for_mode(mode: KeyMode) -> impl Iterator<Item = &'static KeyAction> {\n");
    code.push_str("    KEY_ACTIONS.iter().filter(move |a| a.modes.contains(&mode))\n");
    code.push_str("}\n\n");

    // Generate footer_actions function
    code.push_str(
        "pub fn footer_actions(mode: FooterMode) -> impl Iterator<Item = &'static KeyAction> {\n",
    );
    code.push_str("    KEY_ACTIONS.iter().filter(move |a| a.footer_modes.contains(&mode))\n");
    code.push_str("}\n\n");

    // Generate help_section_keys function
    code.push_str("pub fn help_section_keys(section: HelpSection) -> impl Iterator<Item = &'static KeyAction> {\n");
    code.push_str("    KEY_ACTIONS.iter().filter(move |a| a.help_sections.contains(&section))\n");
    code.push_str("}\n");

    code
}

fn generate_commands_code(commands: &[CommandDef]) -> String {
    let mut code = String::new();

    // Generate SubArg struct
    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct SubArg {\n");
    code.push_str("    pub options: &'static [&'static str],\n");
    code.push_str("}\n\n");

    // Generate Command struct
    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct Command {\n");
    code.push_str("    pub name: &'static str,\n");
    code.push_str("    pub aliases: &'static [&'static str],\n");
    code.push_str("    pub args: Option<&'static str>,\n");
    code.push_str("    pub subargs: &'static [SubArg],\n");
    code.push_str("    pub key: &'static str,\n");
    code.push_str("    pub short_text: &'static str,\n");
    code.push_str("    pub short_description: &'static str,\n");
    code.push_str("    pub long_description: &'static str,\n");
    code.push_str("}\n\n");

    // Generate COMMANDS static
    code.push_str("pub static COMMANDS: &[Command] = &[\n");
    for cmd in commands {
        let aliases_str: Vec<String> = cmd.aliases.iter().map(|a| format!("\"{}\"", a)).collect();
        let args = match &cmd.args {
            Some(a) => format!("Some(\"{}\")", a),
            None => "None".to_string(),
        };
        let subargs = match &cmd.subargs {
            Some(subs) => {
                let subargs_str: Vec<String> = subs
                    .iter()
                    .map(|s| {
                        let opts: Vec<String> =
                            s.options.iter().map(|o| format!("\"{}\"", o)).collect();
                        format!("SubArg {{ options: &[{}] }}", opts.join(", "))
                    })
                    .collect();
                format!("&[{}]", subargs_str.join(", "))
            }
            None => "&[]".to_string(),
        };
        code.push_str(&format!(
            "    Command {{\n        name: \"{}\",\n        aliases: &[{}],\n        args: {},\n        subargs: {},\n        key: \"{}\",\n        short_text: \"{}\",\n        short_description: \"{}\",\n        long_description: \"{}\",\n    }},\n",
            cmd.name,
            aliases_str.join(", "),
            args,
            subargs,
            cmd.key,
            cmd.short_text,
            cmd.short_description,
            cmd.long_description
        ));
    }
    code.push_str("];\n\n");

    // Generate find_command function
    code.push_str("#[must_use]\n");
    code.push_str("pub fn find_command(input: &str) -> Option<&'static Command> {\n");
    code.push_str("    COMMANDS.iter().find(|c| c.name == input || c.aliases.contains(&input))\n");
    code.push_str("}\n\n");

    // Generate commands_matching function for autocomplete
    code.push_str(
        "pub fn commands_matching(prefix: &str) -> impl Iterator<Item = &'static Command> {\n",
    );
    code.push_str("    COMMANDS.iter().filter(move |c| {\n");
    code.push_str(
        "        c.name.starts_with(prefix) || c.aliases.iter().any(|a| a.starts_with(prefix))\n",
    );
    code.push_str("    })\n");
    code.push_str("}\n");

    code
}

fn generate_filters_code(filters: &[FilterDef]) -> String {
    let mut code = String::new();

    // Collect unique categories
    let categories: HashSet<&str> = filters.iter().map(|f| f.category.as_str()).collect();
    let mut categories: Vec<_> = categories.into_iter().collect();
    categories.sort();

    // Generate FilterCategory enum
    code.push_str("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n");
    code.push_str("pub enum FilterCategory {\n");
    for cat in &categories {
        code.push_str(&format!("    {},\n", to_pascal_case(cat)));
    }
    code.push_str("}\n\n");

    // Generate FilterSyntax struct
    code.push_str("#[derive(Clone, Debug, PartialEq)]\n");
    code.push_str("pub struct FilterSyntax {\n");
    code.push_str("    pub syntax: &'static str,\n");
    code.push_str("    pub display: &'static str,\n");
    code.push_str("    pub aliases: &'static [&'static str],\n");
    code.push_str("    pub category: FilterCategory,\n");
    code.push_str("    pub short_text: &'static str,\n");
    code.push_str("    pub short_description: &'static str,\n");
    code.push_str("    pub long_description: &'static str,\n");
    code.push_str("}\n\n");

    // Generate FILTER_SYNTAX static
    code.push_str("pub static FILTER_SYNTAX: &[FilterSyntax] = &[\n");
    for filter in filters {
        let aliases_str: Vec<String> = filter
            .aliases
            .iter()
            .map(|a| format!("\"{}\"", a))
            .collect();
        let display = filter.display.as_deref().unwrap_or(&filter.syntax);
        code.push_str(&format!(
            "    FilterSyntax {{\n        syntax: \"{}\",\n        display: \"{}\",\n        aliases: &[{}],\n        category: FilterCategory::{},\n        short_text: \"{}\",\n        short_description: \"{}\",\n        long_description: \"{}\",\n    }},\n",
            filter.syntax,
            display,
            aliases_str.join(", "),
            to_pascal_case(&filter.category),
            filter.short_text,
            filter.short_description,
            filter.long_description
        ));
    }
    code.push_str("];\n\n");

    // Generate filter_syntax_for_category function
    code.push_str("pub fn filter_syntax_for_category(category: FilterCategory) -> impl Iterator<Item = &'static FilterSyntax> {\n");
    code.push_str("    FILTER_SYNTAX.iter().filter(move |f| f.category == category)\n");
    code.push_str("}\n\n");

    // Generate filter_syntax_matching function for autocomplete
    code.push_str("pub fn filter_syntax_matching(prefix: &str) -> impl Iterator<Item = &'static FilterSyntax> {\n");
    code.push_str("    FILTER_SYNTAX.iter().filter(move |f| {\n");
    code.push_str(
        "        f.syntax.starts_with(prefix) || f.aliases.iter().any(|a| a.starts_with(prefix))\n",
    );
    code.push_str("    })\n");
    code.push_str("}\n");

    code
}

// README generation helpers

fn format_key_display(key: &KeyDef) -> String {
    let format_single = |k: &str| {
        if k == "`" {
            // Escape backtick in markdown
            "`` ` ``".to_string()
        } else {
            format!("`{}`", k)
        }
    };

    match &key.alt_key {
        Some(alt) => format!("{}/{}", format_single(&key.key), format_single(alt)),
        None => format_single(&key.key),
    }
}

fn generate_keys_table_by_section(keys: &[KeyDef], section: &str) -> String {
    let filtered: Vec<_> = keys
        .iter()
        .filter(|k| k.help_sections.contains(&section.to_string()))
        .collect();
    if filtered.is_empty() {
        return String::new();
    }

    let mut table = String::from("| Key | Action |\n|-----|--------|\n");
    for key in filtered {
        table.push_str(&format!(
            "| {} | {} |\n",
            format_key_display(key),
            key.short_description
        ));
    }
    table
}

fn generate_daily_keys_table(keys: &[KeyDef]) -> String {
    let mut table = String::from("| Key | Action |\n|-----|--------|\n");

    // Keys that should appear in Daily section
    for key in keys
        .iter()
        .filter(|k| k.help_sections.contains(&"daily".to_string()))
    {
        table.push_str(&format!(
            "| {} | {} |\n",
            format_key_display(key),
            key.short_description
        ));
    }

    table
}

fn generate_filter_view_table(keys: &[KeyDef]) -> String {
    let mut table = String::from("| Key | Action |\n|-----|--------|\n");

    // Keys that should appear in Filter section
    for key in keys
        .iter()
        .filter(|k| k.help_sections.contains(&"filter".to_string()))
    {
        table.push_str(&format!(
            "| {} | {} |\n",
            format_key_display(key),
            key.short_description
        ));
    }

    table
}

fn generate_commands_table(commands: &[CommandDef]) -> String {
    let mut table = String::from("| Key | Action |\n|-----|--------|\n");

    for cmd in commands {
        let key_display = format_command_display(cmd);
        table.push_str(&format!(
            "| {} | {} |\n",
            key_display, cmd.short_description
        ));
    }

    table
}

fn format_command_display(cmd: &CommandDef) -> String {
    match cmd.aliases.first() {
        Some(alias) => format!("`:[{}]{}`", alias, &cmd.name[alias.len()..]),
        None => format!("`:{}`", cmd.name),
    }
}

fn generate_filter_syntax_table(filters: &[FilterDef]) -> String {
    let mut table = String::from("| Pattern | Matches |\n|---------|---------|");

    for filter in filters {
        // Skip text_search category (just documentation)
        if filter.category == "text_search" {
            continue;
        }

        let display = filter.display.as_deref().unwrap_or(&filter.syntax);
        let pattern = if !filter.aliases.is_empty() {
            format!("`{}` or `{}`", display, filter.aliases[0])
        } else {
            format!("`{}`", display)
        };

        table.push_str(&format!("\n| {} | {} |", pattern, filter.short_description));
    }

    // Add text search pattern
    table.push_str("\n| `word` | Entries containing text |");

    table
}

fn generate_readme(
    manifest_dir: &Path,
    keys: &[KeyDef],
    commands: &[CommandDef],
    filters: &[FilterDef],
) {
    let template_path = manifest_dir.join("README.template.md");
    let readme_path = manifest_dir.join("README.md");

    // Only generate if template exists
    if !template_path.exists() {
        return;
    }

    println!("cargo:rerun-if-changed=README.template.md");

    let template = fs::read_to_string(&template_path).expect("Failed to read README.template.md");

    // Generate tables
    let daily_table = generate_daily_keys_table(keys);
    let reorder_table = generate_keys_table_by_section(keys, "reorder");
    let edit_table = generate_keys_table_by_section(keys, "edit");
    let text_editing_table = generate_keys_table_by_section(keys, "text_editing");
    let filter_table = generate_filter_view_table(keys);
    let commands_table = generate_commands_table(commands);
    let filter_syntax_table = generate_filter_syntax_table(filters);

    // Replace placeholders
    let readme = template
        .replace("<!-- GENERATED:DAILY_KEYS -->", &daily_table)
        .replace("<!-- GENERATED:REORDER_KEYS -->", &reorder_table)
        .replace("<!-- GENERATED:EDIT_KEYS -->", &edit_table)
        .replace("<!-- GENERATED:TEXT_EDITING_KEYS -->", &text_editing_table)
        .replace("<!-- GENERATED:FILTER_KEYS -->", &filter_table)
        .replace("<!-- GENERATED:COMMANDS -->", &commands_table)
        .replace("<!-- GENERATED:FILTER_SYNTAX -->", &filter_syntax_table);

    fs::write(&readme_path, readme).expect("Failed to write README.md");
}

fn main() {
    let manifest_dir_str = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir_str);
    let registry_dir = manifest_dir.join("src/registry");
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("registry_generated.rs");

    // Rerun if TOML files change
    println!("cargo:rerun-if-changed=src/registry/keys.toml");
    println!("cargo:rerun-if-changed=src/registry/commands.toml");
    println!("cargo:rerun-if-changed=src/registry/filters.toml");

    // Read and parse TOML files
    let keys_toml =
        fs::read_to_string(registry_dir.join("keys.toml")).expect("Failed to read keys.toml");
    let commands_toml = fs::read_to_string(registry_dir.join("commands.toml"))
        .expect("Failed to read commands.toml");
    let filters_toml =
        fs::read_to_string(registry_dir.join("filters.toml")).expect("Failed to read filters.toml");

    let keys: KeysFile = toml::from_str(&keys_toml).expect("Failed to parse keys.toml");
    let commands: CommandsFile =
        toml::from_str(&commands_toml).expect("Failed to parse commands.toml");
    let filters: FiltersFile = toml::from_str(&filters_toml).expect("Failed to parse filters.toml");

    // Validate
    validate_keys(&keys.keys);
    validate_commands(&commands.commands);
    validate_filters(&filters.filters);

    // Generate Rust code
    let mut code = String::new();
    code.push_str("// This file is auto-generated by build.rs. Do not edit manually.\n\n");

    code.push_str(&generate_keys_code(&keys.keys));
    code.push('\n');
    code.push_str(&generate_commands_code(&commands.commands));
    code.push('\n');
    code.push_str(&generate_filters_code(&filters.filters));

    // Write generated code
    fs::write(&out_path, code).expect("Failed to write generated code");

    // Generate README
    generate_readme(
        manifest_dir,
        &keys.keys,
        &commands.commands,
        &filters.filters,
    );
}
