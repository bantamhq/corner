use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use chrono::{Days, NaiveDate};
use regex::Regex;

use super::entries::{CrossDayEntry, Entry, EntryType, Line, parse_lines};
use super::persistence::{load_journal, parse_day_header};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterType {
    Task,
    Note,
    Event,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub entry_types: Vec<FilterType>,
    pub completed: Option<bool>,
    pub tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub search_terms: Vec<String>,
    pub exclude_terms: Vec<String>,
    pub exclude_types: Vec<FilterType>,
    pub before_date: Option<NaiveDate>,
    pub after_date: Option<NaiveDate>,
    pub overdue: bool,
    pub invalid_tokens: Vec<String>,
}

pub static TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#([a-zA-Z][a-zA-Z0-9_-]*)").unwrap());

/// Matches @date patterns:
/// - @MM/DD (e.g., @1/9, @01/09)
/// - @MM/DD/YY (e.g., @1/9/26, @01/09/26)
/// - @MM/DD/YYYY (e.g., @1/9/2026, @01/09/2026)
/// - @YYYY/MM/DD (ISO format, e.g., @2026/1/9)
pub static LATER_DATE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@(\d{4}/\d{1,2}/\d{1,2}|\d{1,2}/\d{1,2}(?:/\d{2,4})?)").unwrap());

/// Matches natural date patterns for entries (future by default):
/// @today, @tomorrow, @yesterday, @d7 (7 days, max 999), @mon (next Monday)
pub static NATURAL_DATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)@(today|tomorrow|yesterday|d[1-9]\d{0,2}|mon|tue|wed|thu|fri|sat|sun)")
        .unwrap()
});

/// Matches favorite tag shortcuts: #1 through #9 and #0
pub static FAVORITE_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#([0-9])\b").unwrap());

/// Matches saved filter shortcuts: $name (alphanumeric + underscore)
pub static SAVED_FILTER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$(\w+)\b").unwrap());

