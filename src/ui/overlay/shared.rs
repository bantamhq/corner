use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
};

use super::super::theme;

#[must_use]
pub fn title_case(input: &str) -> String {
    input
        .split(['_', '-'])
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| {
            let mut chars = chunk.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn padded_line(text: &str, width: usize, padding: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let available = width.saturating_sub(padding.saturating_mul(2));
    let trimmed: String = text.chars().take(available).collect();
    let text_len = trimmed.chars().count();
    if width <= padding * 2 {
        return trimmed.chars().take(width).collect();
    }
    format!(
        "{pad}{trimmed}{fill}{pad}",
        pad = " ".repeat(padding),
        fill = " ".repeat(available.saturating_sub(text_len))
    )
}

#[must_use]
pub fn padded_area(area: Rect, padding: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(padding),
        y: area.y,
        width: area.width.saturating_sub(padding.saturating_mul(2)),
        height: area.height,
    }
}

#[must_use]
pub fn item_styles(
    is_selected: bool,
    is_available: bool,
    bg: ratatui::style::Color,
    muted: ratatui::style::Color,
) -> (Style, Style) {
    let dim = if is_available {
        Modifier::empty()
    } else {
        Modifier::DIM
    };

    if is_selected {
        let base = Style::default().bg(bg).add_modifier(Modifier::REVERSED | dim);
        (base.add_modifier(Modifier::BOLD), base)
    } else {
        (
            Style::default()
                .fg(theme::CALENDAR_TEXT)
                .bg(bg)
                .add_modifier(Modifier::BOLD | dim),
            Style::default().fg(muted).bg(bg).add_modifier(dim),
        )
    }
}
