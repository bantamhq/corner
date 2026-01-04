use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::HintContext;

pub const HINT_OVERLAY_HEIGHT: u16 = 5;
const COLUMN_WIDTH: usize = 16;

pub fn render_hint_overlay(f: &mut Frame, hint_state: &HintContext, footer_area: Rect) -> bool {
    if matches!(hint_state, HintContext::Inactive) {
        return false;
    }

    let overlay_area = Rect {
        x: footer_area.x,
        y: footer_area.y.saturating_sub(HINT_OVERLAY_HEIGHT),
        width: footer_area.width,
        height: HINT_OVERLAY_HEIGHT,
    };

    if overlay_area.height == 0 || overlay_area.width < 20 {
        return false;
    }

    f.render_widget(Clear, overlay_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(overlay_area);
    f.render_widget(block, overlay_area);

    let lines = build_hint_lines(hint_state, inner.width as usize, inner.height as usize);
    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);

    true
}

fn build_hint_lines(hint_state: &HintContext, width: usize, max_rows: usize) -> Vec<Line<'static>> {
    let items: Vec<String> = match hint_state {
        HintContext::Inactive => return vec![],
        HintContext::Tags { matches, .. } => {
            matches.iter().map(|t| format!("#{t}")).collect()
        }
        HintContext::Commands { matches, .. } => {
            matches.iter().map(|cmd| format!(":{}", cmd.name)).collect()
        }
        HintContext::FilterTypes { matches, .. } => {
            matches.iter().map(|f| f.syntax.to_string()).collect()
        }
        HintContext::DateOps { matches, .. } => {
            matches.iter().map(|f| f.syntax.to_string()).collect()
        }
        HintContext::Negation { matches, .. } => {
            matches.iter().map(|f| f.syntax.to_string()).collect()
        }
    };

    let num_cols = width / COLUMN_WIDTH;
    if items.is_empty() || max_rows == 0 || num_cols == 0 {
        return vec![];
    }

    let mut row_spans: Vec<Vec<Span>> = vec![Vec::new(); max_rows];

    for (i, item) in items.iter().enumerate() {
        let col = i / max_rows;
        let row = i % max_rows;

        if col >= num_cols {
            break;
        }

        let display = format!("{:width$}", item, width = COLUMN_WIDTH);
        row_spans[row].push(Span::styled(display, Style::default().fg(Color::Cyan)));
    }

    row_spans
        .into_iter()
        .filter(|spans| !spans.is_empty())
        .map(Line::from)
        .collect()
}
