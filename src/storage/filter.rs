use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use chrono::NaiveDate;
use regex::Regex;

use super::date_parsing::{ParseContext, parse_date, parse_weekday};
use super::entries::{Entry, EntryType, Line, RawEntry, RecurringPattern, SourceType, parse_lines};
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
    pub later: bool,
    pub recurring: bool,
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

/// Matches relative date patterns for entries (future by default):
/// @today, @tomorrow, @yesterday, @d7 (7 days, max 999), @mon (next Monday)
pub static RELATIVE_DATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)@(today|tomorrow|yesterday|d[1-9]\d{0,2}|mon|tue|wed|thu|fri|sat|sun)")
        .unwrap()
});

/// Matches favorite tag shortcuts: #1 through #9 and #0
pub static FAVORITE_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#([0-9])\b").unwrap());

/// Matches saved filter shortcuts: $name (alphanumeric + underscore)
pub static SAVED_FILTER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$(\w+)\b").unwrap());

/// Matches @every-* patterns for recurring entries:
/// @every-day, @every-weekday, @every-mon..sun (or full names), @every-1..31
pub static RECURRING_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)@every-(day|weekday|monday|tuesday|wednesday|thursday|friday|saturday|sunday|mon|tue|wed|thu|fri|sat|sun|[1-9]|[12]\d|3[01])(?:\s|$)")
        .unwrap()
});

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

/// Parses natural language date expressions for ENTRIES (future by default):
/// today, tomorrow, yesterday, d7 (7 days from now), mon (next Monday).
/// Delegates to the consolidated parse_date API with Entry context.
#[must_use]
pub fn parse_natural_date(input: &str, today: NaiveDate) -> Option<NaiveDate> {
    parse_date(input, ParseContext::Entry, today)
}

/// Parses date expressions for FILTERS (past by default):
/// today, tomorrow, yesterday, d7 (7 days ago), mon (last Monday).
/// Append + for explicit future: d7+ (7 days from now), mon+ (next Monday).
/// Delegates to the consolidated parse_date API with Filter context.
#[must_use]
pub fn parse_filter_date(input: &str, today: NaiveDate) -> Option<NaiveDate> {
    parse_date(input, ParseContext::Filter, today)
}

/// Replaces relative date patterns (@today, @tomorrow, @yesterday, @d7, @mon) with @MM/DD format.
#[must_use]
pub fn normalize_relative_dates(content: &str, today: NaiveDate) -> String {
    RELATIVE_DATE_REGEX
        .replace_all(content, |caps: &regex::Captures| {
            let natural_str = &caps[1];
            parse_natural_date(natural_str, today).map_or_else(
                || caps[0].to_string(),
                |date| {
                    if natural_str.eq_ignore_ascii_case("today") {
                        format!(
                            "@{}/{}/{}",
                            date.format("%m"),
                            date.format("%d"),
                            date.format("%y")
                        )
                    } else {
                        format!("@{}/{}", date.format("%m"), date.format("%d"))
                    }
                },
            )
        })
        .into_owned()
}

/// Replaces favorite tag shortcuts (#0 through #9) with actual tags from config.
/// Tags that don't exist in config are left unchanged.
#[must_use]
pub fn expand_favorite_tags(content: &str, favorite_tags: &HashMap<String, String>) -> String {
    FAVORITE_TAG_REGEX
        .replace_all(content, |caps: &regex::Captures| {
            let digit = &caps[1];
            favorite_tags
                .get(digit)
                .filter(|s| !s.is_empty())
                .map_or_else(|| caps[0].to_string(), |tag| format!("#{tag}"))
        })
        .into_owned()
}

/// Expands saved filter shortcuts ($name) with their definitions from config.
/// Returns the expanded query and a list of unknown filter names.
#[must_use]
pub fn expand_saved_filters(
    query: &str,
    filters: &HashMap<String, String>,
) -> (String, Vec<String>) {
    use std::cell::RefCell;

    let unknown = RefCell::new(Vec::new());
    let result = SAVED_FILTER_REGEX
        .replace_all(query, |caps: &regex::Captures| {
            let name = &caps[1];
            filters.get(name).map_or_else(
                || {
                    unknown.borrow_mut().push(caps[0].to_string());
                    caps[0].to_string()
                },
                |expansion| expansion.clone(),
            )
        })
        .into_owned();

    (result, unknown.into_inner())
}

