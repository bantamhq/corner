mod command_palette;
mod confirm;
mod date_picker;
mod shared;

pub use command_palette::{CommandPaletteModel, render_command_palette};
pub use confirm::{ConfirmModel, render_confirm_modal};
pub use date_picker::{DatePickerModel, render_date_picker};

use ratatui::{Frame, layout::Rect};

use super::surface::Surface;

pub struct OverlayModel {
    pub confirm: Option<ConfirmModel>,
    pub command_palette: Option<CommandPaletteModel>,
    pub date_picker: Option<DatePickerModel>,
}

pub struct OverlayLayout<'a> {
    pub screen_area: Rect,
    pub surface: &'a Surface,
}

pub fn render_overlays(f: &mut Frame<'_>, overlays: OverlayModel, layout: OverlayLayout<'_>) {
    if let Some(confirm) = overlays.confirm {
        render_confirm_modal(f, layout.screen_area, confirm);
    }
    if let Some(palette) = overlays.command_palette {
        render_command_palette(f, layout.screen_area, palette, layout.surface);
    }
    if let Some(date_picker) = overlays.date_picker {
        render_date_picker(f, layout.screen_area, date_picker);
    }
}