#[must_use]
pub fn extract_tags(content: &str) -> Vec<String> {
    TAG_REGEX
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

/// Collects all unique tags from the current journal.
/// Returns tags sorted alphabetically, deduplicated (case-insensitive, first occurrence preserved).
pub fn collect_journal_tags(path: &Path) -> io::Result<Vec<String>> {
    let journal = load_journal(path)?;
    let mut seen_lower: HashSet<String> = HashSet::new();
    let mut tags: Vec<String> = Vec::new();

    for cap in TAG_REGEX.captures_iter(&journal) {
        let tag = cap[1].to_string();
        let lower = tag.to_lowercase();
        if seen_lower.insert(lower) {
            tags.push(tag);
        }
    }

    tags.sort_by_key(|a| a.to_lowercase());
    Ok(tags)
}

/// Parses a date string (without @) into a NaiveDate.
/// Tries ISO (YYYY/MM/DD), MM/DD/YYYY, MM/DD/YY, and MM/DD formats.
/// For MM/DD without year, uses "always future" logic: if date has passed
/// this year, assumes next year.
#[must_use]
pub fn parse_later_date(date_str: &str, today: NaiveDate) -> Option<NaiveDate> {
    use chrono::Datelike;

    // YYYY/MM/DD (only if first part is exactly 4 digits)
    if let Some(first_slash) = date_str.find('/')
        && first_slash == 4
        && date_str[..4].chars().all(|c| c.is_ascii_digit())
        && let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y/%m/%d")
    {
        return Some(date);
    }

    // MM/DD/YYYY or MM/DD/YY
    if date_str.matches('/').count() == 2 {
        let parts: Vec<&str> = date_str.split('/').collect();
        if parts.len() == 3
            && let (Ok(month), Ok(day), Ok(year)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<i32>(),
            )
        {
            // If year is 2 digits, assume 20xx
            let full_year = if year < 100 { 2000 + year } else { year };
            if let Some(date) = NaiveDate::from_ymd_opt(full_year, month, day) {
                return Some(date);
            }
        }
    }

    // MM/DD (no year) - use "always future" logic
    let parts: Vec<&str> = date_str.split('/').collect();
    if parts.len() == 2 {
        let month: u32 = parts[0].parse().ok()?;
        let day: u32 = parts[1].parse().ok()?;
        if let Some(date) = NaiveDate::from_ymd_opt(today.year(), month, day) {
            if date < today {
                return NaiveDate::from_ymd_opt(today.year() + 1, month, day);
            }
            return Some(date);
        }
    }

    None
}

/// Parses a weekday name (full or abbreviated) into chrono::Weekday.
fn parse_weekday(s: &str) -> Option<chrono::Weekday> {
    use chrono::Weekday;
    match s.to_lowercase().as_str() {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    }
}

/// Returns the next occurrence of a weekday after today (never returns today).
fn next_weekday_from(today: NaiveDate, target: chrono::Weekday) -> Option<NaiveDate> {
    use chrono::Datelike;
    let today_wd = today.weekday().num_days_from_monday();
    let target_wd = target.num_days_from_monday();

    let days_ahead = if target_wd > today_wd {
        target_wd - today_wd
    } else {
        7 - today_wd + target_wd
    };
    let days_ahead = if days_ahead == 0 { 7 } else { days_ahead };

    today.checked_add_days(Days::new(u64::from(days_ahead)))
}

/// Returns the most recent occurrence of a weekday before today (never returns today).
fn prev_weekday_from(today: NaiveDate, target: chrono::Weekday) -> Option<NaiveDate> {
    use chrono::Datelike;
    let today_wd = today.weekday().num_days_from_monday();
    let target_wd = target.num_days_from_monday();

    let days_back = if target_wd < today_wd {
        today_wd - target_wd
    } else {
        7 - target_wd + today_wd
    };
    let days_back = if days_back == 0 { 7 } else { days_back };

    today.checked_sub_days(Days::new(u64::from(days_back)))
}

fn parse_relative_date(input: &str, today: NaiveDate, default_future: bool) -> Option<NaiveDate> {
    let input_lower = input.to_lowercase();

    if input_lower == "today" {
        return Some(today);
    }
    if input_lower == "tomorrow" {
        return today.checked_add_days(Days::new(1));
    }
    if input_lower == "yesterday" {
        return today.checked_sub_days(Days::new(1));
    }

    let (base, is_future) = if let Some(b) = input_lower.strip_suffix('+') {
        (b, true)
    } else {
        (input_lower.as_str(), default_future)
    };

    if let Some(days_str) = base.strip_prefix('d')
        && days_str.len() <= 3
        && let Ok(days) = days_str.parse::<u64>()
        && days > 0
    {
        return if is_future {
            today.checked_add_days(Days::new(days))
        } else {
            today.checked_sub_days(Days::new(days))
        };
    }

    if let Some(target_weekday) = parse_weekday(base) {
        return if is_future {
            next_weekday_from(today, target_weekday)
        } else {
            prev_weekday_from(today, target_weekday)
        };
    }

    None
}

/// Parses natural language date expressions for ENTRIES (future by default):
/// today, tomorrow, yesterday, d7 (7 days from now), mon (next Monday).
/// Falls back to parse_later_date for standard formats.
#[must_use]
pub fn parse_natural_date(input: &str, today: NaiveDate) -> Option<NaiveDate> {
    parse_relative_date(input, today, true).or_else(|| parse_later_date(input, today))
}

/// Parses date expressions for FILTERS (past by default):
/// today, tomorrow, yesterday, d7 (7 days ago), mon (last Monday).
/// Append + for explicit future: d7+ (7 days from now), mon+ (next Monday).
#[must_use]
pub fn parse_filter_date(input: &str, today: NaiveDate) -> Option<NaiveDate> {
    parse_relative_date(input, today, false).or_else(|| parse_later_date(input, today))
}

/// Replaces natural date patterns (@today, @tomorrow, @yesterday, @d7, @mon) with @MM/DD format.
#[must_use]
pub fn normalize_natural_dates(content: &str, today: NaiveDate) -> String {
    let mut result = content.to_string();

    for cap in NATURAL_DATE_REGEX.captures_iter(content) {
        if let Some(m) = cap.get(0) {
            let natural_str = &cap[1];
            if let Some(date) = parse_natural_date(natural_str, today) {
                // Only @today needs the year to avoid "always future" misinterpretation
                let normalized = if natural_str.eq_ignore_ascii_case("today") {
                    format!(
                        "@{}/{}/{}",
                        date.format("%m"),
                        date.format("%d"),
                        date.format("%y")
                    )
                } else {
                    format!("@{}/{}", date.format("%m"), date.format("%d"))
                };
                result = result.replacen(m.as_str(), &normalized, 1);
            }
        }
    }

    result
}

/// Replaces favorite tag shortcuts (#0 through #9) with actual tags from config.
/// Tags that don't exist in config are left unchanged.
#[must_use]
pub fn expand_favorite_tags(content: &str, favorite_tags: &HashMap<String, String>) -> String {
    let mut result = content.to_string();

    for cap in FAVORITE_TAG_REGEX.captures_iter(content) {
        if let Some(m) = cap.get(0) {
            let digit = &cap[1];
            if let Some(tag) = favorite_tags.get(digit).filter(|s| !s.is_empty()) {
                result = result.replacen(m.as_str(), &format!("#{tag}"), 1);
            }
        }
    }

    result
}

/// Expands saved filter shortcuts ($name) with their definitions from config.
/// Returns the expanded query and a list of unknown filter names.
#[must_use]
pub fn expand_saved_filters(
    query: &str,
    filters: &HashMap<String, String>,
) -> (String, Vec<String>) {
    let mut result = query.to_string();
    let mut unknown = Vec::new();

    for cap in SAVED_FILTER_REGEX.captures_iter(query) {
        if let Some(m) = cap.get(0) {
            let name = &cap[1];
            if let Some(expansion) = filters.get(name) {
                result = result.replacen(m.as_str(), expansion, 1);
            } else {
                unknown.push(m.as_str().to_string());
            }
        }
    }

    (result, unknown)
}

/// Extracts the target date from entry content if it contains an @date pattern.
#[must_use]
pub fn extract_target_date(content: &str, today: NaiveDate) -> Option<NaiveDate> {
    LATER_DATE_REGEX
        .captures(content)
        .and_then(|cap| cap.get(1))
        .and_then(|m| parse_later_date(m.as_str(), today))
}

/// Like parse_later_date but for MM/DD format prefers the most recent past occurrence.
/// Used for overdue checking where we want to interpret @12/30 on 1/1 as last year.
#[must_use]
fn parse_date_prefer_past(date_str: &str, today: NaiveDate) -> Option<NaiveDate> {
    use chrono::Datelike;

    // For formats with explicit year (MM/DD/YY, MM/DD/YYYY, or YYYY/MM/DD), parse normally
    if date_str.matches('/').count() == 2 {
        return parse_later_date(date_str, today);
    }

    // MM/DD - prefer past (most recent occurrence)
    let parts: Vec<&str> = date_str.split('/').collect();
    if parts.len() == 2 {
        let month: u32 = parts[0].parse().ok()?;
        let day: u32 = parts[1].parse().ok()?;

        // Try current year first
        if let Some(date) = NaiveDate::from_ymd_opt(today.year(), month, day) {
            if date <= today {
                return Some(date);
            }
            // Date is in future this year, so use last year
            return NaiveDate::from_ymd_opt(today.year() - 1, month, day);
        }
    }

    None
}

/// Extracts target date preferring past interpretation (for overdue checking).
#[must_use]
fn extract_target_date_prefer_past(content: &str, today: NaiveDate) -> Option<NaiveDate> {
    LATER_DATE_REGEX
        .captures(content)
        .and_then(|cap| cap.get(1))
        .and_then(|m| parse_date_prefer_past(m.as_str(), today))
}

/// Collects all entries with @date matching the target date.
/// Entries from the target date itself are excluded (they're regular entries).
pub fn collect_later_entries_for_date(
    target_date: NaiveDate,
    path: &Path,
) -> io::Result<Vec<CrossDayEntry>> {
    let journal = load_journal(path)?;
    let mut entries = Vec::new();
    let mut current_date: Option<NaiveDate> = None;
    let mut line_index_in_day: usize = 0;

    for line in journal.lines() {
        if let Some(date) = parse_day_header(line) {
            current_date = Some(date);
            line_index_in_day = 0;
            continue;
        }

        if let Some(source_date) = current_date {
            // Skip entries from the target day itself (they're regular entries)
            if source_date == target_date {
                line_index_in_day += 1;
                continue;
            }

            let parsed = parse_lines(line);
            if let Some(Line::Entry(entry)) = parsed.first()
                && let Some(entry_target) = extract_target_date(&entry.content, target_date)
                && entry_target == target_date
            {
                let completed = matches!(entry.entry_type, EntryType::Task { completed: true });
                entries.push(CrossDayEntry {
                    source_date,
                    line_index: line_index_in_day,
                    content: entry.content.clone(),
                    entry_type: entry.entry_type.clone(),
                    completed,
                });
            }
            line_index_in_day += 1;
        }
    }

    // Sort by source date (chronologically - older first)
    entries.sort_by_key(|entry| entry.source_date);
    Ok(entries)
}

fn parse_type_keyword(s: &str) -> Option<FilterType> {
    match s {
        "tasks" | "task" | "t" => Some(FilterType::Task),
        "notes" | "note" | "n" => Some(FilterType::Note),
        "events" | "event" | "e" => Some(FilterType::Event),
        _ => None,
    }
}

#[must_use]
pub fn parse_filter_query(query: &str) -> Filter {
    let mut filter = Filter::default();
    let today = chrono::Local::now().date_naive();

    for token in query.split_whitespace() {
        // Date filters: @before:DATE, @after:DATE, @overdue
        // Dates default to past (d7 = 7 days ago, mon = last Monday)
        // Append + for explicit future (d7+ = 7 days from now, mon+ = next Monday)
        if let Some(date_str) = token.strip_prefix("@before:") {
            if filter.before_date.is_some() {
                filter
                    .invalid_tokens
                    .push("Multiple @before dates".to_string());
            } else if let Some(date) = parse_filter_date(date_str, today) {
                filter.before_date = Some(date);
            } else {
                filter.invalid_tokens.push(token.to_string());
            }
            continue;
        }
        if let Some(date_str) = token.strip_prefix("@after:") {
            if filter.after_date.is_some() {
                filter
                    .invalid_tokens
                    .push("Multiple @after dates".to_string());
            } else if let Some(date) = parse_filter_date(date_str, today) {
                filter.after_date = Some(date);
            } else {
                filter.invalid_tokens.push(token.to_string());
            }
            continue;
        }
        if token == "@overdue" {
            filter.overdue = true;
            continue;
        }
        // Any other @command is invalid
        if token.starts_with('@') && token.contains(':') {
            filter.invalid_tokens.push(token.to_string());
            continue;
        }

        if let Some(negated) = token.strip_prefix("not:") {
            if let Some(tag) = negated.strip_prefix('#') {
                filter.exclude_tags.push(tag.to_string());
            } else if let Some(type_str) = negated.strip_prefix('!') {
                if let Some(filter_type) = parse_type_keyword(type_str) {
                    filter.exclude_types.push(filter_type);
                } else {
                    filter.invalid_tokens.push(token.to_string());
                }
            } else if !negated.is_empty() {
                filter.exclude_terms.push(negated.to_string());
            }
        } else if let Some(type_str) = token.strip_prefix('!') {
            let base_type = if let Some(idx) = type_str.find('/') {
                &type_str[..idx]
            } else {
                type_str
            };

            let (new_type, completed_override) = match base_type {
                "tasks" | "task" | "t" => (Some(FilterType::Task), Some(false)),
                "completed" | "c" => (Some(FilterType::Task), Some(true)),
                "notes" | "note" | "n" => (Some(FilterType::Note), None),
                "events" | "event" | "e" => (Some(FilterType::Event), None),
                _ => (None, None),
            };

            if let Some(new_type) = new_type {
                if !filter.entry_types.contains(&new_type) {
                    filter.entry_types.push(new_type);
                }
                // Handle completed filter: if conflicting values, show all (None)
                if let Some(new_completed) = completed_override {
                    filter.completed = match filter.completed {
                        None => Some(new_completed),
                        Some(existing) if existing != new_completed => None,
                        other => other,
                    };
                }
            } else {
                filter.invalid_tokens.push(token.to_string());
            }
        } else if let Some(tag) = token.strip_prefix('#') {
            filter.tags.push(tag.to_string());
        } else if !token.is_empty() {
            filter.search_terms.push(token.to_string());
        }
    }

    filter
}

pub fn collect_filtered_entries(filter: &Filter, path: &Path) -> io::Result<Vec<CrossDayEntry>> {
    if !filter.invalid_tokens.is_empty() {
        return Ok(Vec::new());
    }

    let journal = load_journal(path)?;
    let mut entries = Vec::new();
    let mut current_date: Option<NaiveDate> = None;
    let mut line_index_in_day: usize = 0;
    let today = chrono::Local::now().date_naive();

    for line in journal.lines() {
        if let Some(date) = parse_day_header(line) {
            current_date = Some(date);
            line_index_in_day = 0;
            continue;
        }

        if let Some(source_date) = current_date {
            // Date filters on day header
            if let Some(before) = filter.before_date
                && source_date > before
            {
                line_index_in_day += 1;
                continue;
            }
            if let Some(after) = filter.after_date
                && source_date < after
            {
                line_index_in_day += 1;
                continue;
            }

            let parsed = parse_lines(line);
            if let Some(Line::Entry(entry)) = parsed.first() {
                // Overdue filter: entry must have @date targeting before today
                if filter.overdue {
                    let target = extract_target_date_prefer_past(&entry.content, today);
                    if target.is_none() || target.unwrap() >= today {
                        line_index_in_day += 1;
                        continue;
                    }
                }

                if entry_matches_filter(entry, filter) {
                    let completed = matches!(entry.entry_type, EntryType::Task { completed: true });
                    entries.push(CrossDayEntry {
                        source_date,
                        content: entry.content.clone(),
                        line_index: line_index_in_day,
                        entry_type: entry.entry_type.clone(),
                        completed,
                    });
                }
            }
            line_index_in_day += 1;
        }
    }

    entries.sort_by_key(|entry| entry.source_date);
    Ok(entries)
}

fn entry_type_to_filter_type(entry_type: &EntryType) -> FilterType {
    match entry_type {
        EntryType::Task { .. } => FilterType::Task,
        EntryType::Note => FilterType::Note,
        EntryType::Event => FilterType::Event,
    }
}

fn entry_matches_filter(entry: &Entry, filter: &Filter) -> bool {
    let entry_filter_type = entry_type_to_filter_type(&entry.entry_type);

    if !filter.entry_types.is_empty() && !filter.entry_types.contains(&entry_filter_type) {
        return false;
    }

    for excluded_type in &filter.exclude_types {
        if &entry_filter_type == excluded_type {
            return false;
        }
    }

    if let Some(want_completed) = filter.completed
        && let EntryType::Task { completed } = entry.entry_type
        && completed != want_completed
    {
        return false;
    }

    let entry_tags = extract_tags(&entry.content);

    for required_tag in &filter.tags {
        if !entry_tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case(required_tag))
        {
            return false;
        }
    }

    for excluded_tag in &filter.exclude_tags {
        if entry_tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case(excluded_tag))
        {
            return false;
        }
    }

    let content_lower = entry.content.to_lowercase();

    for term in &filter.search_terms {
        if !content_lower.contains(&term.to_lowercase()) {
            return false;
        }
    }

    for term in &filter.exclude_terms {
        if content_lower.contains(&term.to_lowercase()) {
            return false;
        }
    }

    true
}
