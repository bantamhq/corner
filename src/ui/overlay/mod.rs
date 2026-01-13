use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line as RatatuiLine, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{CommandPaletteMode, CommandPaletteState, ConfirmContext};
use crate::registry::{COMMANDS, Command, KeyActionId, KeyContext, get_keys_for_action};

use super::footer::{centered_rect, centered_rect_max};
use super::scroll_indicator::{ScrollIndicatorStyle, scroll_indicator_text};
use super::theme;

pub struct OverlayModel {
    pub confirm: Option<ConfirmModel>,
    pub command_palette: Option<CommandPaletteModel>,
}

pub struct OverlayLayout {
    pub screen_area: Rect,
}

pub struct ConfirmModel {
    pub context: ConfirmContext,
}

pub struct CommandPaletteModel {
    pub mode: CommandPaletteMode,
    pub selected: usize,
}

impl ConfirmModel {
    #[must_use]
    pub fn new(context: ConfirmContext) -> Self {
        Self { context }
    }
}

impl CommandPaletteModel {
    #[must_use]
    pub fn new(state: &CommandPaletteState) -> Self {
        Self {
            mode: state.mode,
            selected: state.selected,
        }
    }
}

pub fn render_confirm_modal(f: &mut Frame<'_>, model: ConfirmModel, area: Rect) {
    let (title, messages): (&str, Vec<String>) = match &model.context {
        ConfirmContext::CreateProjectJournal => (
            " Create Project Journal ",
            vec![
                "No project journal found.".to_string(),
                "Create .caliber/journal.md?".to_string(),
            ],
        ),
        ConfirmContext::DeleteTag(tag) => (
            " Delete Tag ",
            vec![
                format!("Delete all occurrences of #{}?", tag),
                "This cannot be undone.".to_string(),
            ],
        ),
    };

    let popup_area = centered_rect(50, 30, area);
    f.render_widget(Clear, popup_area);

    let confirm_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::CONFIRM_BORDER));

    let inner_area = confirm_block.inner(popup_area);
    f.render_widget(confirm_block, popup_area);

    let mut lines = vec![RatatuiLine::raw("")];
    for msg in messages {
        lines.push(RatatuiLine::raw(msg));
    }
    lines.push(RatatuiLine::raw(""));
    lines.push(RatatuiLine::from(vec![
        Span::styled("[Y]", Style::default().fg(theme::CONFIRM_YES)),
        Span::raw(" Yes    "),
        Span::styled("[N]", Style::default().fg(theme::CONFIRM_NO)),
        Span::raw(" No"),
    ]));
    let content = ratatui::text::Text::from(lines);
    let paragraph = Paragraph::new(content).alignment(Alignment::Center);
    f.render_widget(paragraph, inner_area);
}

fn title_case(input: &str) -> String {
    input
        .split(['_', '-'])
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| {
            let mut chars = chunk.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn filtered_commands(mode: CommandPaletteMode) -> Vec<&'static Command> {
    if mode != CommandPaletteMode::Commands {
        return Vec::new();
    }
    COMMANDS.iter().collect()
}

fn empty_message(mode: CommandPaletteMode) -> &'static str {
    match mode {
        CommandPaletteMode::Commands => "No commands available",
        CommandPaletteMode::Projects => "Projects not available yet",
        CommandPaletteMode::Tags => "Tags not available yet",
    }
}

fn tab_index(mode: CommandPaletteMode) -> usize {
    match mode {
        CommandPaletteMode::Commands => 0,
        CommandPaletteMode::Projects => 1,
        CommandPaletteMode::Tags => 2,
    }
}

fn padded_line(text: &str, width: usize, padding: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let available = width.saturating_sub(padding.saturating_mul(2));
    let trimmed: String = text.chars().take(available).collect();
    let text_len = trimmed.chars().count();
    if width <= padding * 2 {
        return trimmed.chars().take(width).collect();
    }
    let mut line = String::new();
    line.push_str(&" ".repeat(padding));
    line.push_str(&trimmed);
    if available > text_len {
        line.push_str(&" ".repeat(available - text_len));
    }
    line.push_str(&" ".repeat(padding));
    line
}

