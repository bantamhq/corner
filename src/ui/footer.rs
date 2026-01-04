use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line as RatatuiLine, Span},
};

use crate::app::{App, InputMode, ViewMode};
use crate::registry::{get_key_action, KeyActionId};

pub fn render_footer(app: &App) -> RatatuiLine<'static> {
    match (&app.view, &app.input_mode) {
        (_, InputMode::Command) => RatatuiLine::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow)),
            Span::raw(app.command_buffer.content().to_string()),
        ]),
        (_, InputMode::QueryInput) => {
            let buffer = match &app.view {
                ViewMode::Filter(state) => state.query_buffer.content(),
                ViewMode::Daily(_) => app.command_buffer.content(),
            };
            RatatuiLine::from(vec![
                Span::styled("/", Style::default().fg(Color::Magenta)),
                Span::raw(buffer.to_string()),
            ])
        }
        (_, InputMode::Edit(_)) => {
            let actions = [
                KeyActionId::SaveEdit,
                KeyActionId::SaveAndNew,
                KeyActionId::CycleEntryType,
                KeyActionId::CancelEdit,
            ];
            build_footer_line(" EDIT ", Color::Green, &actions)
        }
        (_, InputMode::Reorder) => {
            let actions = [
                KeyActionId::ReorderMoveDown,
                KeyActionId::ReorderSave,
                KeyActionId::ReorderCancel,
            ];
            build_footer_line(" REORDER ", Color::Yellow, &actions)
        }
        (_, InputMode::Confirm(_)) => RatatuiLine::from(vec![
            Span::styled(
                " CONFIRM ",
                Style::default().fg(Color::Black).bg(Color::Blue),
            ),
            Span::styled("  y", Style::default().fg(Color::Gray)),
            Span::styled(" Yes  ", Style::default().fg(Color::DarkGray)),
            Span::styled("n/Esc", Style::default().fg(Color::Gray)),
            Span::styled(" No", Style::default().fg(Color::DarkGray)),
        ]),
        (ViewMode::Daily(_), InputMode::Normal) => {
            let actions = [
                KeyActionId::NewEntryBottom,
                KeyActionId::EditEntry,
                KeyActionId::ToggleEntry,
                KeyActionId::EnterFilterMode,
                KeyActionId::ShowHelp,
            ];
            build_footer_line(" DAILY ", Color::Cyan, &actions)
        }
        (ViewMode::Filter(_), InputMode::Normal) => {
            let actions = [
                KeyActionId::ToggleEntry,
                KeyActionId::DeleteEntry,
                KeyActionId::RefreshFilter,
                KeyActionId::ViewEntrySource,
                KeyActionId::ExitFilter,
                KeyActionId::ShowHelp,
            ];
            build_footer_line(" FILTER ", Color::Magenta, &actions)
        }
    }
}

fn build_footer_line(mode_name: &str, color: Color, actions: &[KeyActionId]) -> RatatuiLine<'static> {
    let mut spans = vec![Span::styled(
        mode_name.to_string(),
        Style::default().fg(Color::Black).bg(color),
    )];

    for id in actions {
        let action = get_key_action(*id);
        let key_display = match action.alt_key {
            Some(alt) => format!("{}/{}", action.key, alt),
            None => action.key.to_string(),
        };
        spans.push(Span::styled(
            format!("  {key_display}"),
            Style::default().fg(Color::Gray),
        ));
        spans.push(Span::styled(
            format!(" {}  ", action.short_text),
            Style::default().fg(Color::DarkGray),
        ));
    }

    RatatuiLine::from(spans)
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
