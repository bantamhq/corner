use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line as RatatuiLine, Span},
};

use crate::app::{App, InputMode, InterfaceContext, PromptContext, ViewMode};
use crate::dispatch::Keymap;
use crate::registry::{FooterMode, KeyAction, KeyContext, footer_actions};

use super::shared::format_key_for_display;

pub fn render_footer(app: &App) -> RatatuiLine<'static> {
    match (&app.view, &app.input_mode) {
        (_, InputMode::Prompt(PromptContext::Command { buffer })) => RatatuiLine::from(vec![
            Span::styled(":", Style::default().fg(Color::Blue)),
            Span::raw(buffer.content().to_string()),
        ]),
        (_, InputMode::Prompt(PromptContext::Filter { buffer })) => RatatuiLine::from(vec![
            Span::styled("/", Style::default().fg(Color::Magenta)),
            Span::raw(buffer.content().to_string()),
        ]),
        (_, InputMode::Edit(_)) => {
            build_footer_line(" EDIT ", Color::Green, FooterMode::Edit, &app.keymap)
        }
        (_, InputMode::Reorder) => {
            build_footer_line(" REORDER ", Color::Green, FooterMode::Reorder, &app.keymap)
        }
        (_, InputMode::Confirm(_)) => RatatuiLine::from(vec![
            Span::styled(
                " CONFIRM ",
                Style::default().fg(Color::Black).bg(Color::Blue),
            ),
            Span::styled("  y", Style::default().fg(Color::Gray)),
            Span::styled(" Yes  ", Style::default().dim()),
            Span::styled("n/Esc", Style::default().fg(Color::Gray)),
            Span::styled(" No", Style::default().dim()),
        ]),
        (_, InputMode::Selection(state)) => {
            let count = state.count();
            build_footer_line(
                &format!(" SELECT ({count}) "),
                Color::Green,
                FooterMode::Selection,
                &app.keymap,
            )
        }
        (_, InputMode::Interface(InterfaceContext::Date(_))) => {
            build_footer_line(" DATE ", Color::Blue, FooterMode::DateInterface, &app.keymap)
        }
        (_, InputMode::Interface(InterfaceContext::Project(_))) => {
            build_footer_line(" PROJECT ", Color::Blue, FooterMode::ProjectInterface, &app.keymap)
        }
        (_, InputMode::Interface(InterfaceContext::Tag(_))) => RatatuiLine::from(vec![
            Span::styled(
                " TAG ",
                Style::default().fg(Color::Black).bg(Color::Blue),
            ),
        ]),
        (ViewMode::Daily(_), InputMode::Normal) => {
            build_footer_line(" DAILY ", Color::Cyan, FooterMode::NormalDaily, &app.keymap)
        }
        (ViewMode::Filter(_), InputMode::Normal) => build_footer_line(
            " FILTER ",
            Color::Magenta,
            FooterMode::NormalFilter,
            &app.keymap,
        ),
    }
}

fn footer_mode_to_context(mode: FooterMode) -> KeyContext {
    match mode {
        FooterMode::NormalDaily => KeyContext::DailyNormal,
        FooterMode::NormalFilter => KeyContext::FilterNormal,
        FooterMode::Edit => KeyContext::Edit,
        FooterMode::Reorder => KeyContext::Reorder,
        FooterMode::Selection => KeyContext::Selection,
        FooterMode::DateInterface => KeyContext::DateInterface,
        FooterMode::ProjectInterface => KeyContext::ProjectInterface,
    }
}

fn build_footer_line(
    mode_name: &str,
    color: Color,
    mode: FooterMode,
    keymap: &Keymap,
) -> RatatuiLine<'static> {
    let mut spans = vec![Span::styled(
        mode_name.to_string(),
        Style::default().fg(Color::Black).bg(color),
    )];

    let context = footer_mode_to_context(mode);

    for action in footer_actions(mode) {
        spans.extend(action_spans(action, keymap, context));
    }

    RatatuiLine::from(spans)
}

fn action_spans(action: &KeyAction, keymap: &Keymap, context: KeyContext) -> [Span<'static>; 2] {
    let keys = keymap.keys_for_action_ordered(context, action.id);

    let key_display = if keys.is_empty() {
        // Fall back to default_keys if no keys bound (shouldn't happen normally)
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

    [
        Span::styled(format!("  {key_display}"), Style::default().fg(Color::Gray)),
        Span::styled(format!(" {} ", action.footer_text), Style::default().dim()),
    ]
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
