use std::sync::LazyLock;

use ratatui::{
    style::{Color, Style},
    text::{Line as RatatuiLine, Span},
};

use crate::registry::{
    COMMANDS, FILTER_SYNTAX, FilterCategory, HelpSection, KEY_ACTIONS, KeyAction, help_section_keys,
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

    // Daily Mode
    lines.push(section_header("[Daily Mode]", &header_indent, header_style));
    for action in KEY_ACTIONS
        .iter()
        .filter(|a| a.help_sections.contains(&HelpSection::Daily))
    {
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

    // Filter Mode
    lines.push(section_header(
        "[Filter Mode]",
        &header_indent,
        header_style,
    ));
    for action in KEY_ACTIONS
        .iter()
        .filter(|a| a.help_sections.contains(&HelpSection::Filter))
    {
        lines.push(help_line_from_action(action, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

    // Other mode sections
    let other_sections = [
        (HelpSection::Edit, "[Edit Mode]"),
        (HelpSection::Reorder, "[Reorder Mode]"),
        (HelpSection::Selection, "[Selection Mode]"),
        (HelpSection::Date, "[Date Mode]"),
        (HelpSection::TextEditing, "[Text Editing]"),
    ];

    for (section, title) in other_sections {
        let actions: Vec<_> = help_section_keys(section).collect();
        if actions.is_empty() {
            continue;
        }
        lines.push(section_header(title, &header_indent, header_style));
        for action in actions {
            lines.push(help_line_from_action(action, key_style, desc_style));
        }
        lines.push(RatatuiLine::from(""));
    }

    // [Commands] section
    lines.push(section_header("[Commands]", &header_indent, header_style));
    for cmd in COMMANDS.iter() {
        let key_display = format_command_key(cmd.name, None);
        lines.push(help_line(
            &key_display,
            cmd.short_description,
            key_style,
            desc_style,
        ));
    }
    lines.push(RatatuiLine::from(""));

    // [Filter Syntax] section
    lines.push(section_header(
        "[Filter Syntax]",
        &header_indent,
        header_style,
    ));
    for filter in FILTER_SYNTAX.iter() {
        if filter.category == FilterCategory::TextSearch {
            continue;
        }
        lines.push(help_line(
            filter.display,
            filter.short_description,
            key_style,
            desc_style,
        ));
    }
    lines.push(help_line(
        "DATE:",
        "MM/DD, today, tomorrow, yesterday, mon, d7 (+ for future)",
        key_style,
        desc_style,
    ));

    lines
}

fn section_header(title: &str, indent: &str, style: Style) -> RatatuiLine<'static> {
    RatatuiLine::from(Span::styled(format!("{indent}{title}"), style))
}

fn help_line_from_action(
    action: &KeyAction,
    key_style: Style,
    desc_style: Style,
) -> RatatuiLine<'static> {
    let key_display = match action.alt_key {
        Some(alt) => format!("{}/{}", action.key, alt),
        None => action.key.to_string(),
    };
    help_line(
        &key_display,
        action.short_description,
        key_style,
        desc_style,
    )
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
