use crate::registry::{
    COMMANDS, DATE_VALUES, DateScope, FILTER_SYNTAX, FilterCategory,
};

use super::display::first_selectable_index;
use super::patterns::{matches_date_value, strip_direction_suffix};
use super::types::{HintContext, HintItem, HintMode};

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

    fn compute_command_hints(input: &str) -> Self {
        let prefix = input.trim();

        if prefix.contains(' ') {
            return Self::Inactive;
        }

        let matches: Vec<_> = COMMANDS
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
            let matches: Vec<_> = DATE_VALUES
                .iter()
                .filter(|dv| dv.scopes.contains(&DateScope::Entry))
                .collect();
            let selected = first_selectable_index(&date_display_items(
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
        let matches: Vec<_> = DATE_VALUES
            .iter()
            .filter(|dv| dv.scopes.contains(&DateScope::Entry))
            .filter(|dv| matches_date_value(date_prefix, dv))
            .collect();

        if matches.is_empty() {
            return None;
        }

        // Check if we have an exact complete match (hide hints)
        let prefix_lower = date_prefix.to_lowercase();
        let (base, _) = strip_direction_suffix(&prefix_lower);
        let is_exact_match = matches.len() == 1
            && matches[0].values.is_none()
            && matches[0].pattern.is_none()
            && matches[0].syntax.eq_ignore_ascii_case(base);

        if is_exact_match {
            return None;
        }

        let selected =
            first_selectable_index(&date_display_items(&DateScope::Entry, &matches));
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
            let matches: Vec<_> = FILTER_SYNTAX
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
            let matches: Vec<_> = FILTER_SYNTAX
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

pub(super) fn date_display_items(
    scope: &DateScope,
    matches: &[&'static crate::registry::DateValue],
) -> Vec<HintItem> {
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    for dv in matches {
        let selectable = !(dv.pattern.is_some() && dv.values.is_none());
        if let Some(values) = dv.values {
            for value in values {
                let label = format_date_value(scope, value);
                if seen.insert(label.clone()) {
                    items.push(HintItem { label, selectable });
                }
            }
        } else {
            let label = format_date_value(scope, dv.display);
            if seen.insert(label.clone()) {
                items.push(HintItem { label, selectable });
            }
        }
    }

    items
}
