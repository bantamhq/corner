use chrono::NaiveDate;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::OnceLock;

static JOURNAL_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn set_journal_path(path: PathBuf) {
    let _ = JOURNAL_PATH.set(path);
}

pub fn get_active_journal_path() -> PathBuf {
    JOURNAL_PATH.get().cloned().unwrap_or_else(get_journal_path)
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryType {
    Task { completed: bool },
    Note,
    Event,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub entry_type: EntryType,
    pub content: String,
}

impl Entry {
    pub fn new_task(content: &str) -> Self {
        Self {
            entry_type: EntryType::Task { completed: false },
            content: content.to_string(),
        }
    }

    pub fn prefix(&self) -> &'static str {
        match &self.entry_type {
            EntryType::Task { completed: false } => "- [ ] ",
            EntryType::Task { completed: true } => "- [x] ",
            EntryType::Note => "- ",
            EntryType::Event => "* ",
        }
    }

    pub fn toggle_complete(&mut self) {
        if let EntryType::Task { completed } = &mut self.entry_type {
            *completed = !*completed;
        }
    }
}

// Line::Raw preserves non-entry content (blank lines, headers, arbitrary text)
// so the journal file can be manually edited without data loss on save.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Entry(Entry),
    Raw(String),
}

fn parse_line(line: &str) -> Line {
    let trimmed = line.trim_start();

    // Order matters: task patterns must be checked before note pattern
    // since "- " is a prefix of "- [ ] " and "- [x] ".
    if let Some(content) = trimmed.strip_prefix("- [ ] ") {
        return Line::Entry(Entry {
            entry_type: EntryType::Task { completed: false },
            content: content.to_string(),
        });
    }

    if let Some(content) = trimmed.strip_prefix("- [x] ") {
        return Line::Entry(Entry {
            entry_type: EntryType::Task { completed: true },
            content: content.to_string(),
        });
    }

    if let Some(content) = trimmed.strip_prefix("* ") {
        return Line::Entry(Entry {
            entry_type: EntryType::Event,
            content: content.to_string(),
        });
    }

    if let Some(content) = trimmed.strip_prefix("- ") {
        return Line::Entry(Entry {
            entry_type: EntryType::Note,
            content: content.to_string(),
        });
    }

    Line::Raw(line.to_string())
}

pub fn parse_lines(content: &str) -> Vec<Line> {
    content.lines().map(parse_line).collect()
}

fn serialize_line(line: &Line) -> String {
    match line {
        Line::Entry(entry) => format!("{}{}", entry.prefix(), entry.content),
        Line::Raw(s) => s.clone(),
    }
}

pub fn serialize_lines(lines: &[Line]) -> String {
    lines
        .iter()
        .map(serialize_line)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn load_day_lines(date: NaiveDate) -> io::Result<Vec<Line>> {
    let content = load_day(date)?;
    Ok(parse_lines(&content))
}

pub fn save_day_lines(date: NaiveDate, lines: &[Line]) -> io::Result<()> {
    let content = serialize_lines(lines);
    save_day(date, &content)
}

pub fn get_journal_path() -> PathBuf {
    crate::config::get_default_journal_path()
}

fn day_header(date: NaiveDate) -> String {
    format!("# {}", date.format("%Y/%m/%d"))
}

pub fn load_journal() -> io::Result<String> {
    let path = get_active_journal_path();
    if path.exists() {
        fs::read_to_string(path)
    } else {
        Ok(String::new())
    }
}

pub fn save_journal(content: &str) -> io::Result<()> {
    let path = get_active_journal_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, content)
}

pub fn extract_day_content(journal: &str, date: NaiveDate) -> String {
    let header = day_header(date);

    let Some(start_idx) = journal.find(&header) else {
        return String::new();
    };

    let content_start = start_idx + header.len();
    let after_header = &journal[content_start..];
    let after_header = after_header.strip_prefix('\n').unwrap_or(after_header);
    let end_idx = find_next_day_header(after_header);

    match end_idx {
        Some(idx) => after_header[..idx].trim_end().to_string(),
        None => after_header.trim_end().to_string(),
    }
}

fn parse_day_header(line: &str) -> Option<NaiveDate> {
    if !line.starts_with("# ") {
        return None;
    }
    let rest = &line[2..];
    if rest.len() < 10 {
        return None;
    }
    NaiveDate::parse_from_str(&rest[..10], "%Y/%m/%d").ok()
}

fn is_day_header(line: &str) -> bool {
    parse_day_header(line).is_some()
}

