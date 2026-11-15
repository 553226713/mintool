use crate::app::MintoolApp;
use crate::modules::MODULES;
use crate::ui::{dialogs::Dialog, theme};
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use gpui::*;

impl MintoolApp {
    /// Leftmost module switcher. Clicking the active module collapses or
    /// expands its sidebar.
    pub(crate) fn render_activity_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mut modules = Vec::new();
        for module in MODULES {
            let active = self.modules.active == module;
            modules.push(
                div()
                    .id(SharedString::from(format!("module-{}", module.slug())))
                    .mb_1()
                    .child(
                        Button::new(
                            SharedString::from(format!("module-button-{}", module.slug())),
                            "",
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Icon)
                        .icon(module.icon())
                        .tooltip(format!("{} · 点击切换或收起侧栏", module.title()))
                        .selected(active)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.switch_module(module, cx);
                        })),
                    )
                    .into_any_element(),
            );
        }
        div()
            .id("activity-bar")
            .w(px(52.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .pt_2()
            .bg(theme::panel())
            .border_r_1()
            .border_color(theme::border())
            .children(modules)
            .child(div().flex_1())
            .child(
                div().id("module-notices").mb_1().child(
                    Button::new("module-notices-button", "")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Icon)
                        .icon("bell")
                        .tooltip("通知历史")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_notice_history(window, cx)),
                        ),
                ),
            )
            .child(
                div().id("module-theme").mb_1().child(
                    Button::new("toggle-theme", "")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Icon)
                        .icon(self.settings.appearance.icon())
                        .tooltip(format!(
                            "切换为{}主题",
                            self.settings.appearance.toggle().label()
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings.appearance = this.settings.appearance.toggle();
                            theme::install(cx, this.settings.appearance);
                            this.changed(cx);
                        })),
                ),
            )
            .child(
                div().id("module-settings").mb_3().child(
                    Button::new("module-settings-button", "")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Icon)
                        .icon("settings-2")
                        .tooltip("设置")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_dialog(Dialog::Settings, window, cx)
                        })),
                ),
            )
            .into_any_element()
    }
}
