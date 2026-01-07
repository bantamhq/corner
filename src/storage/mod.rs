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
    Entry, EntryType, Line, RawEntry, RecurringPattern, SourceType, parse_lines, parse_to_raw_entry,
    serialize_lines,
};

// Re-export persistence functions and types
pub use persistence::{
    DayInfo, cycle_entry_type, delete_entry, extract_day_content, get_entry_content,
    get_entry_type, load_day, load_day_lines, load_journal, mutate_entry, parse_day_header,
    save_day, save_day_lines, save_journal, scan_days_in_range, toggle_entry_complete,
    update_day_content, update_entry_content,
};

// Re-export filter types and functions
pub use filter::{
    FAVORITE_TAG_REGEX, Filter, FilterType, LATER_DATE_REGEX, NATURAL_DATE_REGEX,
    RECURRING_REGEX, SAVED_FILTER_REGEX, TAG_REGEX, collect_filtered_entries, collect_journal_tags,
    collect_projected_entries_for_date, expand_favorite_tags, expand_saved_filters,
    extract_recurring_pattern, extract_tags, extract_target_date, normalize_natural_dates,
    parse_filter_date, parse_filter_query, parse_later_date, parse_natural_date,
    parse_recurring_pattern, strip_recurring_tags,
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn round_trip_preserves_line_content() {
        let original = "- [ ] Task one\n- [x] Task done\n- A note\n* An event\nRaw line";
        let lines = parse_lines(original);
        let serialized = serialize_lines(&lines);
        assert_eq!(serialized, original);
    }

    #[test]
    fn round_trip_preserves_blank_lines() {
        let original = "- [ ] Task\n\n- Note after blank";
        let lines = parse_lines(original);
        let serialized = serialize_lines(&lines);
        assert_eq!(serialized, original);
    }

    #[test]
    fn extract_day_content_separates_days() {
        let journal = "# 2024/01/15\n- Task 1\n\n# 2024/01/16\n- Task 2\n";

        let date1 = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        assert_eq!(extract_day_content(journal, date1), "- Task 1");

        let date2 = NaiveDate::from_ymd_opt(2024, 1, 16).unwrap();
        assert_eq!(extract_day_content(journal, date2), "- Task 2");
    }

    #[test]
    fn update_day_content_preserves_other_days() {
        let journal =
            "# 2024/01/14\n- Day 14\n\n# 2024/01/15\n- Old task\n\n# 2024/01/16\n- Day 16\n";
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let updated = update_day_content(journal, date, "- Updated task");

        assert!(updated.contains("# 2024/01/14\n- Day 14"));
        assert!(updated.contains("# 2024/01/15\n- Updated task"));
        assert!(updated.contains("# 2024/01/16\n- Day 16"));
    }

    #[test]
    fn parse_natural_date_handles_all_formats() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();

        assert_eq!(
            parse_natural_date("today", today),
            NaiveDate::from_ymd_opt(2026, 1, 5)
        );
        assert_eq!(
            parse_natural_date("tomorrow", today),
            NaiveDate::from_ymd_opt(2026, 1, 6)
        );
        assert_eq!(
            parse_natural_date("yesterday", today),
            NaiveDate::from_ymd_opt(2026, 1, 4)
        );

        assert_eq!(
            parse_natural_date("d3", today),
            NaiveDate::from_ymd_opt(2026, 1, 8)
        );

        assert_eq!(
            parse_natural_date("mon", today),
            NaiveDate::from_ymd_opt(2026, 1, 12)
        );
        assert_eq!(
            parse_natural_date("fri", today),
            NaiveDate::from_ymd_opt(2026, 1, 9)
        );

        assert_eq!(
            parse_natural_date("1/15", today),
            NaiveDate::from_ymd_opt(2026, 1, 15)
        );
    }

    #[test]
    fn parse_filter_date_handles_relative_dates() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();

        assert_eq!(
            parse_filter_date("d3", today),
            NaiveDate::from_ymd_opt(2026, 1, 2)
        );
        assert_eq!(
            parse_filter_date("mon", today),
            NaiveDate::from_ymd_opt(2025, 12, 29)
        );

        assert_eq!(
            parse_filter_date("d3+", today),
            NaiveDate::from_ymd_opt(2026, 1, 8)
        );
        assert_eq!(
            parse_filter_date("mon+", today),
            NaiveDate::from_ymd_opt(2026, 1, 12)
        );

        assert_eq!(
            parse_filter_date("today", today),
            NaiveDate::from_ymd_opt(2026, 1, 5)
        );
    }

    #[test]
    fn normalize_converts_natural_to_mm_dd() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();

        // @today includes year to avoid "always future" misinterpretation
        assert_eq!(
            normalize_natural_dates("Do it @today", today),
            "Do it @01/05/26"
        );
        // Other natural dates use MM/DD format
        assert_eq!(
            normalize_natural_dates("Call dentist @tomorrow", today),
            "Call dentist @01/06"
        );
        // New d# syntax (entry context = future)
        assert_eq!(
            normalize_natural_dates("Review @d3", today),
            "Review @01/08"
        );
        // New weekday syntax (entry context = next occurrence)
        assert_eq!(
            normalize_natural_dates("Meeting @mon", today),
            "Meeting @01/12"
        );
    }

    #[test]
    fn filter_query_combines_types_tags_dates() {
        let filter = parse_filter_query("!tasks #work @after:1/1 @before:1/31");
        assert_eq!(filter.entry_types, vec![FilterType::Task]);
        assert_eq!(filter.tags, vec!["work"]);
        assert!(filter.after_date.is_some());
        assert!(filter.before_date.is_some());
        assert!(filter.invalid_tokens.is_empty());

        // Multiple entry types use OR logic
        let filter = parse_filter_query("!tasks !notes");
        assert_eq!(filter.entry_types, vec![FilterType::Task, FilterType::Note]);
        assert!(filter.invalid_tokens.is_empty());
    }

    #[test]
    fn filter_date_today_parses_to_current_date() {
        // @before:today and @after:today should parse to today's date
        let today = chrono::Local::now().date_naive();

        let filter = parse_filter_query("@before:today");
        assert_eq!(filter.before_date, Some(today));
        assert!(filter.invalid_tokens.is_empty());

        let filter = parse_filter_query("@after:today");
        assert_eq!(filter.after_date, Some(today));
        assert!(filter.invalid_tokens.is_empty());
    }

    #[test]
    fn invalid_tokens_captured_in_filter() {
        assert!(!parse_filter_query("!tas").invalid_tokens.is_empty());
        assert!(
            !parse_filter_query("@before:1/1 @before:1/15")
                .invalid_tokens
                .is_empty()
        );
    }
}