fn find_next_day_header(content: &str) -> Option<usize> {
    let mut byte_pos = 0;
    let mut is_first_line = true;

    for line in content.lines() {
        // Skip first line - this function is called on content after a header,
        // so line 0 is day content, not a potential next header.
        if !is_first_line && is_day_header(line) {
            return Some(byte_pos);
        }
        is_first_line = false;

        byte_pos += line.len();
        if byte_pos < content.len() {
            let next_char = content[byte_pos..].chars().next();
            if next_char == Some('\r') {
                byte_pos += 1;
            }
            if byte_pos < content.len() && content[byte_pos..].starts_with('\n') {
                byte_pos += 1;
            }
        }
    }
    None
}

pub fn update_day_content(journal: &str, date: NaiveDate, new_content: &str) -> String {
    let header = day_header(date);
    let content_is_empty = new_content.trim().is_empty();

    if let Some(start_idx) = journal.find(&header) {
        let (before, after) = split_around_day(journal, start_idx, &header);
        if content_is_empty {
            remove_day(before, after)
        } else {
            replace_day(before, &header, new_content, after)
        }
    } else if content_is_empty {
        journal.to_string()
    } else {
        insert_new_day(journal, date, &header, new_content)
    }
}

fn split_around_day<'a>(journal: &'a str, start_idx: usize, header: &str) -> (&'a str, &'a str) {
    let before = &journal[..start_idx];
    let content_start = start_idx + header.len();
    let after_header = &journal[content_start..];
    let after_header = after_header.strip_prefix('\n').unwrap_or(after_header);

    let after = match find_next_day_header(after_header) {
        Some(idx) => &after_header[idx..],
        None => "",
    };

    (before, after)
}

fn remove_day(before: &str, after: &str) -> String {
    let mut result = before.trim_end().to_string();
    if !result.is_empty() && !after.is_empty() {
        result.push_str("\n\n");
    }
    result.push_str(after.trim_start());
    if result.is_empty() {
        result
    } else {
        result.trim_end().to_string() + "\n"
    }
}

fn replace_day(before: &str, header: &str, content: &str, after: &str) -> String {
    format!("{}{}\n{}\n\n{}", before, header, content.trim_end(), after)
        .trim_end()
        .to_string()
        + "\n"
}

fn insert_new_day(journal: &str, date: NaiveDate, header: &str, content: &str) -> String {
    let new_day = format!("{}\n{}\n", header, content.trim_end());

    let insert_pos = find_insertion_point(journal, date);

    if let Some(pos) = insert_pos {
        let before = journal[..pos].trim_end();
        let after = &journal[pos..];
        if before.is_empty() {
            format!("{}\n{}", new_day.trim_end(), after.trim_start())
                .trim_end()
                .to_string()
                + "\n"
        } else {
            format!(
                "{}\n\n{}\n{}",
                before,
                new_day.trim_end(),
                after.trim_start()
            )
            .trim_end()
            .to_string()
                + "\n"
        }
    } else {
        let mut result = journal.trim_end().to_string();
        if !result.is_empty() {
            result.push_str("\n\n");
        }
        result.push_str(new_day.trim_end());
        result.push('\n');
        result
    }
}

fn find_insertion_point(journal: &str, date: NaiveDate) -> Option<usize> {
    for line in journal.lines() {
        if let Some(existing_date) = parse_day_header(line)
            && existing_date > date
        {
            return journal.find(line);
        }
    }
    None
}

pub fn load_day(date: NaiveDate) -> io::Result<String> {
    let journal = load_journal()?;
    Ok(extract_day_content(&journal, date))
}

pub fn save_day(date: NaiveDate, content: &str) -> io::Result<()> {
    let journal = load_journal()?;
    let updated = update_day_content(&journal, date, content);
    save_journal(&updated)
}

#[derive(Debug, Clone)]
pub struct TaskItem {
    pub date: NaiveDate,
    pub content: String,
    // Index within the day's parsed lines - used for reliable task matching
    // when toggling from tasks view (avoids ambiguity with duplicate content).
    pub line_index: usize,
    pub completed: bool,
}

