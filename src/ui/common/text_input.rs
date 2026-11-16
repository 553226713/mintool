//! Shared single-line fields for search and inline text entry.

use crate::ui::theme;
use adabraka_ui::components::icon::Icon;
use gpui::*;
use gpui_component::input::{Input, InputState};

/// Use the editor-backed input so text, selection and horizontal scrolling
/// share the same layout and input handling as the application's editor.
pub fn text_input(state: &Entity<InputState>) -> Input {
    Input::new(state)
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .text_size(px(13.))
        .text_color(theme::text())
}

pub fn search_input(state: &Entity<InputState>) -> Input {
    text_input(state).prefix(Icon::new("search").size(px(14.)).color(theme::muted()))
}
