use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line as RatatuiLine, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::super::layout::centered_rect_max;
use super::super::theme;

pub struct DatePickerModel {
    pub buffer: String,
    pub cursor_pos: usize,
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
