//! Shared sidebar widgets used by every feature module.

use crate::ui::theme;
use adabraka_ui::components::icon::Icon;
use gpui::*;

/// Collapsible group header (chevron + name + item count).
///
/// Callers attach their own click/drag/drop handlers and turn the result into
/// an element.
pub fn group_header(
    id: SharedString,
    name: String,
    count: usize,
    collapsed: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .mx_3()
        .mt_4()
        .mb_1()
        .h(px(28.))
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .rounded_md()
        .cursor_pointer()
        .hover(|style| style.bg(theme::elevated()))
        .child(
            Icon::new(if collapsed {
                "chevron-right"
            } else {
                "chevron-down"
            })
            .size(px(12.))
            .color(theme::muted()),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::muted())
                .child(name),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(count.to_string()),
        )
}
