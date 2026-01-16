use ratatui::style::Color;
use regex::Regex;
use std::sync::LazyLock;

use crate::registry::{
    COMMANDS, Command, DATE_VALUES, DateScope, DateValue, FILTER_SYNTAX, FilterCategory,
    FilterSyntax,
};
use crate::ui::theme;

/// Compiled regexes for pattern-based date values, keyed by pattern string.
static PATTERN_CACHE: LazyLock<std::collections::HashMap<&'static str, Regex>> =
    LazyLock::new(|| {
        DATE_VALUES
            .iter()
            .filter_map(|dv| {
                dv.pattern
                    .map(|p| (p, Regex::new(p).expect("Invalid date value pattern")))
            })
            .collect()
    });

/// What kind of hints to display
#[derive(Clone, Debug, PartialEq)]
pub enum HintContext {
    /// No hints to display
    Inactive,
    /// Guidance text (help message only, shown at bottom)
    GuidanceMessage { message: &'static str },
    /// Tag hints from current journal
    Tags {
        prefix: String,
        matches: Vec<String>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Command hints (from registry)
    Commands {
        prefix: String,
        matches: Vec<&'static Command>,
    },
    /// Filter type hints (!tasks, !notes, etc.)
    FilterTypes {
        prefix: String,
        matches: Vec<&'static FilterSyntax>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Content pattern hints (@recurring)
    DateOps {
        prefix: String,
        matches: Vec<&'static FilterSyntax>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Date value hints (entry dates or filter date values)
    DateValues {
        prefix: String,
        scope: DateScope,
        matches: Vec<&'static DateValue>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Saved filter hints ($name)
    SavedFilters {
        prefix: String,
        matches: Vec<String>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Negation hints - wraps inner context for recursive hints
    Negation { inner: Box<HintContext> },
}

/// Which input context we're computing hints for
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HintMode {
    /// Command mode (:)
    Command,
    /// Filter query mode (/)
    Filter,
    /// Entry editing mode
    Entry,
}

#[derive(Clone, Debug)]
pub struct HintItem {
    pub label: String,
    pub selectable: bool,
}

impl HintContext {
    /// Compute hints based on current input buffer and mode
    #[must_use]
    pub fn compute(
        input: &str,
        mode: HintMode,
        journal_tags: &[String],
        saved_filters: &[String],
    ) -> Self {
        match mode {
            HintMode::Command => Self::compute_command_hints(input),
            HintMode::Filter => Self::compute_filter_hints(input, journal_tags, saved_filters),
            HintMode::Entry => Self::compute_entry_hints(input, journal_tags),
        }
    }

    /// Strip optional +/- suffix from input, returning (base, suffix)
    fn strip_direction_suffix(input: &str) -> (&str, Option<char>) {
        if let Some(base) = input.strip_suffix('+') {
            (base, Some('+'))
        } else if let Some(base) = input.strip_suffix('-') {
            (base, Some('-'))
        } else {
            (input, None)
        }
    }

    /// Check if input matches a date value (returns true if input is a valid prefix).
    /// Handles values-based, pattern-based, and literal matching.
    fn matches_date_value(input: &str, dv: &DateValue) -> bool {
        let input_lower = input.to_lowercase();
        let (base, _suffix) = Self::strip_direction_suffix(&input_lower);

        // Values-based matching: input is prefix of any enumerated value
        if let Some(values) = dv.values {
            return values.iter().any(|v| v.starts_with(base));
        }

        if let Some(pattern_str) = dv.pattern
            && let Some(regex) = PATTERN_CACHE.get(pattern_str)
        {
            if regex.is_match(&input_lower) {
                return true;
            }
            return Self::is_valid_pattern_prefix(base, regex, pattern_str);
        }

        dv.syntax.to_lowercase().starts_with(base)
    }

    /// Check if input is a valid prefix for a pattern (could complete to match the regex).
    fn is_valid_pattern_prefix(input: &str, regex: &Regex, pattern_str: &str) -> bool {
        // For d[1-999] pattern: ^d[1-9][0-9]{0,2}$
        if pattern_str.starts_with("^d") {
            return Self::is_valid_d_prefix(input);
        }

        // For every-[1-31] pattern: ^every-([1-9]|[12][0-9]|3[01])$
        if pattern_str.contains("every-") && pattern_str.contains("[1-9]") {
            return Self::is_valid_every_number_prefix(input);
        }

        // Default: try adding digits/letters to see if it could match
        // This is a simple heuristic - if current input + "1" or input + "a" matches, it's valid
        regex.is_match(&format!("{input}1"))
            || regex.is_match(&format!("{input}a"))
            || regex.is_match(input)
    }

    /// Check if input is a valid prefix for d[1-999] pattern.
    fn is_valid_d_prefix(input: &str) -> bool {
        if input == "d" {
            return true;
        }
        if let Some(rest) = input.strip_prefix('d') {
            // Must be 1-3 digits, first digit non-zero
            !rest.is_empty()
                && rest.len() <= 3
                && rest.chars().all(|c| c.is_ascii_digit())
                && !rest.starts_with('0')
        } else {
            false
        }
    }

    /// Check if input is a valid prefix for every-[1-31] pattern.
    fn is_valid_every_number_prefix(input: &str) -> bool {
        // Must start with "every-" or be a prefix of it
        if !"every-".starts_with(input) && !input.starts_with("every-") {
            return false;
        }

        if let Some(rest) = input.strip_prefix("every-") {
            if rest.is_empty() {
                return true;
            }
            // Must be valid day of month prefix (1-31)
            if let Ok(n) = rest.parse::<u32>() {
                (1..=31).contains(&n)
            } else {
                false
            }
        } else {
            // Still typing "every-"
            true
        }
    }

    #[allow(dead_code)]
    fn compute_date_completion(input: &str, dv: &DateValue) -> Option<String> {
        let input_lower = input.to_lowercase();
        let (base, suffix) = Self::strip_direction_suffix(&input_lower);

        if suffix.is_some() {
            return Some(String::new());
        }

        if let Some(values) = dv.values {
            for value in values {
                if let Some(remainder) = value.strip_prefix(base) {
                    return Some(remainder.to_string());
                }
            }
            return None;
        }

        if dv.pattern.is_some() {
            return Some(String::new());
        }

        let syntax_lower = dv.syntax.to_lowercase();
        if syntax_lower.starts_with(base) {
            Some(dv.syntax[base.len()..].to_string())
        } else {
            None
        }
    }

    fn compute_command_hints(input: &str) -> Self {
        let prefix = input.trim();

        if prefix.contains(' ') {
            return Self::Inactive;
        }

        let matches: Vec<&'static Command> = COMMANDS
            .iter()
            .filter(|c| c.name.starts_with(prefix))
            .collect();

        if matches.is_empty() {
            Self::Inactive
        } else {
            Self::Commands {
                prefix: prefix.to_string(),
                matches,
            }
        }
    }

    fn match_tags(prefix: &str, journal_tags: &[String]) -> Option<(String, Vec<String>)> {
        let matches: Vec<String> = journal_tags
            .iter()
            .filter(|t| t.to_lowercase().starts_with(&prefix.to_lowercase()))
            .cloned()
            .collect();

        if matches.is_empty() || (matches.len() == 1 && matches[0].eq_ignore_ascii_case(prefix)) {
            None
        } else {
            Some((prefix.to_string(), matches))
        }
    }

    fn compute_tag_hints(input: &str, journal_tags: &[String]) -> Self {
        if input.ends_with(' ') {
            return Self::Inactive;
        }

        let current_token = input.split_whitespace().last().unwrap_or("");

        if let Some(tag_prefix) = current_token.strip_prefix('#')
            && let Some((prefix, matches)) = Self::match_tags(tag_prefix, journal_tags)
        {
            return Self::Tags {
                prefix,
                matches,
                selected: 0,
                scroll_offset: 0,
            };
        }

        Self::Inactive
    }

    fn compute_entry_hints(input: &str, journal_tags: &[String]) -> Self {
        if let Some(hint) = Self::compute_entry_date_hints(input) {
            return hint;
        }
        Self::compute_tag_hints(input, journal_tags)
    }

    fn compute_entry_date_hints(input: &str) -> Option<Self> {
        if input.ends_with(' ') {
            return None;
        }

        let current_token = input.split_whitespace().last()?;
        let date_prefix = current_token.strip_prefix('@')?;

        // Empty prefix: show all entry dates
        if date_prefix.is_empty() {
            let matches: Vec<&'static DateValue> = DATE_VALUES
                .iter()
                .filter(|dv| dv.scopes.contains(&DateScope::Entry))
                .collect();
            let selected = Self::first_selectable_index(&Self::date_display_items(
                &DateScope::Entry,
                &matches,
            ));
            return Some(Self::DateValues {
                prefix: date_prefix.to_string(),
                scope: DateScope::Entry,
                matches,
                selected,
                scroll_offset: 0,
            });
        }

        // Find all matching date values using unified matching
        let matches: Vec<&'static DateValue> = DATE_VALUES
            .iter()
            .filter(|dv| dv.scopes.contains(&DateScope::Entry))
            .filter(|dv| Self::matches_date_value(date_prefix, dv))
            .collect();

        if matches.is_empty() {
            return None;
        }

        // Check if we have an exact complete match (hide hints)
        let prefix_lower = date_prefix.to_lowercase();
        let (base, _) = Self::strip_direction_suffix(&prefix_lower);
        let is_exact_match = matches.len() == 1
            && matches[0].values.is_none()
            && matches[0].pattern.is_none()
            && matches[0].syntax.eq_ignore_ascii_case(base);

        if is_exact_match {
            return None;
        }

        let selected =
            Self::first_selectable_index(&Self::date_display_items(&DateScope::Entry, &matches));
        Some(Self::DateValues {
            prefix: date_prefix.to_string(),
            scope: DateScope::Entry,
            matches,
            selected,
            scroll_offset: 0,
        })
    }

    fn compute_filter_hints(
        input: &str,
        journal_tags: &[String],
        saved_filters: &[String],
    ) -> Self {
        if input.is_empty() {
            return Self::GuidanceMessage {
                message: "Type to search, or use ! @ # $ - for filters",
            };
        }

        if input.ends_with(' ') {
            return Self::Inactive;
        }

        let current_token = input.split_whitespace().last().unwrap_or("");

        if let Some(neg_suffix) = current_token.strip_prefix('-') {
            let inner = Self::compute_filter_token(neg_suffix, journal_tags, saved_filters);
            if matches!(inner, Self::Inactive) && neg_suffix.is_empty() {
                return Self::Negation {
                    inner: Box::new(Self::GuidanceMessage {
                        message: "! # or text to negate",
                    }),
                };
            }
            if matches!(inner, Self::Inactive) {
                return Self::Inactive;
            }
            return Self::Negation {
                inner: Box::new(inner),
            };
        }

        Self::compute_filter_token(current_token, journal_tags, saved_filters)
    }

    fn compute_filter_token(
        token: &str,
        journal_tags: &[String],
        saved_filters: &[String],
    ) -> Self {
        if let Some(tag_prefix) = token.strip_prefix('#')
            && let Some((prefix, matches)) = Self::match_tags(tag_prefix, journal_tags)
        {
            return Self::Tags {
                prefix,
                matches,
                selected: 0,
                scroll_offset: 0,
            };
        }

        if let Some(type_prefix) = token.strip_prefix('!') {
            let matches: Vec<&'static FilterSyntax> = FILTER_SYNTAX
                .iter()
                .filter(|f| f.category == FilterCategory::EntryType)
                .filter(|f| {
                    f.syntax
                        .get(1..)
                        .is_some_and(|s| s.starts_with(type_prefix))
                })
                .collect();

            if matches.is_empty() {
                return Self::Inactive;
            }
            return Self::FilterTypes {
                prefix: type_prefix.to_string(),
                matches,
                selected: 0,
                scroll_offset: 0,
            };
        }

        // Content pattern filters: @recurring
        if let Some(content_prefix) = token.strip_prefix('@') {
            let matches: Vec<&'static FilterSyntax> = FILTER_SYNTAX
                .iter()
                .filter(|f| f.category == FilterCategory::ContentPattern)
                .filter(|f| {
                    f.syntax
                        .get(1..)
                        .is_some_and(|s| s.starts_with(content_prefix))
                })
                .collect();

            if matches.is_empty() {
                return Self::Inactive;
            }
            return Self::DateOps {
                prefix: content_prefix.to_string(),
                matches,
                selected: 0,
                scroll_offset: 0,
            };
        }

        if let Some(filter_prefix) = token.strip_prefix('$') {
            let matches: Vec<String> = saved_filters
                .iter()
                .filter(|f| f.to_lowercase().starts_with(&filter_prefix.to_lowercase()))
                .cloned()
                .collect();

            if matches.is_empty()
                || (matches.len() == 1 && matches[0].eq_ignore_ascii_case(filter_prefix))
            {
                return Self::Inactive;
            }
            return Self::SavedFilters {
                prefix: filter_prefix.to_string(),
                matches,
                selected: 0,
                scroll_offset: 0,
            };
        }

        Self::Inactive
    }

    fn suffix_after(s: &str, prefix_len: usize) -> String {
        s.get(prefix_len..).unwrap_or("").to_string()
    }

    #[must_use]
    pub fn first_completion(&self) -> Option<String> {
        match self {
            Self::Inactive | Self::GuidanceMessage { .. } => None,
            Self::Tags {
                prefix,
                matches,
                selected,
                ..
            } => matches
                .get(*selected)
                .map(|t| Self::suffix_after(t, prefix.len())),
            Self::Commands { prefix, matches } => matches
                .first()
                .map(|c| Self::suffix_after(c.name, prefix.len())),
            Self::FilterTypes {
                prefix,
                matches,
                selected,
                ..
            } => matches
                .get(*selected)
                .map(|f| Self::suffix_after(f.syntax, 1 + prefix.len())),
            Self::DateOps {
                prefix,
                matches,
                selected,
                ..
            } => matches
                .get(*selected)
                .map(|f| Self::suffix_after(f.syntax, 1 + prefix.len())),
            Self::DateValues {
                prefix,
                matches,
                selected,
                scope,
                ..
            } => {
                let options = Self::date_display_items(scope, matches);
                options.get(*selected).and_then(|item| {
                    if !item.selectable {
                        return None;
                    }
                    let trimmed = item.label.trim_start_matches('@');
                    Some(Self::suffix_after(trimmed, prefix.len()))
                })
            }
            Self::SavedFilters {
                prefix,
                matches,
                selected,
                ..
            } => matches
                .get(*selected)
                .map(|f| Self::suffix_after(f, prefix.len())),
            Self::Negation { inner } => inner.first_completion(),
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive)
    }

    /// Get help text/description for the current hint context
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        let effective = match self {
            Self::Negation { inner } => inner.as_ref(),
            other => other,
        };

        match effective {
            Self::Commands { prefix, matches } if !prefix.is_empty() => {
                matches.first().map(|c| c.help)
            }
            Self::FilterTypes {
                prefix,
                matches,
                selected,
                ..
            } if !prefix.is_empty() => matches.get(*selected).map(|f| f.help),
            Self::DateOps {
                prefix,
                matches,
                selected,
                ..
            } if !prefix.is_empty() => matches.get(*selected).map(|f| f.help),
            Self::DateValues {
                prefix,
                matches,
                selected,
                ..
            } if !prefix.is_empty() => matches.get(*selected).map(|dv| dv.completion_hint),
            Self::DateValues {
                scope: DateScope::Filter,
                ..
            } => Some("Dates default to past. Append + for future."),
            _ => None,
        }
    }

    /// Get the display color for this hint context
    #[must_use]
    pub fn color(&self) -> Color {
        let effective = match self {
            Self::Negation { inner } => inner.as_ref(),
            other => other,
        };

        match effective {
            Self::Tags { .. } => theme::TAG,
            Self::Commands { .. } => theme::HUB_PRIMARY,
            Self::FilterTypes { .. } | Self::DateOps { .. } => Color::Magenta,
            Self::DateValues { .. } => theme::PROJECTED_DATE,
            Self::SavedFilters { .. } => Color::Magenta,
            Self::Inactive | Self::GuidanceMessage { .. } | Self::Negation { .. } => Color::Reset,
        }
    }

    /// Get formatted display items for rendering
    #[must_use]
    pub fn display_items(&self, negation_prefix: &str) -> Vec<HintItem> {
        let effective = match self {
            Self::Negation { inner } => inner.as_ref(),
            other => other,
        };

        match effective {
            Self::Inactive | Self::GuidanceMessage { .. } | Self::Negation { .. } => vec![],
            Self::Tags { matches, .. } => matches
                .iter()
                .map(|t| HintItem {
                    label: format!("{}#{t}", negation_prefix),
                    selectable: true,
                })
                .collect(),
            Self::Commands { matches, .. } => matches
                .iter()
                .map(|cmd| HintItem {
                    label: format!(":{}", cmd.name),
                    selectable: true,
                })
                .collect(),
            Self::FilterTypes { matches, .. } => matches
                .iter()
                .map(|f| HintItem {
                    label: format!("{}{}", negation_prefix, f.syntax),
                    selectable: true,
                })
                .collect(),
            Self::DateOps { matches, .. } => matches
                .iter()
                .map(|f| HintItem {
                    label: format!("{}{}", negation_prefix, f.syntax),
                    selectable: true,
                })
                .collect(),
            Self::DateValues { matches, scope, .. } => Self::date_display_items(scope, matches),
            Self::SavedFilters { matches, .. } => matches
                .iter()
                .map(|f| HintItem {
                    label: format!("{}${f}", negation_prefix),
                    selectable: true,
                })
                .collect(),
        }
    }

    fn format_date_value(scope: &DateScope, value: &str) -> String {
        match scope {
            DateScope::Entry => {
                if value.starts_with('@') {
                    value.to_string()
                } else {
                    format!("@{value}")
                }
            }
            DateScope::Filter => value.to_string(),
        }
    }

    fn date_display_items(scope: &DateScope, matches: &[&'static DateValue]) -> Vec<HintItem> {
        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();

        for dv in matches {
            let selectable = !(dv.pattern.is_some() && dv.values.is_none());
            if let Some(values) = dv.values {
                for value in values {
                    let label = Self::format_date_value(scope, value);
                    if seen.insert(label.clone()) {
                        items.push(HintItem { label, selectable });
                    }
                }
            } else {
                let label = Self::format_date_value(scope, dv.display);
                if seen.insert(label.clone()) {
                    items.push(HintItem { label, selectable });
                }
            }
        }

        items
    }

    fn first_selectable_index(items: &[HintItem]) -> usize {
        items.iter().position(|item| item.selectable).unwrap_or(0)
    }

    const VISIBLE_HINTS: usize = 5;

    fn adjust_scroll_offset(selected: usize, scroll_offset: &mut usize) {
        if selected >= *scroll_offset + Self::VISIBLE_HINTS {
            *scroll_offset = selected + 1 - Self::VISIBLE_HINTS;
        }
        if selected < *scroll_offset {
            *scroll_offset = selected;
        }
    }

    pub fn select_next(&mut self) {
        match self {
            Self::Tags {
                matches,
                selected,
                scroll_offset,
                ..
            } => {
                if !matches.is_empty() {
                    *selected = (*selected + 1).min(matches.len().saturating_sub(1));
                    Self::adjust_scroll_offset(*selected, scroll_offset);
                }
            }
            Self::FilterTypes {
                matches,
                selected,
                scroll_offset,
                ..
            }
            | Self::DateOps {
                matches,
                selected,
                scroll_offset,
                ..
            } => {
                if !matches.is_empty() {
                    *selected = (*selected + 1).min(matches.len().saturating_sub(1));
                    Self::adjust_scroll_offset(*selected, scroll_offset);
                }
            }
            Self::DateValues {
                matches,
                scope,
                selected,
                scroll_offset,
                ..
            } => {
                let items = Self::date_display_items(scope, matches);
                if !items.is_empty() {
                    let start = (*selected).saturating_add(1);
                    if let Some((index, _)) = items
                        .iter()
                        .enumerate()
                        .skip(start)
                        .find(|(_, item)| item.selectable)
                    {
                        *selected = index;
                        Self::adjust_scroll_offset(*selected, scroll_offset);
                    }
                }
            }
            Self::SavedFilters {
                matches,
                selected,
                scroll_offset,
                ..
            } => {
                if !matches.is_empty() {
                    *selected = (*selected + 1).min(matches.len().saturating_sub(1));
                    Self::adjust_scroll_offset(*selected, scroll_offset);
                }
            }
            _ => {}
        }
    }

    pub fn select_prev(&mut self) {
        match self {
            Self::Tags {
                selected,
                scroll_offset,
                ..
            }
            | Self::FilterTypes {
                selected,
                scroll_offset,
                ..
            }
            | Self::DateOps {
                selected,
                scroll_offset,
                ..
            }
            | Self::SavedFilters {
                selected,
                scroll_offset,
                ..
            } => {
                *selected = selected.saturating_sub(1);
                Self::adjust_scroll_offset(*selected, scroll_offset);
            }
            Self::DateValues {
                matches,
                scope,
                selected,
                scroll_offset,
                ..
            } => {
                if *selected == 0 {
                    return;
                }
                let items = Self::date_display_items(scope, matches);
                if let Some((index, _)) = items
                    .iter()
                    .enumerate()
                    .take(*selected)
                    .rev()
                    .find(|(_, item)| item.selectable)
                {
                    *selected = index;
                    Self::adjust_scroll_offset(*selected, scroll_offset);
                }
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn with_previous_selection(self, previous: &HintContext) -> Self {
        match (self, previous) {
            (
                HintContext::Tags {
                    prefix,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::Tags {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let selected = (*selected).min(matches.len().saturating_sub(1));
                HintContext::Tags {
                    prefix,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (
                HintContext::FilterTypes {
                    prefix,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::FilterTypes {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let selected = (*selected).min(matches.len().saturating_sub(1));
                HintContext::FilterTypes {
                    prefix,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (
                HintContext::DateOps {
                    prefix,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::DateOps {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let selected = (*selected).min(matches.len().saturating_sub(1));
                HintContext::DateOps {
                    prefix,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (
                HintContext::DateValues {
                    prefix,
                    scope,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::DateValues {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let items = Self::date_display_items(&scope, &matches);
                let mut selected = (*selected).min(items.len().saturating_sub(1));
                if !items.get(selected).is_some_and(|item| item.selectable) {
                    selected = Self::first_selectable_index(&items);
                }
                HintContext::DateValues {
                    prefix,
                    scope,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (
                HintContext::SavedFilters {
                    prefix,
                    matches,
                    selected: _,
                    ..
                },
                HintContext::SavedFilters {
                    selected,
                    scroll_offset,
                    ..
                },
            ) => {
                let selected = (*selected).min(matches.len().saturating_sub(1));
                HintContext::SavedFilters {
                    prefix,
                    matches,
                    selected,
                    scroll_offset: *scroll_offset,
                }
            }
            (next, _) => next,
        }
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        match self {
            Self::Tags { selected, .. }
            | Self::FilterTypes { selected, .. }
            | Self::DateOps { selected, .. }
            | Self::DateValues { selected, .. }
            | Self::SavedFilters { selected, .. } => *selected,
            _ => 0,
        }
    }

    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        match self {
            Self::Tags { scroll_offset, .. }
            | Self::FilterTypes { scroll_offset, .. }
            | Self::DateOps { scroll_offset, .. }
            | Self::DateValues { scroll_offset, .. }
            | Self::SavedFilters { scroll_offset, .. } => *scroll_offset,
            _ => 0,
        }
    }
}
