use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line as RatatuiLine, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
};
use unicode_width::UnicodeWidthStr;

use super::surface::Surface;
use crate::app::{CommandPaletteMode, CommandPaletteState, ConfirmContext, TagInfo};
use crate::registry::{COMMANDS, Command, KeyActionId, KeyContext, get_keys_for_action};
use crate::storage::ProjectRegistry;

use super::layout::{centered_rect, centered_rect_max};
use super::scroll_indicator::{ScrollIndicatorStyle, scroll_indicator_text};
use super::theme;

pub struct OverlayModel {
    pub confirm: Option<ConfirmModel>,
    pub command_palette: Option<CommandPaletteModel>,
    pub date_picker: Option<DatePickerModel>,
}

pub struct DatePickerModel {
    pub buffer: String,
    pub cursor_pos: usize,
}

pub struct OverlayLayout<'a> {
    pub screen_area: Rect,
    pub surface: &'a Surface,
}

pub struct ConfirmModel {
    pub context: ConfirmContext,
}

pub struct CommandPaletteModel {
    pub mode: CommandPaletteMode,
    pub selected: usize,
    pub projects: Vec<PaletteProject>,
    pub tags: Vec<PaletteTag>,
}

pub struct PaletteProject {
    pub name: String,
    pub path: String,
    pub available: bool,
    pub is_current: bool,
}

pub struct PaletteTag {
    pub name: String,
    pub count: usize,
}

impl ConfirmModel {
    #[must_use]
    pub fn new(context: ConfirmContext) -> Self {
        Self { context }
    }
}

impl CommandPaletteModel {
    #[must_use]
    pub fn new(
        state: &CommandPaletteState,
        tags: &[TagInfo],
        current_project_path: Option<&std::path::Path>,
    ) -> Self {
        let registry = ProjectRegistry::load();
        let projects = registry
            .projects
            .iter()
            .filter(|p| !p.hide_from_registry)
            .map(|p| {
                let is_current = current_project_path
                    .map(|cp| {
                        cp.starts_with(&p.root) || p.root.starts_with(cp.parent().unwrap_or(cp))
                    })
                    .unwrap_or(false);
                PaletteProject {
                    name: p.name.clone(),
                    path: p.root.display().to_string(),
                    available: p.available,
                    is_current,
                }
            })
            .collect();

        let tags = tags
            .iter()
            .map(|t| PaletteTag {
                name: t.name.clone(),
                count: t.count,
            })
            .collect();

        Self {
            mode: state.mode,
            selected: state.selected,
            projects,
            tags,
        }
    }
}

