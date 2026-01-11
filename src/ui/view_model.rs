use ratatui::{layout::Alignment, text::Line as RatatuiLine};

use crate::app::{App, EditContext, InputMode, InterfaceContext, ViewMode};

use super::container::ContainerConfig;
use super::context::RenderContext;
use super::daily::build_daily_list;
use super::filter::build_filter_list;
use super::footer::FooterModel;
use super::model::ListModel;
use super::overlay::{
    ConfirmModel, HelpModel, HintModel, InterfaceModel, JournalIndicatorModel, StatusModel,
};
use super::prep::RenderPrep;
use super::scroll::CursorContext;

pub struct ViewModel<'a> {
    pub container: ContainerModel,
    pub overlays: OverlayModel<'a>,
    pub cursor: CursorModel,
}

pub struct ContainerModel {
    pub config: ContainerConfig,
    pub list: ListModel,
}

pub struct OverlayModel<'a> {
    pub status: StatusModel<'a>,
    pub footer: FooterModel<'a>,
    pub hint: HintModel<'a>,
    pub journal: JournalIndicatorModel,
    pub help: Option<HelpModel<'a>>,
    pub confirm: Option<ConfirmModel<'a>>,
    pub interface: Option<InterfaceModel<'a>>,
}

pub struct CursorModel {
    pub edit: Option<CursorContext>,
    pub prompt: Option<(u16, u16)>,
}

pub fn build_view_model<'a>(
    app: &'a App,
    context: &RenderContext,
    prep: RenderPrep,
) -> ViewModel<'a> {
    let is_filter_context = matches!(app.view, ViewMode::Filter(_))
        || matches!(
            app.input_mode,
            InputMode::Edit(EditContext::FilterEdit { .. })
                | InputMode::Edit(EditContext::FilterQuickAdd { .. })
        );

    let container_config = if is_filter_context {
        ContainerConfig::filter()
    } else {
        let date_title = app.current_date.format(" %m/%d/%y ").to_string();
        let title_line = RatatuiLine::from(date_title).alignment(Alignment::Right);
        ContainerConfig::daily(title_line)
    };

    let list = if is_filter_context {
        build_filter_list(app, context.content_width)
    } else {
        build_daily_list(app, context.content_width)
    };

    let current_project_id = app.current_project_id();

    let overlays = OverlayModel {
        status: StatusModel::new(app.status_message.as_deref()),
        footer: FooterModel::new(&app.view, &app.input_mode, &app.keymap),
        hint: HintModel::new(&app.hint_state),
        journal: JournalIndicatorModel::new(app.active_journal(), current_project_id.clone()),
        help: app
            .help_visible
            .then(|| HelpModel::new(&app.keymap, app.help_scroll, app.help_visible_height)),
        confirm: match &app.input_mode {
            InputMode::Confirm(confirm_context) => Some(ConfirmModel::new(confirm_context)),
            _ => None,
        },
        interface: match &app.input_mode {
            InputMode::Interface(ctx) => Some(match ctx {
                InterfaceContext::Date(state) => InterfaceModel::date(state),
                InterfaceContext::Project(state) => {
                    InterfaceModel::project(state, current_project_id.as_ref().cloned())
                }
                InterfaceContext::Tag(state) => InterfaceModel::tag(state),
            }),
            _ => None,
        },
    };

    ViewModel {
        container: ContainerModel {
            config: container_config,
            list,
        },
        overlays,
        cursor: CursorModel {
            edit: prep.edit_cursor,
            prompt: prep.prompt_cursor,
        },
    }
}
