use chrono::{Days, NaiveDate};

/// Context for date parsing, determining default behavior for relative dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseContext {
    /// Entry context: future bias by default, `+/-` allowed for explicit direction
    Entry,
    /// Filter context: past bias by default, `+/-` allowed for explicit direction
    Filter,
    /// Interface context: future bias (same as Entry)
    Interface,
}

/// Parses a weekday name (full or abbreviated) into chrono::Weekday.
#[must_use]
pub fn parse_weekday(s: &str) -> Option<chrono::Weekday> {
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

/// Parses relative date expressions (today, tomorrow, yesterday, d7, mon, etc.).
///
/// Context determines default direction and suffix handling:
/// - Entry: always future; `+` is ignored; `-` returns None (rejected)
/// - Filter/Interface: default past; `+` for explicit future; `-` for explicit past
#[must_use]
pub fn parse_relative_date(input: &str, today: NaiveDate, ctx: ParseContext) -> Option<NaiveDate> {
    let input_lower = input.to_lowercase();

    // Fixed dates (not affected by context)
    if input_lower == "today" {
        return Some(today);
    }
    if input_lower == "tomorrow" {
        return today.checked_add_days(Days::new(1));
    }
    if input_lower == "yesterday" {
        return today.checked_sub_days(Days::new(1));
    }

    // Check for explicit direction suffixes
    let (base, explicit_future, explicit_past) = if let Some(b) = input_lower.strip_suffix('+') {
        (b, true, false)
    } else if let Some(b) = input_lower.strip_suffix('-') {
        (b, false, true)
    } else {
        (input_lower.as_str(), false, false)
    };

    // Determine final direction based on context and explicit markers
    let is_future = match ctx {
        ParseContext::Entry | ParseContext::Interface => {
            // Entry/Interface: default future, but respect explicit `-` for past
            !explicit_past
        }
        ParseContext::Filter => {
            // Filter: default past, only future if explicitly requested with '+'
            explicit_future
        }
    };

    // Parse d[1-999] format
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

    // Parse weekday names
    if let Some(target_weekday) = parse_weekday(base) {
        return if is_future {
            next_weekday_from(today, target_weekday)
        } else {
            prev_weekday_from(today, target_weekday)
        };
    }

    None
}

/// For MM/DD without year: if date has passed this year, use next year.
#[must_use]
fn resolve_month_day_future(month: u32, day: u32, today: NaiveDate) -> Option<NaiveDate> {
    use chrono::Datelike;
    let date = NaiveDate::from_ymd_opt(today.year(), month, day)?;
    if date < today {
        NaiveDate::from_ymd_opt(today.year() + 1, month, day)
    } else {
        Some(date)
    }
}

/// For MM/DD without year: if date is in future this year, use last year.
#[must_use]
fn resolve_month_day_past(month: u32, day: u32, today: NaiveDate) -> Option<NaiveDate> {
    use chrono::Datelike;
    let date = NaiveDate::from_ymd_opt(today.year(), month, day)?;
    if date > today {
        NaiveDate::from_ymd_opt(today.year() - 1, month, day)
    } else {
        Some(date)
    }
}

/// Parses absolute date formats:
/// - MM/DD (context determines year bias)
/// - MM/DD/YY (assumed 20xx)
/// - MM/DD/YYYY
/// - YYYY/MM/DD (ISO format)
#[must_use]
pub fn parse_absolute_date(
    date_str: &str,
    today: NaiveDate,
    ctx: ParseContext,
) -> Option<NaiveDate> {
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
            let full_year = if year < 100 { 2000 + year } else { year };
            return NaiveDate::from_ymd_opt(full_year, month, day);
        }
    }

    // MM/DD (no year) - use context-dependent logic
    let parts: Vec<&str> = date_str.split('/').collect();
    if parts.len() == 2 {
        let month: u32 = parts[0].parse().ok()?;
        let day: u32 = parts[1].parse().ok()?;

        return match ctx {
            ParseContext::Entry | ParseContext::Interface => {
                resolve_month_day_future(month, day, today)
            }
            ParseContext::Filter => resolve_month_day_past(month, day, today),
        };
    }

    None
}

/// Parses a date string, trying relative formats first, then absolute formats.
#[must_use]
pub fn parse_date(input: &str, ctx: ParseContext, today: NaiveDate) -> Option<NaiveDate> {
    parse_relative_date(input, today, ctx).or_else(|| parse_absolute_date(input, today, ctx))
}
