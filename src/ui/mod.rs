mod container;
mod context;
mod daily;
mod date_interface;
mod filter;
mod footer;
mod help;
mod helpers;
mod hints;
mod interface_modal;
mod list_helpers;
mod model;
mod overlay;
mod prep;
mod project_interface;
mod render;
mod rows;
mod scroll;
mod scroll_indicator;
mod shared;
mod tag_interface;
mod theme;
mod view_model;

use crate::app::App;
use ratatui::text::Line as RatatuiLine;

pub use context::RenderContext;
pub use daily::build_daily_list;
pub use filter::build_filter_list;
pub use help::get_help_total_lines;
pub use prep::prepare_render;
pub use render::render_app;
pub use shared::{
    format_key_for_display, remove_all_trailing_tags, remove_last_trailing_tag, wrap_text,
};
pub use view_model::build_view_model;

pub fn render_daily_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
    daily::build_daily_list(app, width).to_lines()
}

pub fn render_filter_view(app: &App, width: usize) -> Vec<RatatuiLine<'static>> {
    filter::build_filter_list(app, width).to_lines()
}

pub fn render_footer(app: &App) -> RatatuiLine<'static> {
    footer::render_footer(footer::FooterModel::new(
        &app.view,
        &app.input_mode,
        &app.keymap,
    ))
}
