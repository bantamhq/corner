use std::sync::LazyLock;

use ratatui::{
    style::{Color, Style},
    text::{Line as RatatuiLine, Span},
};

use crate::registry::{
    get_key_action, key_actions_for_mode, FilterCategory, KeyActionId, KeyMode, COMMANDS,
    FILTER_SYNTAX,
};

const KEY_WIDTH: usize = 14;
const GUTTER_WIDTH: usize = 2;

static HELP_LINES: LazyLock<Vec<RatatuiLine<'static>>> = LazyLock::new(build_help_lines);

fn build_help_lines() -> Vec<RatatuiLine<'static>> {
    let header_style = Style::default().fg(Color::Cyan);
    let key_style = Style::default().fg(Color::Yellow);
    let desc_style = Style::default().fg(Color::White);
    let header_indent = " ".repeat(KEY_WIDTH + GUTTER_WIDTH);

    let mut lines = Vec::new();

    // [Daily Mode] section
    lines.push(section_header("[Daily Mode]", &header_indent, header_style));
    for action in key_actions_for_mode(KeyMode::DailyNormal) {
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    let shared_daily = [
        KeyActionId::EditEntry,
        KeyActionId::ToggleEntry,
        KeyActionId::DeleteEntry,
        KeyActionId::YankEntry,
        KeyActionId::Undo,
        KeyActionId::MoveDown,
        KeyActionId::MoveUp,
        KeyActionId::JumpToFirst,
        KeyActionId::JumpToLast,
        KeyActionId::QuickFilterTag,
        KeyActionId::EnterFilterMode,
        KeyActionId::ToggleJournal,
        KeyActionId::ShowHelp,
        KeyActionId::EnterCommandMode,
    ];
    for id in shared_daily {
        let action = get_key_action(id);
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

    // [Filter Mode] section
    lines.push(section_header("[Filter Mode]", &header_indent, header_style));
    let filter_nav = [
        KeyActionId::MoveDown,
        KeyActionId::MoveUp,
        KeyActionId::JumpToFirst,
        KeyActionId::JumpToLast,
    ];
    for id in filter_nav {
        let action = get_key_action(id);
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    for action in key_actions_for_mode(KeyMode::FilterNormal) {
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    let filter_shared = [
        KeyActionId::EditEntry,
        KeyActionId::ToggleEntry,
        KeyActionId::DeleteEntry,
        KeyActionId::YankEntry,
        KeyActionId::ViewEntrySource,
        KeyActionId::EnterFilterMode,
        KeyActionId::EnterCommandMode,
        KeyActionId::ShowHelp,
    ];
    for id in filter_shared {
        let action = get_key_action(id);
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

    // [Edit Mode] section
    lines.push(section_header("[Edit Mode]", &header_indent, header_style));
    for action in key_actions_for_mode(KeyMode::Edit) {
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

    // [Reorder Mode] section
    lines.push(section_header("[Reorder Mode]", &header_indent, header_style));
    for action in key_actions_for_mode(KeyMode::Reorder) {
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

    // [Text Editing] section
    lines.push(section_header("[Text Editing]", &header_indent, header_style));
    for action in key_actions_for_mode(KeyMode::TextEditing) {
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

    // [Commands] section
    lines.push(section_header("[Commands]", &header_indent, header_style));
    for cmd in COMMANDS.iter() {
        let key_display = format_command_key(cmd.name, cmd.aliases.first().copied());
        lines.push(help_line(&key_display, cmd.short_description, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

    // [Filter Syntax] section
    lines.push(section_header("[Filter Syntax]", &header_indent, header_style));
    for filter in FILTER_SYNTAX.iter() {
        if filter.category == FilterCategory::TextSearch {
            continue;
        }
        lines.push(help_line(filter.display, filter.short_description, key_style, desc_style));
    }
    lines.push(help_line(
        "DATE:",
        "MM/DD, tomorrow, yesterday, next-mon, last-fri, 3d, -3d",
        key_style,
        desc_style,
    ));

    lines
}

fn section_header(title: &str, indent: &str, style: Style) -> RatatuiLine<'static> {
    RatatuiLine::from(Span::styled(format!("{indent}{title}"), style))
}

fn help_line_from_action(
    action: &crate::registry::KeyAction,
    key_style: Style,
    desc_style: Style,
) -> RatatuiLine<'static> {
    let key_display = match action.alt_key {
        Some(alt) => format!("{}/{}", action.key, alt),
        None => action.key.to_string(),
    };
    help_line(&key_display, action.short_description, key_style, desc_style)
}

fn help_line(key: &str, desc: &str, key_style: Style, desc_style: Style) -> RatatuiLine<'static> {
    RatatuiLine::from(vec![
        Span::styled(
            format!(
                "{:>width$}{}",
                key,
                " ".repeat(GUTTER_WIDTH),
                width = KEY_WIDTH
            ),
            key_style,
        ),
        Span::styled(desc.to_string(), desc_style),
    ])
}

fn format_command_key(name: &str, alias: Option<&str>) -> String {
    match alias {
        Some(a) => format!(":[{}]{}", a, &name[a.len()..]),
        None => format!(":{name}"),
    }
}

#[must_use]
pub fn get_help_total_lines() -> usize {
    HELP_LINES.len()
}

pub fn render_help_content(scroll: usize, visible_height: usize) -> Vec<RatatuiLine<'static>> {
    HELP_LINES
        .iter()
        .skip(scroll)
        .take(visible_height)
        .cloned()
        .collect()
}
