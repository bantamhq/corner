pub(crate) mod agenda_widget;
pub(crate) mod autocomplete;
mod calendar;
mod container;
mod context;
mod daily;
mod filter;
mod footer;
mod header;
mod help;
mod helpers;
mod layout;
mod model;
mod overlay;
mod prep;
mod render;
mod rows;
mod scroll;
mod scroll_indicator;
mod shared;
pub mod surface;
pub(crate) mod theme;
mod view_model;
mod views;

use crate::app::App;
use ratatui::text::Line as RatatuiLine;

pub use context::RenderContext;
pub use daily::build_daily_list;
pub use filter::build_filter_list;
pub use prep::prepare_render;
pub use render::render_app;
pub use rows::build_calendar_row;
pub use rows::build_daily_entry_row;
pub use rows::build_edit_rows_with_prefix_width;
pub use rows::build_filter_row;
pub use rows::build_filter_selected_row;
pub use rows::build_projected_row;
pub use shared::{
    format_key_for_display, remove_all_trailing_tags, remove_last_trailing_tag, wrap_text,
};
pub use view_model::build_view_model;

pub fn render_daily_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
    daily::build_daily_list(app, width).into_lines()
}

pub fn render_filter_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
    filter::build_filter_list(app, width).into_lines()
}
