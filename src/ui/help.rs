use ratatui::{
    style::{Color, Style},
    text::{Line as RatatuiLine, Span},
};

use crate::dispatch::Keymap;
use crate::registry::{
    COMMANDS, FILTER_SYNTAX, FilterCategory, HelpSection, KeyAction, KeyContext, help_section_keys,
};

use super::shared::format_key_for_display;

const KEY_WIDTH: usize = 14;
const GUTTER_WIDTH: usize = 2;

fn help_section_to_context(section: HelpSection) -> Option<KeyContext> {
    match section {
        HelpSection::Daily => Some(KeyContext::DailyNormal),
        HelpSection::Filter => Some(KeyContext::FilterNormal),
        HelpSection::Edit => Some(KeyContext::Edit),
        HelpSection::Reorder => Some(KeyContext::Reorder),
        HelpSection::Selection => Some(KeyContext::Selection),
        HelpSection::Date => Some(KeyContext::DateInterface),
        HelpSection::Project => Some(KeyContext::ProjectInterface),
        HelpSection::Help => Some(KeyContext::Help),
        HelpSection::Commands | HelpSection::Filters => None,
    }
}

fn build_help_lines(keymap: &Keymap) -> Vec<RatatuiLine<'static>> {
    let header_style = Style::default().fg(Color::Cyan);
    let key_style = Style::default().fg(Color::Yellow);
    let desc_style = Style::default().fg(Color::White);
    let header_indent = " ".repeat(KEY_WIDTH + GUTTER_WIDTH);

    let mut lines = Vec::new();

    let sections = [
        (HelpSection::Daily, "[Daily Mode]"),
        (HelpSection::Filter, "[Filter Mode]"),
        (HelpSection::Edit, "[Edit Mode]"),
        (HelpSection::Reorder, "[Reorder Mode]"),
        (HelpSection::Selection, "[Selection Mode]"),
        (HelpSection::Date, "[Date Interface]"),
        (HelpSection::Project, "[Project Interface]"),
    ];

    for (section, title) in sections {
        let actions: Vec<_> = help_section_keys(section).collect();
        if actions.is_empty() {
            continue;
        }
        let context = help_section_to_context(section);
        lines.push(section_header(title, &header_indent, header_style));
        for action in actions {
            lines.push(help_line_from_action(
                action, keymap, context, key_style, desc_style,
            ));
        }
        lines.push(RatatuiLine::from(""));
    }

    lines.push(section_header("[Commands]", &header_indent, header_style));
    for cmd in COMMANDS.iter() {
        let key_display = format_command_key(cmd.name, None);
        lines.push(help_line(&key_display, cmd.help, key_style, desc_style));
    }
    lines.push(RatatuiLine::from(""));

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
            filter.help,
            key_style,
            desc_style,
        ));
    }
    lines.push(help_line(
        "Dates:",
        "MM/DD, MM/DD/YY, YYYY/MM/DD",
        key_style,
        desc_style,
    ));
    lines.push(help_line(
        "Relative:",
        "today, tomorrow, yesterday, mon..sun, d1..d999 (+ for future)",
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
    keymap: &Keymap,
    context: Option<KeyContext>,
    key_style: Style,
    desc_style: Style,
) -> RatatuiLine<'static> {
    let keys = context
        .map(|ctx| keymap.keys_for_action(ctx, action.id))
        .unwrap_or_default();

    let key_display = if keys.is_empty() {
        match action.default_keys {
            [first, second, ..] => {
                format!(
                    "{}/{}",
                    format_key_for_display(first),
                    format_key_for_display(second)
                )
            }
            [first] => format_key_for_display(first),
            [] => String::new(),
        }
    } else if keys.len() == 1 {
        format_key_for_display(&keys[0])
    } else {
        format!(
            "{}/{}",
            format_key_for_display(&keys[0]),
            format_key_for_display(&keys[1])
        )
    };

    help_line(&key_display, action.help, key_style, desc_style)
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
pub fn get_help_total_lines(keymap: &Keymap) -> usize {
    build_help_lines(keymap).len()
}

pub fn render_help_content(
    keymap: &Keymap,
    scroll: usize,
    visible_height: usize,
) -> Vec<RatatuiLine<'static>> {
    build_help_lines(keymap)
        .into_iter()
        .skip(scroll)
        .take(visible_height)
        .collect()
}
