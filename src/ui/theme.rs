#![allow(dead_code)]

use ratatui::style::Color;

pub const BORDER_DAILY: Color = Color::White;
pub const BORDER_FILTER: Color = Color::Magenta;
pub const BORDER_FOCUSED: Color = Color::Cyan;

pub const STATUS_FG: Color = Color::Black;
pub const STATUS_BG: Color = Color::Yellow;

pub const JOURNAL_HUB: Color = Color::Green;
pub const JOURNAL_PROJECT: Color = Color::Blue;

pub const PROMPT_COMMAND: Color = Color::Blue;
pub const PROMPT_FILTER: Color = Color::Magenta;
pub const PROMPT_RENAME_TAG: Color = Color::Blue;
pub const PROMPT_TAG_HIGHLIGHT: Color = Color::Yellow;

pub const MODE_EDIT: Color = Color::Green;
pub const MODE_REORDER: Color = Color::Green;
pub const MODE_SELECTION: Color = Color::Green;
pub const MODE_INTERFACE: Color = Color::Blue;
pub const MODE_DAILY: Color = Color::Cyan;
pub const MODE_FILTER: Color = Color::Magenta;

pub const ENTRY_CURSOR: Color = Color::Cyan;
pub const ENTRY_SELECTION: Color = Color::Green;
pub const ENTRY_PROJECTED_ACTIVE: Color = Color::Cyan;
pub const ENTRY_PROJECTED_INACTIVE: Color = Color::Red;

pub const CALENDAR_INDICATOR: Color = Color::Blue;

pub const HELP_BORDER: Color = Color::Cyan;
pub const HELP_HEADER: Color = Color::Cyan;
pub const HELP_KEY: Color = Color::Yellow;
pub const HELP_DESC: Color = Color::White;

pub const CONFIRM_BORDER: Color = Color::Blue;
pub const CONFIRM_YES: Color = Color::Green;
pub const CONFIRM_NO: Color = Color::Red;

pub const FOOTER_INVERSE_FG: Color = Color::Black;
pub const FOOTER_HINT: Color = Color::Gray;
pub const FOOTER_KEY: Color = Color::Gray;

pub const HELP_FOOTER: Color = Color::White;

pub const HINT_BORDER: Color = Color::Gray;
pub const HINT_GUIDANCE: Color = Color::Gray;
pub const HINT_DESC: Color = Color::Gray;
pub const HINT_TAG: Color = Color::Yellow;
pub const HINT_COMMAND: Color = Color::Blue;
pub const HINT_FILTER: Color = Color::Magenta;
pub const HINT_DATE: Color = Color::Red;
pub const HINT_SAVED_FILTER: Color = Color::Magenta;

pub const PALETTE_BORDER: Color = Color::White;
pub const PALETTE_BG: Color = Color::Reset;
pub const PALETTE_HEADER: Color = Color::White;
pub const PALETTE_HINT: Color = Color::Gray;
pub const PALETTE_GROUP: Color = Color::Cyan;
pub const PALETTE_COMMAND: Color = Color::White;
pub const PALETTE_DESC: Color = Color::Gray;
pub const PALETTE_SELECTED_BG: Color = Color::Reset;
pub const PALETTE_SELECTED_FG: Color = Color::Reset;
pub const PALETTE_TAB_ACTIVE_BG: Color = Color::Reset;
pub const PALETTE_TAB_ACTIVE_FG: Color = Color::White;
pub const PALETTE_TAB_INACTIVE: Color = Color::Gray;
pub const PALETTE_TAB_RULE: Color = Color::DarkGray;
pub const PALETTE_TAB_ACTIVE_RULE: Color = Color::Cyan;

pub const POPUP_BORDER: Color = Color::Blue;
pub const POPUP_TITLE: Color = Color::Blue;
pub const POPUP_QUERY: Color = Color::Blue;
pub const POPUP_QUERY_DIM: Color = Color::Blue;
pub const POPUP_SCROLL: Color = Color::Gray;

pub const DATE_TASK: Color = Color::Yellow;
pub const DATE_EVENT: Color = Color::Magenta;
pub const DATE_CALENDAR: Color = Color::Blue;
pub const DATE_OTHER: Color = Color::Gray;
pub const DATE_TODAY: Color = Color::Cyan;

pub const PROJECT_SELECTED_FG: Color = Color::Black;
pub const PROJECT_SELECTED_BG: Color = Color::Yellow;
pub const PROJECT_NORMAL_FG: Color = Color::Yellow;

pub const TAG_SELECTED_FG: Color = Color::Black;
pub const TAG_SELECTED_BG: Color = Color::Yellow;
pub const TAG_NORMAL_FG: Color = Color::Yellow;

pub const ENTRY_TAG: Color = Color::Yellow;
pub const ENTRY_DATE: Color = Color::Red;

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

pub const SCROLL_LABEL: &str = " scroll";
pub const SCROLL_PADDING: &str = " ";
