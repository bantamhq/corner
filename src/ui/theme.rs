use ratatui::style::Color;

use crate::storage::JournalSlot;

// Context primaries - change based on mode/journal
pub const HUB_PRIMARY: Color = Color::Blue;
pub const PROJECT_PRIMARY: Color = Color::Cyan;
pub const FILTER_PRIMARY: Color = Color::Magenta;
pub const EDIT_PRIMARY: Color = Color::Green;

// Content highlighting
pub const TAG: Color = Color::Yellow;
pub const PROJECTED_DATE: Color = Color::Red;

// Calendar (independent set)
pub const CALENDAR_TASK: Color = Color::Yellow;
pub const CALENDAR_EVENT: Color = Color::Magenta;
pub const CALENDAR_ENTRY: Color = Color::Blue;
pub const CALENDAR_TODAY: Color = Color::Cyan;
pub const CALENDAR_OTHER: Color = Color::Gray;

// Confirm dialog (semantic)
pub const CONFIRM_YES: Color = Color::Green;
pub const CONFIRM_NO: Color = Color::Red;

/// Returns the appropriate primary color based on journal and view context.
/// Used for cursor, projected indicators, and other context-aware elements.
#[must_use]
pub fn context_primary(journal: JournalSlot, in_filter: bool) -> Color {
    if in_filter {
        FILTER_PRIMARY
    } else {
        match journal {
            JournalSlot::Hub => HUB_PRIMARY,
            JournalSlot::Project => PROJECT_PRIMARY,
        }
    }
}

// Glyphs
pub const GLYPH_CURSOR: &str = "→";
pub const GLYPH_SELECTED: &str = "◉";
pub const GLYPH_UNSELECTED: &str = "○";
pub const GLYPH_REORDER: &str = "↕";
pub const GLYPH_PROJECTED_LATER: &str = "↪";
pub const GLYPH_PROJECTED_RECURRING: &str = "↺";
pub const GLYPH_PROJECTED_CALENDAR: &str = "○";

pub const GLYPH_SCROLL_UP: &str = "▲";
pub const GLYPH_SCROLL_DOWN: &str = "▼";
pub const GLYPH_SCROLL_BOTH: &str = "▲▼";
pub const GLYPH_AGENDA_CALENDAR: char = '○';
pub const GLYPH_AGENDA_EVENT: char = '*';
pub const GLYPH_AGENDA_RECURRING: char = '↪';
pub const GLYPH_AGENDA_FALLBACK: char = '•';

pub const SCROLL_LABEL: &str = " scroll";
pub const SCROLL_PADDING: &str = " ";

// View heading
pub const HEADING_PADDING: usize = 2;

// Sidebar layout
pub const CALENDAR_PANEL_HEIGHT: u16 = 10;
pub const UPCOMING_MIN_HEIGHT: u16 = 3;

// Agenda widget
pub const AGENDA_MIN_ENTRIES: usize = 7;
pub const AGENDA_MAX_DAYS_SEARCH: i64 = 365;
pub const AGENDA_DATE_WIDTH: usize = 12;
pub const AGENDA_ENTRY_PADDING: usize = 5;
pub const AGENDA_MIN_GUTTER: u16 = 20;
pub const AGENDA_BORDER_WIDTH: usize = 2;