/// Parses an @every-* pattern string (without the @every- prefix) into a RecurringPattern.
/// Reuses `parse_weekday()` for weekday names to avoid duplication.
#[must_use]
pub fn parse_recurring_pattern(pattern_str: &str) -> Option<RecurringPattern> {
    let lower = pattern_str.to_lowercase();
    match lower.as_str() {
        "day" => Some(RecurringPattern::Daily),
        "weekday" => Some(RecurringPattern::Weekday),
        _ => {
            // Try parsing as weekday (reuse existing parse_weekday)
            if let Some(weekday) = parse_weekday(&lower) {
                return Some(RecurringPattern::Weekly(weekday));
            }
            // Then try as day of month (1-31)
            lower
                .parse::<u8>()
                .ok()
                .filter(|&d| (1..=31).contains(&d))
                .map(RecurringPattern::Monthly)
        }
    }
}

/// Strips @every-* tags from content (e.g., for matching done-today entries).
#[must_use]
pub fn strip_recurring_tags(content: &str) -> String {
    RECURRING_REGEX.replace_all(content, "").trim().to_string()
}

/// Extracts the recurring pattern from entry content if it contains an @every-* pattern.
#[must_use]
pub fn extract_recurring_pattern(content: &str) -> Option<RecurringPattern> {
    RECURRING_REGEX
        .captures(content)
        .and_then(|cap| cap.get(1))
        .and_then(|m| parse_recurring_pattern(m.as_str()))
}

/// Extracts the target date from entry content if it contains an @date pattern.
/// Uses Entry context (future bias) since entry @dates are forward-looking.
#[must_use]
pub fn extract_target_date(content: &str, today: NaiveDate) -> Option<NaiveDate> {
    LATER_DATE_REGEX
        .captures(content)
        .and_then(|cap| cap.get(1))
        .and_then(|m| parse_date(m.as_str(), ParseContext::Entry, today))
}

/// Extracts target date preferring past interpretation (for overdue checking).
/// Uses Filter context (past bias) to interpret @12/30 on 1/1 as last year.
#[must_use]
fn extract_target_date_prefer_past(content: &str, today: NaiveDate) -> Option<NaiveDate> {
    LATER_DATE_REGEX
        .captures(content)
        .and_then(|cap| cap.get(1))
        .and_then(|m| parse_date(m.as_str(), ParseContext::Filter, today))
}

/// Collects all projected entries (both @later and @recurring) for the target date.
/// Entries from the target date itself are excluded (they're regular entries).
/// Returns entries with appropriate SourceType (Later or Recurring).
pub fn collect_projected_entries_for_date(
    target_date: NaiveDate,
    path: &Path,
) -> io::Result<Vec<Entry>> {
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
            if let Some(Line::Entry(raw_entry)) = parsed.first() {
                if let Some(entry_target) = extract_target_date(&raw_entry.content, source_date)
                    && entry_target == target_date
                {
                    entries.push(Entry::from_raw(
                        raw_entry,
                        source_date,
                        line_index_in_day,
                        SourceType::Later,
                    ));
                }
                // Check for @recurring pattern (repeating projection)
                else if let Some(pattern) = extract_recurring_pattern(&raw_entry.content)
                    && pattern.matches(target_date)
                {
                    entries.push(Entry::from_raw(
                        raw_entry,
                        source_date,
                        line_index_in_day,
                        SourceType::Recurring,
                    ));
                }
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
        if token == "@later" {
            filter.later = true;
            continue;
        }
        if token == "@recurring" {
            filter.recurring = true;
            continue;
        }
        if token.starts_with('@') {
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

/// Collects entries matching the filter criteria.
/// Returns entries with SourceType::Local (filter results are from their source day).
pub fn collect_filtered_entries(filter: &Filter, path: &Path) -> io::Result<Vec<Entry>> {
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
            if let Some(Line::Entry(raw_entry)) = parsed.first() {
                // Overdue filter: entry must have @date targeting before today
                if filter.overdue {
                    let target = extract_target_date_prefer_past(&raw_entry.content, today);
                    if target.is_none() || target.unwrap() >= today {
                        line_index_in_day += 1;
                        continue;
                    }
                }

                // Later filter: entry must have a @date pattern
                if filter.later
                    && !LATER_DATE_REGEX.is_match(&raw_entry.content)
                    && !RELATIVE_DATE_REGEX.is_match(&raw_entry.content)
                {
                    line_index_in_day += 1;
                    continue;
                }

                // Recurring filter: entry must have an @every-* pattern
                if filter.recurring && !RECURRING_REGEX.is_match(&raw_entry.content) {
                    line_index_in_day += 1;
                    continue;
                }

                if entry_matches_filter(raw_entry, filter) {
                    entries.push(Entry::from_raw(
                        raw_entry,
                        source_date,
                        line_index_in_day,
                        SourceType::Local,
                    ));
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

fn entry_matches_filter(entry: &RawEntry, filter: &Filter) -> bool {
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
