use ratatui::style::Color;

use super::surface::Surface;
use crate::storage::JournalSlot;

// Context primaries - change based on journal
pub const HUB_PRIMARY: Color = Color::Blue;
pub const PROJECT_PRIMARY: Color = Color::Cyan;
pub const EDIT_PRIMARY: Color = Color::Green;

// View mode colors
pub const DAILY_PRIMARY: Color = Color::White;
pub const FILTER_PRIMARY: Color = Color::LightMagenta;
pub const FILTER_BORDER: Color = Color::Magenta;

// Content highlighting
pub const TAG: Color = Color::Yellow;
pub const PROJECTED_DATE: Color = Color::Red;

// Confirm dialog
pub const CONFIRM_BORDER: Color = Color::Blue;
pub const CONFIRM_YES: Color = Color::Green;
pub const CONFIRM_NO: Color = Color::Red;

// Command palette
pub const PALETTE_ACCENT: Color = Color::Cyan;

// Border colors
pub const BORDER_DEFAULT: Color = Color::White;
pub const BORDER_MUTED: Color = Color::Gray;

// Text colors
pub const TEXT_MUTED: Color = Color::DarkGray;
pub const TEXT_ON_ACCENT: Color = Color::Black;

// Calendar
pub const CALENDAR_TEXT: Color = Color::White;

// Status indicator
pub const STATUS_TEXT: Color = Color::Gray;
pub const STATUS_ERROR: Color = Color::Red;

/// Returns the appropriate primary color based on journal context.
/// Used for cursor, headings, and other context-aware elements.
#[must_use]
pub fn context_primary(journal: JournalSlot) -> Color {
    match journal {
        JournalSlot::Hub => HUB_PRIMARY,
        JournalSlot::Project => PROJECT_PRIMARY,
    }
}

// Panel/overlay colors (derived from terminal surface)

/// Background for floating panels, overlays, and sidebars.
#[must_use]
pub fn panel_bg(surface: &Surface) -> Color {
    surface.gray2
}

/// Dividers and rules within panels.
#[must_use]
pub fn panel_rule(surface: &Surface) -> Color {
    surface.gray4
}

/// Secondary/description text on panels.
#[must_use]
pub fn secondary_text(surface: &Surface) -> Color {
    surface.muted_text
}

// Glyphs
pub const GLYPH_CURSOR: &str = "→";
pub const GLYPH_SELECTED: &str = "◉";
pub const GLYPH_UNSELECTED: &str = "○";
pub const GLYPH_REORDER: &str = "↕";
pub const GLYPH_PROJECTED_RECURRING: &str = "↺";
pub const GLYPH_PROJECTED_CALENDAR: &str = "○";

// Default color cycle for calendars (based on config order)
pub const CALENDAR_COLORS: &[Color] = &[
    Color::LightMagenta,
    Color::LightCyan,
    Color::LightBlue,
    Color::LightYellow,
    Color::LightRed,
    Color::LightGreen,
];

pub const GLYPH_SCROLL_UP: &str = "▲";
pub const GLYPH_SCROLL_DOWN: &str = "▼";
pub const GLYPH_SCROLL_BOTH: &str = "▲▼";
pub const GLYPH_CALENDAR: char = '●';
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

// Entry list buffer (rows reserved at bottom for scroll indicator)
pub const ENTRY_LIST_BOTTOM_BUFFER: u16 = 2;

// Agenda widget
pub const AGENDA_MIN_ENTRIES: usize = 7;
pub const AGENDA_MAX_DAYS_SEARCH: i64 = 365;
pub const AGENDA_DATE_WIDTH: usize = 12;
pub const AGENDA_ENTRY_PADDING: usize = 5;
pub const AGENDA_MIN_GUTTER: u16 = 20;
pub const AGENDA_BORDER_WIDTH: usize = 2;
