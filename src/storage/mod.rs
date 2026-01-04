mod context;
mod entries;
mod filter;
mod persistence;

// Re-export context types and functions
pub use context::{
    JournalContext, JournalSlot, add_caliber_to_gitignore, create_project_journal,
    detect_project_journal, find_git_root,
};

// Re-export entry types
pub use entries::{
    CrossDayEntry, Entry, EntryType, FilterEntry, LaterEntry, Line, parse_lines, serialize_lines,
};

// Re-export persistence functions
pub use persistence::{
    cycle_entry_type, delete_entry, extract_day_content, load_day, load_day_lines, load_journal,
    mutate_entry, parse_day_header, save_day, save_day_lines, save_journal, toggle_entry_complete,
    update_day_content, update_entry_content,
};

// Re-export filter types and functions
pub use filter::{
    FAVORITE_TAG_REGEX, Filter, FilterType, LATER_DATE_REGEX, NATURAL_DATE_REGEX,
    SAVED_FILTER_REGEX, TAG_REGEX, collect_filtered_entries, collect_journal_tags,
    collect_later_entries_for_date, expand_favorite_tags, expand_saved_filters, extract_tags,
    extract_target_date, normalize_natural_dates, parse_filter_query, parse_later_date,
    parse_natural_date,
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

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
    fn test_extract_day_content_multiple_days() {
        let journal = "# 2024/01/15\n- Task 1\n\n# 2024/01/16\n- Task 2\n";

        let date1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        assert_eq!(extract_day_content(journal, date1), "- Task 1");

        let date2 = NaiveDate::from_ymd_opt(2024, 1, 16).unwrap();
        assert_eq!(extract_day_content(journal, date2), "- Task 2");
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

    #[test]
    fn test_parse_natural_date_all_formats() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(); // Monday

        // tomorrow/yesterday
        assert_eq!(
            parse_natural_date("tomorrow", today),
            NaiveDate::from_ymd_opt(2026, 1, 6)
        );
        assert_eq!(
            parse_natural_date("yesterday", today),
            NaiveDate::from_ymd_opt(2026, 1, 4)
        );

        // relative days
        assert_eq!(
            parse_natural_date("3d", today),
            NaiveDate::from_ymd_opt(2026, 1, 8)
        );
        assert_eq!(
            parse_natural_date("-3d", today),
            NaiveDate::from_ymd_opt(2026, 1, 2)
        );

        // weekdays
        assert_eq!(
            parse_natural_date("next-monday", today),
            NaiveDate::from_ymd_opt(2026, 1, 12)
        );
        assert_eq!(
            parse_natural_date("last-friday", today),
            NaiveDate::from_ymd_opt(2026, 1, 2)
        );

        // fallback to standard format
        assert_eq!(
            parse_natural_date("1/15", today),
            NaiveDate::from_ymd_opt(2026, 1, 15)
        );
    }

    #[test]
    fn test_normalize_natural_dates() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();

        assert_eq!(
            normalize_natural_dates("Call dentist @tomorrow", today),
            "Call dentist @01/06"
        );
        assert_eq!(
            normalize_natural_dates("Review @3d and @-3d", today),
            "Review @01/08 and @01/02"
        );
        assert_eq!(
            normalize_natural_dates("Meeting @next-monday", today),
            "Meeting @01/12"
        );
    }

    #[test]
    fn test_filter_combined() {
        let filter = parse_filter_query("!tasks #work @after:1/1 @before:1/31");
        assert_eq!(filter.entry_type, Some(FilterType::Task));
        assert_eq!(filter.tags, vec!["work"]);
        assert!(filter.after_date.is_some());
        assert!(filter.before_date.is_some());
        assert!(filter.invalid_tokens.is_empty());
    }

    #[test]
    fn test_filter_invalid_tokens() {
        assert!(!parse_filter_query("!tas").invalid_tokens.is_empty());
        assert!(
            !parse_filter_query("!tasks !notes")
                .invalid_tokens
                .is_empty()
        );
        assert!(
            !parse_filter_query("@before:1/1 @before:1/15")
                .invalid_tokens
                .is_empty()
        );
    }
}