pub fn render_confirm_modal(f: &mut Frame<'_>, area: Rect, model: ConfirmModel) {
    let (title, messages): (&str, Vec<String>) = match &model.context {
        ConfirmContext::CreateProjectJournal => (
            theme::TITLE_CREATE_PROJECT,
            vec![
                theme::MSG_NO_PROJECT_JOURNAL.to_string(),
                theme::MSG_CREATE_PROJECT_JOURNAL.to_string(),
            ],
        ),
        ConfirmContext::DeleteTag(tag) => (
            theme::TITLE_DELETE_TAG,
            vec![
                format!("Delete all occurrences of #{}?", tag),
                theme::LABEL_CANNOT_UNDO.to_string(),
            ],
        ),
        ConfirmContext::DeleteTagFromCompleted(tag) => (
            theme::TITLE_REMOVE_FROM_COMPLETED,
            vec![
                format!("Remove #{} from completed tasks?", tag),
                theme::LABEL_CANNOT_UNDO.to_string(),
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
        Span::styled(
            theme::LABEL_CONFIRM_YES,
            Style::default().fg(theme::CONFIRM_YES),
        ),
        Span::raw(theme::LABEL_YES),
        Span::styled(
            theme::LABEL_CONFIRM_NO,
            Style::default().fg(theme::CONFIRM_NO),
        ),
        Span::raw(theme::LABEL_NO),
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
        CommandPaletteMode::Commands => theme::LABEL_EMPTY_COMMANDS,
        CommandPaletteMode::Projects => theme::LABEL_EMPTY_PROJECTS,
        CommandPaletteMode::Tags => theme::LABEL_EMPTY_TAGS,
    }
}

fn tab_index(mode: CommandPaletteMode) -> usize {
    match mode {
        CommandPaletteMode::Commands => 0,
        CommandPaletteMode::Projects => 1,
        CommandPaletteMode::Tags => 2,
    }
}

fn item_styles(is_selected: bool, is_available: bool, bg: Color, muted: Color) -> (Style, Style) {
    let base_modifier = if is_available {
        Modifier::empty()
    } else {
        Modifier::DIM
    };

    let name_style = if is_selected {
        Style::default()
            .bg(bg)
            .add_modifier(Modifier::REVERSED | Modifier::BOLD | base_modifier)
    } else {
        Style::default()
            .fg(theme::CALENDAR_TEXT)
            .bg(bg)
            .add_modifier(Modifier::BOLD | base_modifier)
    };

    let desc_style = if is_selected {
        Style::default()
            .bg(bg)
            .add_modifier(Modifier::REVERSED | base_modifier)
    } else {
        Style::default()
            .fg(muted)
            .bg(bg)
            .add_modifier(base_modifier)
    };

    (name_style, desc_style)
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

struct PaletteItem<'a> {
    name: &'a str,
    description: &'a str,
    is_selected: bool,
    is_available: bool,
}

fn build_palette_item_line(
    item: PaletteItem<'_>,
    list_width: usize,
    padding: usize,
    bg: Color,
    muted: Color,
) -> RatatuiLine<'static> {
    let (name_style, desc_style) = item_styles(item.is_selected, item.is_available, bg, muted);
    let available = list_width.saturating_sub(padding * 2);
    let name_width = item.name.len();
    let desc_width = item.description.len();
    let gap = available.saturating_sub(name_width + desc_width);

    RatatuiLine::from(vec![
        Span::styled(format!("{}{}", " ".repeat(padding), item.name), name_style),
        Span::styled(
            " ".repeat(gap),
            if item.is_selected {
                Style::default().bg(bg).add_modifier(Modifier::REVERSED)
            } else {
                Style::default().bg(bg)
            },
        ),
        Span::styled(
            format!("{}{}", item.description, " ".repeat(padding)),
            desc_style.remove_modifier(Modifier::BOLD),
        ),
    ])
}

pub fn render_command_palette(
    f: &mut Frame<'_>,
    area: Rect,
    model: CommandPaletteModel,
    surface: &Surface,
) {
    let popup_area = centered_rect_max(90, 22, area);
    f.render_widget(Clear, popup_area);

    let bg = theme::panel_bg(surface);
    let block = Block::default().style(Style::default().bg(bg));
    let inner_area = Rect {
        x: popup_area.x + 1,
        y: popup_area.y + 1,
        width: popup_area.width.saturating_sub(2),
        height: popup_area.height.saturating_sub(2),
    };
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

    let tab_titles = [
        theme::LABEL_TAB_COMMANDS,
        theme::LABEL_TAB_PROJECTS,
        theme::LABEL_TAB_TAGS,
    ];
    let tabs = Tabs::new(tab_titles)
        .select(tab_index(model.mode))
        .style(
            Style::default()
                .fg(theme::secondary_text(surface))
                .bg(bg)
                .add_modifier(Modifier::DIM),
        )
        .highlight_style(
            Style::default()
                .fg(theme::CALENDAR_TEXT)
                .bg(bg)
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
        Style::default().fg(theme::secondary_text(surface)).bg(bg),
    )))
    .alignment(Alignment::Right);
    f.render_widget(cancel_hint, tabs_row[1]);

    let tab_labels = [
        theme::LABEL_TAB_COMMANDS,
        theme::LABEL_TAB_PROJECTS,
        theme::LABEL_TAB_TAGS,
    ];

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
            Style::default().fg(theme::panel_rule(surface)).bg(bg),
        ));
    }
    if highlight_len > 0 {
        rule_spans.push(Span::styled(
            "─".repeat(highlight_len),
            Style::default().fg(theme::PALETTE_ACCENT).bg(bg),
        ));
    }
    if after_len > 0 {
        rule_spans.push(Span::styled(
            "─".repeat(after_len),
            Style::default().fg(theme::panel_rule(surface)).bg(bg),
        ));
    }
    let rule_line = RatatuiLine::from(rule_spans);
    f.render_widget(Paragraph::new(rule_line), tabs_layout[1]);

    let list_width = list_area.width as usize;
    if list_width == 0 {
        return;
    }
    let padding = 1usize;
    let mut lines = Vec::new();
    let mut selected_line = None;

    let muted = theme::secondary_text(surface);
    let header_style = Style::default()
        .fg(theme::PALETTE_ACCENT)
        .bg(bg)
        .add_modifier(Modifier::BOLD);

    match model.mode {
        CommandPaletteMode::Commands => {
            let commands = filtered_commands(model.mode);
            let mut current_group = "";

            for (index, command) in commands.iter().enumerate() {
                if command.group != current_group {
                    if !lines.is_empty() {
                        lines.push(RatatuiLine::styled(
                            " ".repeat(list_width),
                            Style::default().bg(bg),
                        ));
                    }
                    current_group = command.group;
                    let group_line = padded_line(command.group, list_width, padding);
                    lines.push(RatatuiLine::from(Span::styled(group_line, header_style)));
                }

                let is_selected = index == model.selected;
                if is_selected {
                    selected_line = Some(lines.len());
                }

                let name = title_case(command.name);
                lines.push(build_palette_item_line(
                    PaletteItem {
                        name: &name,
                        description: command.help,
                        is_selected,
                        is_available: true,
                    },
                    list_width,
                    padding,
                    bg,
                    muted,
                ));
            }
        }
        CommandPaletteMode::Projects => {
            let current_project = model.projects.iter().position(|p| p.is_current);
            let other_projects: Vec<_> = model
                .projects
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.is_current)
                .collect();

            let header_line = padded_line("Current Project", list_width, padding);
            lines.push(RatatuiLine::from(Span::styled(header_line, header_style)));

            if let Some(idx) = current_project {
                let project = &model.projects[idx];
                let is_selected = idx == model.selected;
                if is_selected {
                    selected_line = Some(lines.len());
                }
                lines.push(build_palette_item_line(
                    PaletteItem {
                        name: &project.name,
                        description: &project.path,
                        is_selected,
                        is_available: project.available,
                    },
                    list_width,
                    padding,
                    bg,
                    muted,
                ));
            } else {
                let empty_line = padded_line("No project loaded", list_width, padding);
                lines.push(RatatuiLine::from(Span::styled(
                    empty_line,
                    Style::default().fg(muted).bg(bg),
                )));
            }

            lines.push(RatatuiLine::styled(
                " ".repeat(list_width),
                Style::default().bg(bg),
            ));

            let header_line = padded_line("Additional Projects", list_width, padding);
            lines.push(RatatuiLine::from(Span::styled(header_line, header_style)));

            if other_projects.is_empty() {
                let empty_line = padded_line("No additional projects", list_width, padding);
                lines.push(RatatuiLine::from(Span::styled(
                    empty_line,
                    Style::default().fg(muted).bg(bg),
                )));
            } else {
                for (index, project) in other_projects {
                    let is_selected = index == model.selected;
                    if is_selected {
                        selected_line = Some(lines.len());
                    }
                    lines.push(build_palette_item_line(
                        PaletteItem {
                            name: &project.name,
                            description: &project.path,
                            is_selected,
                            is_available: project.available,
                        },
                        list_width,
                        padding,
                        bg,
                        muted,
                    ));
                }
            }
        }
        CommandPaletteMode::Tags => {
            let header_line = padded_line("All Tags", list_width, padding);
            lines.push(RatatuiLine::from(Span::styled(header_line, header_style)));

            if model.tags.is_empty() {
                let empty_line = padded_line(theme::LABEL_EMPTY_TAGS, list_width, padding);
                lines.push(RatatuiLine::from(Span::styled(
                    empty_line,
                    Style::default().fg(muted).bg(bg),
                )));
            } else {
                for (index, tag) in model.tags.iter().enumerate() {
                    let is_selected = index == model.selected;
                    if is_selected {
                        selected_line = Some(lines.len());
                    }

                    let tag_name = format!("#{}", tag.name);
                    let count_str = format!("({})", tag.count);
                    lines.push(build_palette_item_line(
                        PaletteItem {
                            name: &tag_name,
                            description: &count_str,
                            is_selected,
                            is_available: true,
                        },
                        list_width,
                        padding,
                        bg,
                        muted,
                    ));
                }
            }
        }
    }

    if lines.is_empty() {
        let empty_line = padded_line(empty_message(model.mode), list_width, padding);
        lines.push(RatatuiLine::from(Span::styled(
            empty_line,
            Style::default().fg(muted).bg(bg),
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

    let can_scroll_up = offset > 0;
    let can_scroll_down = offset + visible_height < total_lines;
    let footer_content = scroll_indicator_text(
        can_scroll_up,
        can_scroll_down,
        ScrollIndicatorStyle::Labeled,
    );

    if let Some(content) = footer_content {
        let footer = Paragraph::new(RatatuiLine::from(Span::styled(
            content,
            Style::default().fg(theme::secondary_text(surface)).bg(bg),
        )))
        .alignment(Alignment::Right);
        f.render_widget(footer, footer_area);
    }
}

pub fn render_date_picker(f: &mut Frame<'_>, area: Rect, model: DatePickerModel) {
    let popup_area = centered_rect_max(16, 3, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default().title(" Go to Date ").borders(Borders::ALL);

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let cursor_char = if model.cursor_pos < model.buffer.len() {
        model.buffer.chars().nth(model.cursor_pos).unwrap_or(' ')
    } else {
        ' '
    };
    let before_cursor: String = model.buffer.chars().take(model.cursor_pos).collect();
    let after_cursor: String = model.buffer.chars().skip(model.cursor_pos + 1).collect();

    let input_spans = vec![
        Span::raw(" "),
        Span::styled(&before_cursor, Style::default().fg(theme::CALENDAR_TEXT)),
        Span::styled(
            cursor_char.to_string(),
            Style::default()
                .fg(theme::TEXT_ON_ACCENT)
                .bg(theme::CALENDAR_TEXT),
        ),
        Span::styled(after_cursor, Style::default().fg(theme::CALENDAR_TEXT)),
    ];
    let input_line = Paragraph::new(RatatuiLine::from(input_spans));
    f.render_widget(input_line, inner);
}

pub fn render_overlays(f: &mut Frame<'_>, overlays: OverlayModel, layout: OverlayLayout<'_>) {
    if let Some(confirm) = overlays.confirm {
        render_confirm_modal(f, layout.screen_area, confirm);
    }
    if let Some(palette) = overlays.command_palette {
        render_command_palette(f, layout.screen_area, palette, layout.surface);
    }
    if let Some(date_picker) = overlays.date_picker {
        render_date_picker(f, layout.screen_area, date_picker);
    }
}
