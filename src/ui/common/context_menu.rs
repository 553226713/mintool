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
    pub(crate) fn render_context_menu(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (position, items) = self.menu.clone()?;
        let margin = px(8.);
        let inset = window.client_inset().unwrap_or(px(0.));
        let max_height = (window.viewport_size().height - (margin + inset) * 2.).max(px(1.));
        let terminals: Vec<_> = self.system_terminals.values().cloned().collect();
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
                            .snap_to_window_with_margin(margin)
                            .child(
                                div()
                                    .id("context-menu")
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .relative()
                                    .child(
                                        div()
                                            .id("context-menu-scroll")
                                            .max_h(max_height)
                                            .overflow_y_scroll()
                                            .child(Menu::new(items).max_height(None)),
                                    )
                                    .child(
                                        canvas(
                                            move |bounds, window, cx| {
                                                for terminal in &terminals {
                                                    terminal.update(cx, |view, _| {
                                                        view.set_menu_bounds(Some(bounds), window);
                                                    });
                                                }
                                            },
                                            |_, _, _, _| {},
                                        )
                                        .absolute()
                                        .inset_0(),
                                    ),
                            ),
                    ),
            )
            .into_any_element(),
        )
    }
}
