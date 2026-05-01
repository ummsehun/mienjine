mod aspect;
mod confirm;
mod footer;
mod header;
mod min_size;
mod summary;

use ratatui::prelude::*;

use super::theme::StartUiTheme;
use crate::interfaces::tui::helpers::tr;
use crate::runtime::config::UiLanguage;

fn summary_kv_line(
    ui_language: UiLanguage,
    theme: StartUiTheme,
    ko: &str,
    en: &str,
    value: impl Into<String>,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{}: ", tr(ui_language, ko, en)),
            theme.text_secondary,
        ),
        Span::styled(value.into(), theme.text_primary),
    ])
}

pub(super) use aspect::draw_aspect_calibration;
pub(super) use confirm::draw_confirm_panel;
pub(super) use footer::draw_action_bar;
pub(super) use header::{draw_header, draw_stepper};
pub(super) use min_size::draw_min_size_screen;
pub(super) use summary::draw_summary_panel;