fn padded_area(area: Rect, padding: u16) -> Rect {
    let width = area.width.saturating_sub(padding.saturating_mul(2));
    Rect {
        x: area.x.saturating_add(padding),
        y: area.y,
        width,
        height: area.height,
    }
}

pub fn render_command_palette(f: &mut Frame<'_>, model: CommandPaletteModel, area: Rect) {
    let popup_area = centered_rect_max(90, 22, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(theme::PALETTE_BORDER));
    let inner_area = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(inner_area);

    let padding = 1u16;
    let tabs_section = layout[0];
    let list_area = layout[1];
    let footer_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(layout[2]);
    let footer_area = padded_area(footer_layout[1], padding);

    let tabs_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(tabs_section);

    let tabs_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(5)])
        .split(padded_area(tabs_layout[0], padding));

    let tab_titles = ["Commands", "Projects", "Tags"];
    let tabs = Tabs::new(tab_titles)
        .select(tab_index(model.mode))
        .style(
            Style::default()
                .fg(theme::PALETTE_TAB_INACTIVE)
                .bg(theme::PALETTE_BG)
                .add_modifier(Modifier::DIM),
        )
        .highlight_style(
            Style::default()
                .fg(theme::PALETTE_TAB_ACTIVE_FG)
                .bg(theme::PALETTE_TAB_ACTIVE_BG)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM),
        )
        .divider("   ")
        .padding("", "");
    f.render_widget(tabs, tabs_row[0]);

    let cancel_key = get_keys_for_action(KeyContext::CommandPalette, KeyActionId::Cancel)
        .first()
        .copied()
        .unwrap_or("esc");
    let cancel_hint = Paragraph::new(RatatuiLine::from(Span::styled(
        cancel_key,
        Style::default()
            .fg(theme::PALETTE_HINT)
            .bg(theme::PALETTE_BG),
    )))
    .alignment(Alignment::Right);
    f.render_widget(cancel_hint, tabs_row[1]);

    let tab_labels = ["Commands", "Projects", "Tags"];

    let divider = "   ";
    let divider_width = divider.width();
    let mut starts = Vec::new();
    let mut cursor = 0usize;
    for (index, label) in tab_labels.iter().enumerate() {
        starts.push(cursor);
        cursor = cursor.saturating_add(label.width());
        if index + 1 < tab_labels.len() {
            cursor = cursor.saturating_add(divider_width);
        }
    }
    let rule_width = tabs_section.width as usize;
    let padding_offset = padding as usize;
    let selected_index = tab_index(model.mode);
    let active_start = padding_offset + starts.get(selected_index).copied().unwrap_or(0);
    let active_width = tab_labels
        .get(selected_index)
        .map(|label| label.width())
        .unwrap_or(0);
    let before_len = active_start.min(rule_width);
    let highlight_len = active_width.min(rule_width.saturating_sub(before_len));
    let after_len = rule_width.saturating_sub(before_len + highlight_len);

    let mut rule_spans = Vec::new();
    if before_len > 0 {
        rule_spans.push(Span::styled(
            "─".repeat(before_len),
            Style::default()
                .fg(theme::PALETTE_TAB_RULE)
                .bg(theme::PALETTE_BG),
        ));
    }
    if highlight_len > 0 {
        rule_spans.push(Span::styled(
            "─".repeat(highlight_len),
            Style::default()
                .fg(theme::PALETTE_TAB_ACTIVE_RULE)
                .bg(theme::PALETTE_BG),
        ));
    }
    if after_len > 0 {
        rule_spans.push(Span::styled(
            "─".repeat(after_len),
            Style::default()
                .fg(theme::PALETTE_TAB_RULE)
                .bg(theme::PALETTE_BG),
        ));
    }
    let rule_line = RatatuiLine::from(rule_spans);
    f.render_widget(Paragraph::new(rule_line), tabs_layout[1]);

    let commands = filtered_commands(model.mode);
    let list_width = list_area.width as usize;
    if list_width == 0 {
        return;
    }
    let padding = 1usize;
    let mut lines = Vec::new();
    let mut selected_line = None;
    let mut current_group = "";

    for (index, command) in commands.iter().enumerate() {
        if command.group != current_group {
            if !lines.is_empty() {
                lines.push(RatatuiLine::raw(""));
            }
            current_group = command.group;
            let group_line = padded_line(command.group, list_width, padding);
            lines.push(RatatuiLine::from(Span::styled(
                group_line,
                Style::default()
                    .fg(theme::PALETTE_GROUP)
                    .bg(theme::PALETTE_BG)
                    .add_modifier(Modifier::BOLD),
            )));
        }

        let is_selected = index == model.selected;
        let selection_style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(theme::PALETTE_COMMAND)
        };
        let desc_style = if is_selected {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::DIM)
        } else {
            Style::default()
                .fg(theme::PALETTE_COMMAND)
                .add_modifier(Modifier::DIM)
        };

        let name_line_index = lines.len();
        if is_selected {
            selected_line = Some(name_line_index);
        }

        let name_line = padded_line(&title_case(command.name), list_width, padding);
        lines.push(RatatuiLine::from(Span::styled(
            name_line,
            selection_style.add_modifier(Modifier::BOLD),
        )));
        let desc_line = padded_line(command.help, list_width, padding);
        lines.push(RatatuiLine::from(Span::styled(desc_line, desc_style)));
    }

    if lines.is_empty() {
        let empty_line = padded_line(empty_message(model.mode), list_width, padding);
        lines.push(RatatuiLine::from(Span::styled(
            empty_line,
            Style::default()
                .fg(theme::PALETTE_DESC)
                .bg(theme::PALETTE_BG),
        )));
    }

    let visible_height = list_area.height as usize;
    if visible_height == 0 {
        return;
    }
    let total_lines = lines.len();
    let mut offset = 0;
    if let Some(selected) = selected_line {
        let selected_end = selected.saturating_add(1);
        if selected_end >= visible_height {
            offset = selected_end + 1 - visible_height;
        }
        if selected < offset {
            offset = selected;
        }
    }
    if total_lines > visible_height {
        offset = offset.min(total_lines.saturating_sub(visible_height));
    }

    let visible_lines: Vec<RatatuiLine<'static>> = lines
        .clone()
        .into_iter()
        .skip(offset)
        .take(visible_height)
        .collect();
    let list = Paragraph::new(visible_lines);
    f.render_widget(list, list_area);

    let footer_content = match model.mode {
        CommandPaletteMode::Commands => {
            let can_scroll_up = offset > 0;
            let can_scroll_down = offset + visible_height < total_lines;
            scroll_indicator_text(
                can_scroll_up,
                can_scroll_down,
                ScrollIndicatorStyle::Labeled,
            )
        }
        CommandPaletteMode::Projects => Some("Project management coming soon".to_string()),
        CommandPaletteMode::Tags => Some("Tag management coming soon".to_string()),
    };

    if let Some(content) = footer_content {
        let footer = Paragraph::new(RatatuiLine::from(Span::styled(
            content,
            Style::default().fg(theme::PALETTE_HINT),
        )))
        .alignment(Alignment::Right);
        f.render_widget(footer, footer_area);
    }
}

pub fn render_overlays(f: &mut Frame<'_>, overlays: OverlayModel, layout: OverlayLayout) {
    if let Some(confirm) = overlays.confirm {
        render_confirm_modal(f, confirm, layout.screen_area);
    }
    if let Some(palette) = overlays.command_palette {
        render_command_palette(f, palette, layout.screen_area);
    }
}
