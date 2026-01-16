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
    pub recurring: bool,
    pub invalid_tokens: Vec<String>,
}

pub static TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#([a-zA-Z][a-zA-Z0-9_-]*)").unwrap());

/// The character class for valid tag characters (after the first letter)
pub const TAG_CHAR_CLASS: &str = "[a-zA-Z0-9_-]";

/// Create a regex that matches a specific tag (case-insensitive) with word boundary
pub fn create_tag_match_regex(tag: &str) -> Result<Regex, regex::Error> {
    Regex::new(&format!(r"(?i)#{}", regex::escape(tag)))
}

/// Create a regex for tag deletion - includes preceding space to avoid double spaces
pub fn create_tag_delete_regex(tag: &str) -> Result<Regex, regex::Error> {
    Regex::new(&format!(r"(?i)\s?#{}", regex::escape(tag)))
}

/// Matches trailing tags (one or more tags at end of line)
pub static TRAILING_TAGS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(\s+#[a-zA-Z]{}*)+\s*$", TAG_CHAR_CLASS)).unwrap());

/// Matches the last trailing tag at end of line
pub static LAST_TRAILING_TAG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\s+#[a-zA-Z]{}*\s*$", TAG_CHAR_CLASS)).unwrap());

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

/// Matches <!-- done: ... --> metadata comment at end of content
static DONE_META_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*<!--\s*done:\s*([^>]*)\s*-->").unwrap());

/// Extracts completion dates from entry content's <!-- done: ... --> comment.
fn extract_done_dates(content: &str) -> Vec<NaiveDate> {
    DONE_META_REGEX
        .captures(content)
        .and_then(|caps| caps.get(1))
        .map(|m| {
            m.as_str()
                .split(',')
                .filter_map(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Checks if a specific date is marked as done in the entry content.
#[must_use]
pub fn is_done_on_date(content: &str, date: NaiveDate) -> bool {
    extract_done_dates(content).contains(&date)
}

/// Formats content with done metadata comment. Returns base unchanged if dates is empty.
fn format_done_meta(base: &str, dates: &[NaiveDate]) -> String {
    if dates.is_empty() {
        return base.to_string();
    }
    let dates_str = dates
        .iter()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{base} <!-- done: {dates_str} -->")
}

/// Adds a date to the done list in entry content. Returns the new content.
#[must_use]
pub fn add_done_date(content: &str, date: NaiveDate) -> String {
    let mut dates = extract_done_dates(content);
    if dates.contains(&date) {
        return content.to_string();
    }
    dates.push(date);
    dates.sort();
    format_done_meta(&strip_done_meta(content), &dates)
}

/// Removes a date from the done list in entry content. Returns the new content.
#[must_use]
pub fn remove_done_date(content: &str, date: NaiveDate) -> String {
    let mut dates = extract_done_dates(content);
    dates.retain(|d| d != &date);
    format_done_meta(&strip_done_meta(content), &dates)
}

/// Strips the <!-- done: ... --> metadata from content for display.
#[must_use]
pub fn strip_done_meta(content: &str) -> String {
    DONE_META_REGEX.replace(content, "").trim().to_string()
}

/// Transfers done metadata from original content to new content.
/// Used when editing entries to preserve completion tracking.
#[must_use]
pub fn restore_done_meta(new_content: &str, original: &str) -> String {
    let done_dates = extract_done_dates(original);
    format_done_meta(new_content, &done_dates)
}

/// Checks if a token looks like spread date syntax (not plain text search).
/// Spread syntax includes: DATE, DATE.., ..DATE, DATE..DATE
/// Where DATE can be: mm/dd, mm/dd/yy, mm/dd/yyyy, yyyy/mm/dd, d[1-999][+], weekday[+]
fn is_spread_syntax(token: &str) -> bool {
    // Contains ".." -> definitely spread syntax
    if token.contains("..") {
        return true;
    }
    // Absolute date: starts with digit, contains /
    if token.chars().next().is_some_and(|c| c.is_ascii_digit()) && token.contains('/') {
        return true;
    }
    // Relative days: d followed by digit (d1-d999, optionally with +)
    if token.starts_with('d') && token.chars().nth(1).is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    // Weekday (mon, tue, etc. optionally with +)
    let base = token.strip_suffix('+').unwrap_or(token);
    matches!(base, "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun")
}

/// Parses spread date syntax and returns (before_date, after_date).
/// Spread syntax:
/// - Single date: exact match (before=date, after=date)
/// - DATE.. (past): from date to today
/// - DATE+.. (future): from date to infinity
/// - ..DATE (past): all past through date
/// - ..DATE+ (future): from today to date
/// - DATE..DATE: between two dates
fn parse_spread_date(
    token: &str,
    today: NaiveDate,
) -> Option<(Option<NaiveDate>, Option<NaiveDate>)> {
    let Some((start, end)) = token.split_once("..") else {
        let date = parse_filter_date(token, today)?;
        return Some((Some(date), Some(date)));
    };

    if start.is_empty() && end.is_empty() {
        return None;
    }

    let start_is_future = start.ends_with('+');
    let end_is_future = end.ends_with('+');

    let start_date = (!start.is_empty())
        .then(|| parse_filter_date(start, today))
        .flatten();
    let end_date = (!end.is_empty())
        .then(|| parse_filter_date(end, today))
        .flatten();

    if (!start.is_empty() && start_date.is_none()) || (!end.is_empty() && end_date.is_none()) {
        return None;
    }

    match (start.is_empty(), end.is_empty()) {
        (true, false) if end_is_future => Some((end_date, Some(today))),
        (true, false) => Some((end_date, None)),
        (false, true) if start_is_future => Some((None, start_date)),
        (false, true) => Some((Some(today), start_date)),
        (false, false) => Some((end_date, start_date)),
        _ => None,
    }
}

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

/// Normalizes entry structure to: [content] [recurring_dates] [#tags]
///
/// - Trailing section = contiguous dates/tags at end (only whitespace between them)
/// - Inline #tags (in content section) have # stripped
/// - @every-* patterns are extracted from anywhere and moved to structure
#[must_use]
pub fn normalize_entry_structure(content: &str) -> (String, Option<String>) {
    let recurring_dates: Vec<_> = RECURRING_REGEX.find_iter(content).collect();
    let tags: Vec<_> = TAG_REGEX.find_iter(content).collect();

    if recurring_dates.is_empty() && tags.is_empty() {
        return (content.to_string(), None);
    }

    let trailing_start = find_trailing_section_start(content, &recurring_dates, &tags);

    let (trailing_tags, inline_tags): (Vec<&regex::Match>, Vec<&regex::Match>) =
        tags.iter().partition(|t| t.start() >= trailing_start);

    // Removals: (start, end, replacement) - replacement None means delete
    let mut removals: Vec<(usize, usize, Option<&str>)> = Vec::new();
    for m in &recurring_dates {
        removals.push((m.start(), m.end(), None));
    }
    for m in &trailing_tags {
        removals.push((m.start(), m.end(), None));
    }
    for m in &inline_tags {
        removals.push((m.start(), m.end(), Some(&m.as_str()[1..])));
    }

    // Sort descending so we can modify from end to start without invalidating positions
    removals.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = content.to_string();
    for (start, end, replacement) in removals {
        result.replace_range(start..end, replacement.unwrap_or(""));
    }

    let result = result.split_whitespace().collect::<Vec<_>>().join(" ");

    // Reconstruct: [content] [recurring] [tags]
    let mut final_parts = vec![result];
    for m in &recurring_dates {
        final_parts.push(m.as_str().trim().to_string());
    }
    for m in &trailing_tags {
        final_parts.push(m.as_str().to_string());
    }

    (final_parts.join(" "), None)
}

/// Find the byte position where the trailing section starts.
/// Trailing section = contiguous sequence of recurring patterns and tags at the end,
/// with only whitespace between them.
fn find_trailing_section_start(
    content: &str,
    recurring_dates: &[regex::Match],
    tags: &[regex::Match],
) -> usize {
    let mut patterns: Vec<(usize, usize)> = recurring_dates
        .iter()
        .chain(tags.iter())
        .map(|m| (m.start(), m.end()))
        .collect();

    if patterns.is_empty() {
        return content.len();
    }

    patterns.sort_by_key(|p| p.0);

    let content_bytes = content.as_bytes();
    let mut trailing_start = content.len();

    // Walk backwards, including patterns that connect to trailing section via whitespace only
    for &(pat_start, pat_end) in patterns.iter().rev() {
        let gap = &content_bytes[pat_end..trailing_start];
        if gap.iter().all(|&b| b.is_ascii_whitespace()) {
            trailing_start = pat_start;
        } else {
            break;
        }
    }

    // Trim any whitespace before the first trailing pattern
    if trailing_start > 0 && trailing_start < content.len() {
        trailing_start = content[..trailing_start].trim_end().len();
    }

    trailing_start
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

/// Collects all recurring projected entries for the target date.
/// Entries from the target date itself are excluded (they're regular entries).
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
            if source_date == target_date {
                line_index_in_day += 1;
                continue;
            }

            let parsed = parse_lines(line);
            if let Some(Line::Entry(raw_entry)) = parsed.first()
                && let Some(pattern) = extract_recurring_pattern(&raw_entry.content)
                && target_date >= source_date
                && pattern.matches(target_date)
            {
                let is_done = is_done_on_date(&raw_entry.content, target_date);
                let entry_type = if is_done {
                    EntryType::Task { completed: true }
                } else {
                    raw_entry.entry_type.clone()
                };

                entries.push(Entry {
                    entry_type,
                    content: strip_done_meta(&raw_entry.content),
                    source_date,
                    line_index: line_index_in_day,
                    source_type: SourceType::Recurring,
                });
            }
            line_index_in_day += 1;
        }
    }

    entries.sort_by_key(|e| e.source_date);
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
        // Spread date syntax: DATE, DATE.., ..DATE, DATE..DATE
        // Dates default to past (d7 = 7 days ago, mon = last Monday)
        // Append + for explicit future (d7+ = 7 days from now, mon+ = next Monday)
        if is_spread_syntax(token) {
            if filter.before_date.is_some() || filter.after_date.is_some() {
                filter
                    .invalid_tokens
                    .push("Multiple date ranges".to_string());
            } else if let Some((before, after)) = parse_spread_date(token, today) {
                filter.before_date = before;
                filter.after_date = after;
            } else {
                filter.invalid_tokens.push(token.to_string());
            }
            continue;
        }

        // Content-based filters: @recurring
        if token == "@recurring" {
            filter.recurring = true;
            continue;
        }
        if token.starts_with('@') {
            filter.invalid_tokens.push(token.to_string());
            continue;
        }

        if let Some(negated) = token.strip_prefix('-') {
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
                // @recurring shows only recurring; otherwise recurring entries are excluded
                let is_recurring = RECURRING_REGEX.is_match(&raw_entry.content);
                if filter.recurring != is_recurring {
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
