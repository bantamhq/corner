use std::fs;
use std::io;
use std::path::Path;

use chrono::NaiveDate;

use super::entries::{Entry, EntryType, Line, parse_lines, serialize_lines};

pub fn load_day_lines(date: NaiveDate, path: &Path) -> io::Result<Vec<Line>> {
    let content = load_day(date, path)?;
    Ok(parse_lines(&content))
}

pub fn save_day_lines(date: NaiveDate, path: &Path, lines: &[Line]) -> io::Result<()> {
    let content = serialize_lines(lines);
    save_day(date, path, &content)
}

/// Helper to load, mutate an entry, and save in one operation.
/// Returns the result of the mutation function if the entry exists.
pub fn mutate_entry<F, R>(
    date: NaiveDate,
    path: &Path,
    line_index: usize,
    f: F,
) -> io::Result<Option<R>>
where
    F: FnOnce(&mut Entry) -> R,
{
    let mut lines = load_day_lines(date, path)?;
    let result = lines.get_mut(line_index).and_then(|line| match line {
        Line::Entry(entry) => Some(f(entry)),
        _ => None,
    });
    if result.is_some() {
        save_day_lines(date, path, &lines)?;
    }
    Ok(result)
}

/// Updates an entry's content at a specific line index for a given date.
/// Returns Ok(true) if update succeeded, Ok(false) if no entry at that index.
pub fn update_entry_content(
    date: NaiveDate,
    path: &Path,
    line_index: usize,
    content: String,
) -> io::Result<bool> {
    mutate_entry(date, path, line_index, |entry| {
        entry.content = content;
    })
    .map(|opt| opt.is_some())
}

/// Toggles the completion status of a task at a specific line index.
pub fn toggle_entry_complete(date: NaiveDate, path: &Path, line_index: usize) -> io::Result<()> {
    mutate_entry(date, path, line_index, |entry| {
        entry.toggle_complete();
    })?;
    Ok(())
}

/// Cycles the entry type (Task -> Note -> Event -> Task) at a specific line index.
/// Returns the new entry type if successful.
pub fn cycle_entry_type(
    date: NaiveDate,
    path: &Path,
    line_index: usize,
) -> io::Result<Option<EntryType>> {
    mutate_entry(date, path, line_index, |entry| {
        entry.entry_type = entry.entry_type.cycle();
        entry.entry_type.clone()
    })
}

/// Deletes an entry at a specific line index for a given date.
pub fn delete_entry(date: NaiveDate, path: &Path, line_index: usize) -> io::Result<()> {
    let mut lines = load_day_lines(date, path)?;
    if line_index < lines.len() {
        lines.remove(line_index);
    }
    save_day_lines(date, path, &lines)
}

fn day_header(date: NaiveDate) -> String {
    format!("# {}", date.format("%Y/%m/%d"))
}

pub fn load_journal(path: &Path) -> io::Result<String> {
    if path.exists() {
        fs::read_to_string(path)
    } else {
        Ok(String::new())
    }
}

pub fn save_journal(path: &Path, content: &str) -> io::Result<()> {
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

pub fn parse_day_header(line: &str) -> Option<NaiveDate> {
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

pub fn load_day(date: NaiveDate, path: &Path) -> io::Result<String> {
    let journal = load_journal(path)?;
    Ok(extract_day_content(&journal, date))
}

pub fn save_day(date: NaiveDate, path: &Path, content: &str) -> io::Result<()> {
    let journal = load_journal(path)?;
    let updated = update_day_content(&journal, date, content);
    save_journal(path, &updated)
}
