use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line as RatatuiLine, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::ConfirmContext;

use super::super::layout::centered_rect;
use super::super::theme;

pub struct ConfirmModel {
    pub context: ConfirmContext,
}

impl ConfirmModel {
    #[must_use]
    pub fn new(context: ConfirmContext) -> Self {
        Self { context }
    }
}

pub fn render_confirm_modal(f: &mut Frame<'_>, area: Rect, model: ConfirmModel) {
    let (title, messages): (&str, [String; 2]) = match &model.context {
        ConfirmContext::CreateProjectJournal => (
            theme::TITLE_CREATE_PROJECT,
            [
                theme::MSG_NO_PROJECT_JOURNAL.to_string(),
                theme::MSG_CREATE_PROJECT_JOURNAL.to_string(),
            ],
        ),
        ConfirmContext::DeleteTag(tag) => (
            theme::TITLE_DELETE_TAG,
            [
                format!("Delete all occurrences of #{tag}?"),
                theme::LABEL_CANNOT_UNDO.to_string(),
            ],
        ),
        ConfirmContext::DeleteTagFromCompleted(tag) => (
            theme::TITLE_REMOVE_FROM_COMPLETED,
            [
                format!("Remove #{tag} from completed tasks?"),
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

    let lines = vec![
        RatatuiLine::raw(""),
        RatatuiLine::raw(messages[0].clone()),
        RatatuiLine::raw(messages[1].clone()),
        RatatuiLine::raw(""),
        RatatuiLine::from(vec![
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
        ]),
    ];
    let content = ratatui::text::Text::from(lines);
    let paragraph = Paragraph::new(content).alignment(Alignment::Center);
    f.render_widget(paragraph, inner_area);
}
