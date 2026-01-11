use chrono::{Datelike, NaiveDate, Weekday};

#[derive(Debug, Clone, PartialEq)]
pub enum EntryType {
    Task { completed: bool },
    Note,
    Event,
}

impl EntryType {
    #[must_use]
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Task { completed: false } => "- [ ] ",
            Self::Task { completed: true } => "- [x] ",
            Self::Note => "- ",
            Self::Event => "* ",
        }
    }

    #[must_use]
    pub fn cycle(&self) -> Self {
        match self {
            Self::Task { .. } => Self::Note,
            Self::Note => Self::Event,
            Self::Event => Self::Task { completed: false },
        }
    }
}

/// Where an entry originates from relative to the viewed day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceType {
    /// Entry belongs to the viewed day, editable
    Local,
    /// Projected via @date pattern, read-only
    Later,
    /// Projected via @every-* pattern, read-only
    Recurring,
    /// From external calendar (ICS), read-only
    Calendar {
        calendar_id: String,
        calendar_name: String,
    },
}

/// Raw entry as parsed from markdown, without location metadata.
/// Used internally for parsing and serialization.
#[derive(Debug, Clone, PartialEq)]
pub struct RawEntry {
    pub entry_type: EntryType,
    pub content: String,
}

impl RawEntry {
    #[must_use]
    pub fn new_task(content: &str) -> Self {
        Self {
            entry_type: EntryType::Task { completed: false },
            content: content.to_string(),
        }
    }

    pub fn prefix(&self) -> &'static str {
        self.entry_type.prefix()
    }

    pub fn toggle_complete(&mut self) {
        if let EntryType::Task { completed } = &mut self.entry_type {
            *completed = !*completed;
        }
    }
}

/// Entry with full location and source metadata.
/// This is the unified entry type used throughout the application.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub entry_type: EntryType,
    pub content: String,
    pub source_date: NaiveDate,
    pub line_index: usize,
    pub source_type: SourceType,
}

impl Entry {
    /// Create an Entry from a RawEntry with location metadata.
    #[must_use]
    pub fn from_raw(
        raw: &RawEntry,
        source_date: NaiveDate,
        line_index: usize,
        source_type: SourceType,
    ) -> Self {
        Self {
            entry_type: raw.entry_type.clone(),
            content: raw.content.clone(),
            source_date,
            line_index,
            source_type,
        }
    }

    #[must_use]
    pub fn new_task(content: &str, source_date: NaiveDate, line_index: usize) -> Self {
        Self {
            entry_type: EntryType::Task { completed: false },
            content: content.to_string(),
            source_date,
            line_index,
            source_type: SourceType::Local,
        }
    }

    pub fn prefix(&self) -> &'static str {
        self.entry_type.prefix()
    }

    pub fn toggle_complete(&mut self) {
        if let EntryType::Task { completed } = &mut self.entry_type {
            *completed = !*completed;
        }
    }

    /// Convert back to RawEntry for serialization.
    #[must_use]
    pub fn to_raw(&self) -> RawEntry {
        RawEntry {
            entry_type: self.entry_type.clone(),
            content: self.content.clone(),
        }
    }

    /// Returns true if this entry can be edited/deleted.
    #[must_use]
    pub fn is_editable(&self) -> bool {
        self.source_type == SourceType::Local
    }
}

/// A line in the journal file - either a parsed entry or raw markdown.
/// Uses RawEntry for parsing/serialization; Entry is derived with context.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Entry(RawEntry),
    Raw(String),
}

/// Recurring pattern for @every-* syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurringPattern {
    /// @every-day - every day
    Daily,
    /// @every-weekday - Monday through Friday
    Weekday,
    /// @every-monday through @every-sunday
    Weekly(Weekday),
    /// @every-1 through @every-31 (day of month)
    Monthly(u8),
}

impl RecurringPattern {
    /// Returns true if this pattern matches the given date.
    #[must_use]
    pub fn matches(&self, date: NaiveDate) -> bool {
        match self {
            Self::Daily => true,
            Self::Weekday => !matches!(date.weekday(), Weekday::Sat | Weekday::Sun),
            Self::Weekly(day) => date.weekday() == *day,
            Self::Monthly(day) => {
                let last_day = last_day_of_month(date);
                if u32::from(*day) > last_day {
                    date.day() == last_day
                } else {
                    date.day() == u32::from(*day)
                }
            }
        }
    }
}

/// Returns the last day of the month for the given date.
#[must_use]
fn last_day_of_month(date: NaiveDate) -> u32 {
    let (year, month) = (date.year(), date.month());
    let next_month = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next_month
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

/// Parses a line into a RawEntry, treating unparsed lines as notes.
#[must_use]
pub fn parse_to_raw_entry(line: &str) -> RawEntry {
    let trimmed = line.trim_start();

    if let Some(content) = trimmed.strip_prefix("- [ ] ") {
        return RawEntry {
            entry_type: EntryType::Task { completed: false },
            content: content.to_string(),
        };
    }
    if let Some(content) = trimmed.strip_prefix("- [x] ") {
        return RawEntry {
            entry_type: EntryType::Task { completed: true },
            content: content.to_string(),
        };
    }
    if let Some(content) = trimmed.strip_prefix("* ") {
        return RawEntry {
            entry_type: EntryType::Event,
            content: content.to_string(),
        };
    }
    if let Some(content) = trimmed.strip_prefix("- ") {
        return RawEntry {
            entry_type: EntryType::Note,
            content: content.to_string(),
        };
    }
    RawEntry {
        entry_type: EntryType::Note,
        content: trimmed.to_string(),
    }
}

fn parse_line(line: &str) -> Line {
    let trimmed = line.trim_start();
    if trimmed.starts_with("- [ ] ")
        || trimmed.starts_with("- [x] ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("- ")
    {
        Line::Entry(parse_to_raw_entry(line))
    } else {
        Line::Raw(line.to_string())
    }
}

#[must_use]
pub fn parse_lines(content: &str) -> Vec<Line> {
    content.lines().map(parse_line).collect()
}

fn serialize_line(line: &Line) -> String {
    match line {
        Line::Entry(raw_entry) => format!("{}{}", raw_entry.prefix(), raw_entry.content),
        Line::Raw(s) => s.clone(),
    }
}

#[must_use]
pub fn serialize_lines(lines: &[Line]) -> String {
    lines
        .iter()
        .map(serialize_line)
        .collect::<Vec<_>>()
        .join("\n")
}
