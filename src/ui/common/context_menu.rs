use crate::app::MintoolApp;
use adabraka_ui::navigation::menu::{Menu, MenuItem};
use gpui::*;
impl MintoolApp {
    pub(crate) fn menu_action(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        cx: &Context<Self>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> MenuItem {
        let app = cx.entity().downgrade();
        MenuItem::new(id, label).on_click(move |window, cx| {
            let _ = app.update(cx, |this, cx| {
                this.menu = None;
                action(this, window, cx);
                cx.notify();
            });
        })
    }
    pub(crate) fn render_context_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (position, items) = self.menu.clone()?;
        Some(
            deferred(
                div()
                    .id("menu-layer")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.menu = None;
                            cx.notify();
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            this.menu = None;
                            cx.notify();
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        anchored()
                            .position(position)
                            .snap_to_window_with_margin(px(8.))
                            .child(
                                div()
                                    .id("context-menu")
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .child(Menu::new(items)),
                            ),
                    ),
            )
            .into_any_element(),
        )
    }
}
