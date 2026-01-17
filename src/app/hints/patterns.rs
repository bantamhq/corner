use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::registry::{DATE_VALUES, DateValue};

pub(super) static PATTERN_CACHE: LazyLock<HashMap<&'static str, Regex>> = LazyLock::new(|| {
    DATE_VALUES
        .iter()
        .filter_map(|dv| {
            dv.pattern
                .map(|p| (p, Regex::new(p).expect("Invalid date value pattern")))
        })
        .collect()
});

pub(super) fn strip_direction_suffix(input: &str) -> (&str, Option<char>) {
    if let Some(base) = input.strip_suffix('+') {
        (base, Some('+'))
    } else if let Some(base) = input.strip_suffix('-') {
        (base, Some('-'))
    } else {
        (input, None)
    }
}

pub(super) fn matches_date_value(input: &str, dv: &DateValue) -> bool {
    let input_lower = input.to_lowercase();
    let (base, _suffix) = strip_direction_suffix(&input_lower);

    if let Some(values) = dv.values {
        return values.iter().any(|v| v.starts_with(base));
    }

    if let Some(pattern_str) = dv.pattern
        && let Some(regex) = PATTERN_CACHE.get(pattern_str)
    {
        return regex.is_match(&input_lower) || is_valid_pattern_prefix(base, regex, pattern_str);
    }

    dv.syntax.to_lowercase().starts_with(base)
}

fn is_valid_pattern_prefix(input: &str, regex: &Regex, pattern_str: &str) -> bool {
    if pattern_str.starts_with("^d") {
        return is_valid_d_prefix(input);
    }

    if pattern_str.contains("every-") && pattern_str.contains("[1-9]") {
        return is_valid_every_number_prefix(input);
    }

    regex.is_match(&format!("{input}1"))
        || regex.is_match(&format!("{input}a"))
        || regex.is_match(input)
}

fn is_valid_d_prefix(input: &str) -> bool {
    if input == "d" {
        return true;
    }
    input
        .strip_prefix('d')
        .is_some_and(|rest| {
            !rest.is_empty()
                && rest.len() <= 3
                && rest.chars().all(|c| c.is_ascii_digit())
                && !rest.starts_with('0')
        })
}

fn is_valid_every_number_prefix(input: &str) -> bool {
    if !"every-".starts_with(input) && !input.starts_with("every-") {
        return false;
    }

    input.strip_prefix("every-").is_none_or(|rest| {
        rest.is_empty() || rest.parse::<u32>().is_ok_and(|n| (1..=31).contains(&n))
    })
}

#[allow(dead_code)]
pub(super) fn compute_date_completion(input: &str, dv: &DateValue) -> Option<String> {
    let input_lower = input.to_lowercase();
    let (base, suffix) = strip_direction_suffix(&input_lower);

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
