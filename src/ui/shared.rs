use std::sync::LazyLock;

use chrono::NaiveDate;
use ratatui::{
    style::{Color, Style},
    text::Span,
};
use regex::Regex;
use unicode_width::UnicodeWidthStr;

use crate::storage::{EntryType, LATER_DATE_REGEX, RECURRING_REGEX, RELATIVE_DATE_REGEX, TAG_REGEX};

#[must_use]
pub fn entry_style(entry_type: &EntryType) -> Style {
    match entry_type {
        EntryType::Task { completed: true } => {
            Style::default().add_modifier(ratatui::style::Modifier::DIM)
        }
        EntryType::Event => Style::default().add_modifier(ratatui::style::Modifier::ITALIC),
        _ => Style::default(),
    }
}

#[must_use]
pub fn format_date_suffix(date: NaiveDate) -> (String, usize) {
    let suffix = format!(" ({})", date.format("%m/%d"));
    let width = suffix.width();
    (suffix, width)
}

/// Style for date suffixes - always dimmed relative to entry content
#[must_use]
pub fn date_suffix_style(base: Style) -> Style {
    base.add_modifier(ratatui::style::Modifier::DIM)
}

/// Matches one or more trailing tags at end of line
static TRAILING_TAGS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\s+#[a-zA-Z][a-zA-Z0-9_-]*)+\s*$").unwrap());

/// Matches the last trailing tag (space + tag at end)
static LAST_TRAILING_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+#[a-zA-Z][a-zA-Z0-9_-]*\s*$").unwrap());

/// Remove the last trailing tag, returns None if no trailing tags or entry is only tags
#[must_use]
pub fn remove_last_trailing_tag(text: &str) -> Option<String> {
    LAST_TRAILING_TAG_REGEX.find(text).and_then(|m| {
        let before = &text[..m.start()];
        if before.chars().any(|c| !c.is_whitespace()) {
            Some(before.to_string())
        } else {
            None
        }
    })
}

/// Remove all trailing tags, returns None if no trailing tags or entry is only tags
#[must_use]
pub fn remove_all_trailing_tags(text: &str) -> Option<String> {
    TRAILING_TAGS_REGEX.find(text).and_then(|m| {
        let before = &text[..m.start()];
        if before.chars().any(|c| !c.is_whitespace()) {
            Some(before.to_string())
        } else {
            None
        }
    })
}

pub fn style_content(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut last_end = 0;

    let tag_color = Color::Yellow;
    let date_color = Color::Red;

    let mut matches: Vec<(usize, usize, Color)> = Vec::new();

    for cap in TAG_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), tag_color));
        }
    }

    for cap in LATER_DATE_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), date_color));
        }
    }

    for cap in RELATIVE_DATE_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), date_color));
        }
    }

    for cap in RECURRING_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(0) {
            matches.push((m.start(), m.end(), date_color));
        }
    }

    matches.sort_by_key(|(start, _, _)| *start);

    for (start, end, color) in matches {
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        // Apply base style modifiers (like DIM) to colored spans
        spans.push(Span::styled(
            text[start..end].to_string(),
            base_style.fg(color),
        ));
        last_end = end;
    }

    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }

    spans
}

pub fn truncate_text(text: &str, max_width: usize) -> String {
    if text.width() <= max_width {
        return text.to_string();
    }

    let ellipsis = "…";
    let target_width = max_width.saturating_sub(1); // Room for ellipsis

    let mut result = String::new();
    let mut current_width = 0;

    for ch in text.chars() {
        let ch_width = ch.to_string().width();
        if current_width + ch_width > target_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }

    format!("{result}{ellipsis}")
}

/// Split text into (content, trailing_tags) if tags exist at end
#[must_use]
pub fn split_trailing_tags(text: &str) -> (&str, Option<&str>) {
    if let Some(m) = TRAILING_TAGS_REGEX.find(text) {
        (&text[..m.start()], Some(m.as_str().trim()))
    } else {
        (text, None)
    }
}

/// Truncate text while preserving trailing tags when possible
#[must_use]
pub fn truncate_with_tags(text: &str, max_width: usize) -> String {
    let (content, tags) = split_trailing_tags(text);

    let Some(tags) = tags else {
        return truncate_text(text, max_width);
    };

    let tag_width = tags.width() + 1; // +1 for space before tags
    if tag_width >= max_width {
        return truncate_text(text, max_width);
    }

    let content_width = max_width - tag_width;
    if content.width() <= content_width {
        text.to_string()
    } else {
        format!("{} {}", truncate_text(content, content_width), tags)
    }
}

/// Format a key spec for user-facing display
#[must_use]
pub fn format_key_for_display(key: &str) -> String {
    match key {
        "down" => "↓".to_string(),
        "up" => "↑".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "ret" => "Enter".to_string(),
        "esc" => "Esc".to_string(),
        "tab" => "Tab".to_string(),
        "backtab" => "Shift+Tab".to_string(),
        "backspace" => "Bksp".to_string(),
        " " => "Space".to_string(),
        _ if key.starts_with("S-") => format!("Shift+{}", &key[2..]),
        _ => key.to_string(),
    }
}

pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_inclusive(' ') {
        let word_width = word.width();

        if current_width + word_width <= max_width {
            current_line.push_str(word);
            current_width += word_width;
        } else if current_line.is_empty() {
            // Word is longer than max_width, must break it by character
            for ch in word.chars() {
                let ch_width = ch.to_string().width();
                if current_width + ch_width > max_width && !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::new();
                    current_width = 0;
                }
                current_line.push(ch);
                current_width += ch_width;
            }
        } else {
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() || lines.is_empty() {
        lines.push(current_line);
    }

    lines
}
