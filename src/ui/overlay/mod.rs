use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::{Line as RatatuiLine, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{
    ConfirmContext, DateInterfaceState, HintContext, ProjectInterfaceState, TagInterfaceState,
};
use crate::dispatch::Keymap;
use crate::registry::{KeyActionId, KeyContext};
use crate::storage::JournalSlot;

use super::date_interface::render_date_interface;
use super::footer::{FooterModel, centered_rect};
use super::help::{get_help_total_lines, render_help_content};
use super::hints;
use super::project_interface::render_project_interface;
use super::scroll_indicator::{ScrollIndicatorStyle, scroll_indicator_text};
use super::shared::format_key_for_display;
use super::tag_interface::render_tag_interface;
use super::theme;

pub struct HintModel<'a> {
    pub hint_state: &'a HintContext,
}

impl<'a> HintModel<'a> {
    #[must_use]
    pub fn new(hint_state: &'a HintContext) -> Self {
        Self { hint_state }
    }
}

pub struct StatusModel<'a> {
    pub message: Option<&'a str>,
}

impl<'a> StatusModel<'a> {
    #[must_use]
    pub fn new(message: Option<&'a str>) -> Self {
        Self { message }
    }
}

pub struct JournalIndicatorModel {
    pub slot: JournalSlot,
    pub current_project_id: Option<String>,
}

impl JournalIndicatorModel {
    #[must_use]
    pub fn new(slot: JournalSlot, current_project_id: Option<String>) -> Self {
        Self {
            slot,
            current_project_id,
        }
    }
}

pub struct HelpModel<'a> {
    pub keymap: &'a Keymap,
    pub scroll: usize,
    pub visible_height: usize,
}

impl<'a> HelpModel<'a> {
    #[must_use]
    pub fn new(keymap: &'a Keymap, scroll: usize, visible_height: usize) -> Self {
        Self {
            keymap,
            scroll,
            visible_height,
        }
    }
}

pub struct ConfirmModel<'a> {
    pub context: &'a ConfirmContext,
}

impl<'a> ConfirmModel<'a> {
    #[must_use]
    pub fn new(context: &'a ConfirmContext) -> Self {
        Self { context }
    }
}

pub enum InterfaceModel<'a> {
    Date(&'a DateInterfaceState),
    Project {
        state: &'a ProjectInterfaceState,
        current_project_id: Option<String>,
    },
    Tag(&'a TagInterfaceState),
}

impl<'a> InterfaceModel<'a> {
    #[must_use]
    pub fn date(state: &'a DateInterfaceState) -> Self {
        Self::Date(state)
    }

    #[must_use]
    pub fn project(state: &'a ProjectInterfaceState, current_project_id: Option<String>) -> Self {
        Self::Project {
            state,
            current_project_id,
        }
    }

    #[must_use]
    pub fn tag(state: &'a TagInterfaceState) -> Self {
        Self::Tag(state)
    }
}

pub fn render_footer_bar(f: &mut Frame<'_>, model: FooterModel<'_>, area: Rect) {
    let footer = Paragraph::new(super::footer::render_footer(model));
    f.render_widget(footer, area);
}

pub fn render_hint_overlay(f: &mut Frame<'_>, model: HintModel<'_>, footer_area: Rect) -> bool {
    hints::render_hint_overlay(f, model.hint_state, footer_area)
}

pub fn render_status_banner(f: &mut Frame<'_>, model: StatusModel<'_>, content_area: Rect) {
    if let Some(msg) = model.message {
        let msg_width = msg.len() as u16 + 2;
        let status_area = Rect {
            x: content_area.x,
            y: content_area.y + content_area.height.saturating_sub(1),
            width: msg_width.min(content_area.width),
            height: 1,
        };
        let status = Paragraph::new(Span::styled(
            format!(" {msg} "),
            Style::default().fg(theme::STATUS_FG).bg(theme::STATUS_BG),
        ));
        f.render_widget(status, status_area);
    }
}

pub fn render_journal_indicator(
    f: &mut Frame<'_>,
    model: JournalIndicatorModel,
    footer_area: Rect,
) {
    let (indicator, indicator_color) = match model.slot {
        JournalSlot::Hub => ("[HUB]".to_string(), theme::JOURNAL_HUB),
        JournalSlot::Project => {
            let id = model
                .current_project_id
                .as_ref()
                .map(|id| format!("[{}]", id.to_uppercase()))
                .unwrap_or_else(|| "[PROJECT]".to_string());
            (id, theme::JOURNAL_PROJECT)
        }
    };
    let indicator_width = indicator.len() as u16;
    let indicator_area = Rect {
        x: footer_area.x + footer_area.width.saturating_sub(indicator_width),
        y: footer_area.y,
        width: indicator_width,
        height: 1,
    };
    let indicator_widget = Paragraph::new(Span::styled(
        indicator,
        Style::default().fg(indicator_color),
    ));
    f.render_widget(indicator_widget, indicator_area);
}

pub fn render_help_modal(f: &mut Frame<'_>, model: HelpModel<'_>, popup_area: Rect) {
    f.render_widget(Clear, popup_area);

    let total = get_help_total_lines(model.keymap);
    let max_scroll = total.saturating_sub(model.visible_height);
    let can_scroll_up = model.scroll > 0;
    let can_scroll_down = model.scroll < max_scroll;

    let arrows =
        scroll_indicator_text(can_scroll_up, can_scroll_down, ScrollIndicatorStyle::Arrows)
            .unwrap_or("");

    let help_block = Block::default()
        .title(" Keybindings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::HELP_BORDER));

    let inner_area = help_block.inner(popup_area);
    f.render_widget(help_block, popup_area);

    let help_content = render_help_content(model.keymap, model.scroll, model.visible_height);
    let help_paragraph = Paragraph::new(help_content);
    f.render_widget(help_paragraph, inner_area);

    let footer_area = Rect {
        x: inner_area.x,
        y: inner_area.y + inner_area.height.saturating_sub(1),
        width: inner_area.width,
        height: 1,
    };

    let close_keys = model
        .keymap
        .keys_for_action_ordered(KeyContext::Help, KeyActionId::ToggleHelp);
    let close_key = close_keys
        .first()
        .map(|k| format_key_for_display(k))
        .unwrap_or_else(|| "?".to_string());

    let footer_line = if arrows.is_empty() {
        RatatuiLine::from(vec![
            Span::styled(close_key.clone(), Style::default().fg(theme::HELP_FOOTER)),
            Span::styled(" close ", Style::default().dim()),
        ])
    } else {
        RatatuiLine::from(vec![
            Span::styled(arrows, Style::default().fg(theme::HELP_FOOTER)),
            Span::styled(" scroll  ", Style::default().dim()),
            Span::styled(close_key, Style::default().fg(theme::HELP_FOOTER)),
            Span::styled(" close ", Style::default().dim()),
        ])
    };
    let footer = Paragraph::new(footer_line).alignment(Alignment::Right);
    f.render_widget(footer, footer_area);
}

pub fn render_confirm_modal(f: &mut Frame<'_>, model: ConfirmModel<'_>, area: Rect) {
    let (title, messages): (&str, Vec<String>) = match model.context {
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

pub fn render_interface_modal(f: &mut Frame<'_>, model: InterfaceModel<'_>, area: Rect) {
    match model {
        InterfaceModel::Date(state) => render_date_interface(f, state, area),
        InterfaceModel::Project {
            state,
            current_project_id,
        } => render_project_interface(f, state, area, current_project_id.as_deref()),
        InterfaceModel::Tag(state) => render_tag_interface(f, state, area),
    }
}
