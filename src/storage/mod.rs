mod context;
mod date_parsing;
mod entries;
mod filter;
mod persistence;
mod project_registry;

// Re-export context types and functions
pub use context::{JournalContext, JournalSlot, detect_project_journal, find_git_root};

// Re-export entry types
pub use entries::{
    Entry, EntryType, Line, RawEntry, RecurringPattern, SourceType, parse_lines,
    parse_to_raw_entry, serialize_lines,
};

// Re-export persistence functions and types
pub use persistence::{
    DayInfo, cycle_entry_type, delete_entry, extract_day_content, get_entry_content,
    get_entry_type, load_day, load_day_lines, load_journal, mutate_entry, parse_day_header,
    save_day, save_day_lines, save_journal, scan_days_in_range, toggle_entry_complete,
    update_day_content, update_entry_content,
};

// Re-export date parsing types and functions
pub use date_parsing::{ParseContext, parse_date, parse_weekday};

// Re-export filter types and functions
pub use filter::{
    FAVORITE_TAG_REGEX, Filter, FilterType, LAST_TRAILING_TAG_REGEX, LATER_DATE_REGEX,
    RECURRING_REGEX, RELATIVE_DATE_REGEX, SAVED_FILTER_REGEX, TAG_CHAR_CLASS, TAG_REGEX,
    TRAILING_TAGS_REGEX, collect_filtered_entries, collect_journal_tags,
    collect_projected_entries_for_date, create_tag_delete_regex, create_tag_match_regex,
    defer_date, expand_favorite_tags, expand_saved_filters, extract_recurring_pattern,
    extract_tags, extract_target_date, normalize_relative_dates, parse_filter_date,
    parse_filter_query, parse_natural_date, parse_recurring_pattern, remove_date,
    strip_recurring_tags,
};

// Re-export project registry types
pub use project_registry::{
    ProjectInfo, ProjectRegistry, get_registry_path, set_hide_from_registry,
};