pub fn collect_incomplete_tasks() -> io::Result<Vec<TaskItem>> {
    let journal = load_journal()?;
    let mut tasks = Vec::new();
    let mut current_date: Option<NaiveDate> = None;
    let mut line_index_in_day: usize = 0;

    for line in journal.lines() {
        if let Some(date) = parse_day_header(line) {
            current_date = Some(date);
            line_index_in_day = 0;
            continue;
        }

        if let Some(date) = current_date {
            if let Some(content) = line.trim_start().strip_prefix("- [ ] ") {
                tasks.push(TaskItem {
                    date,
                    content: content.to_string(),
                    line_index: line_index_in_day,
                    completed: false,
                });
            }
            line_index_in_day += 1;
        }
    }

    tasks.sort_by_key(|t| t.date);
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_parse_task_incomplete() {
        let line = parse_line("- [ ] Buy groceries");
        assert_eq!(
            line,
            Line::Entry(Entry {
                entry_type: EntryType::Task { completed: false },
                content: "Buy groceries".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_task_complete() {
        let line = parse_line("- [x] Finished task");
        assert_eq!(
            line,
            Line::Entry(Entry {
                entry_type: EntryType::Task { completed: true },
                content: "Finished task".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_note() {
        let line = parse_line("- Just a note");
        assert_eq!(
            line,
            Line::Entry(Entry {
                entry_type: EntryType::Note,
                content: "Just a note".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_event() {
        let line = parse_line("* Meeting at 3pm");
        assert_eq!(
            line,
            Line::Entry(Entry {
                entry_type: EntryType::Event,
                content: "Meeting at 3pm".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_raw_line() {
        let line = parse_line("Some random text");
        assert_eq!(line, Line::Raw("Some random text".to_string()));
    }

    #[test]
    fn test_parse_empty_line() {
        let line = parse_line("");
        assert_eq!(line, Line::Raw(String::new()));
    }

    #[test]
    fn test_round_trip_parsing() {
        let original = "- [ ] Task one\n- [x] Task done\n- A note\n* An event\nRaw line";
        let lines = parse_lines(original);
        let serialized = serialize_lines(&lines);
        assert_eq!(serialized, original);
    }

    #[test]
    fn test_round_trip_with_blank_lines() {
        let original = "- [ ] Task\n\n- Note after blank";
        let lines = parse_lines(original);
        let serialized = serialize_lines(&lines);
        assert_eq!(serialized, original);
    }

    #[test]
    fn test_entry_toggle() {
        let mut entry = Entry::new_task("Test");
        assert!(matches!(
            entry.entry_type,
            EntryType::Task { completed: false }
        ));

        entry.toggle_complete();
        assert!(matches!(
            entry.entry_type,
            EntryType::Task { completed: true }
        ));

        entry.toggle_complete();
        assert!(matches!(
            entry.entry_type,
            EntryType::Task { completed: false }
        ));
    }

    #[test]
    fn test_is_day_header() {
        assert!(is_day_header("# 2024/01/15"));
        assert!(is_day_header("# 2024/12/31"));
        assert!(!is_day_header("## 2024/01/15"));
        assert!(!is_day_header("# 2024-01-15"));
        assert!(!is_day_header("# not a date"));
        assert!(!is_day_header("2024/01/15"));
    }

    #[test]
    fn test_extract_day_content_single_day() {
        let journal = "# 2024/01/15\n- Task 1\n- Task 2\n";
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let content = extract_day_content(journal, date);
        assert_eq!(content, "- Task 1\n- Task 2");
    }

    #[test]
    fn test_extract_day_content_multiple_days() {
        let journal = "# 2024/01/15\n- Task 1\n\n# 2024/01/16\n- Task 2\n";

        let date1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let content1 = extract_day_content(journal, date1);
        assert_eq!(content1, "- Task 1");

        let date2 = NaiveDate::from_ymd_opt(2024, 1, 16).unwrap();
        let content2 = extract_day_content(journal, date2);
        assert_eq!(content2, "- Task 2");
    }

    #[test]
    fn test_extract_day_content_not_found() {
        let journal = "# 2024/01/15\n- Task 1\n";
        let date = NaiveDate::from_ymd_opt(2024, 1, 16).unwrap();
        let content = extract_day_content(journal, date);
        assert_eq!(content, "");
    }

    #[test]
    fn test_update_day_content_new_day() {
        let journal = "";
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let updated = update_day_content(journal, date, "- New task");
        assert_eq!(updated, "# 2024/01/15\n- New task\n");
    }

    #[test]
    fn test_update_day_content_existing_day() {
        let journal = "# 2024/01/15\n- Old task\n";
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let updated = update_day_content(journal, date, "- New task");
        assert_eq!(updated, "# 2024/01/15\n- New task\n");
    }

    #[test]
    fn test_update_day_content_preserves_other_days() {
        let journal =
            "# 2024/01/14\n- Day 14\n\n# 2024/01/15\n- Old task\n\n# 2024/01/16\n- Day 16\n";
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let updated = update_day_content(journal, date, "- Updated task");

        assert!(updated.contains("# 2024/01/14\n- Day 14"));
        assert!(updated.contains("# 2024/01/15\n- Updated task"));
        assert!(updated.contains("# 2024/01/16\n- Day 16"));
    }
}
