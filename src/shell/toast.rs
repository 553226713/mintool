//! Floating toast used for errors and other short-lived messages.
//!
//! It is drawn as an overlay (absolutely positioned) so it never consumes
//! layout space, similar to Element UI's `Message` component.

use crate::app::MintoolApp;
use crate::ui::theme;
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use gpui::*;
use std::time::{Duration, Instant};

/// How long a toast stays visible before auto-dismissing.
pub(crate) const TOAST_LIFETIME: Duration = Duration::from_secs(5);

impl MintoolApp {
    /// Render the current message as a floating toast.
    ///
    /// `self.error` is also used for a few non-error status messages, so the
    /// toast keeps a neutral look instead of the old red banner.
    pub(crate) fn render_toast(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let message = self.error.clone();
        match (&message, &self.error_shown) {
            (Some(message), Some((shown, _))) if shown == message => {}
            (Some(message), _) => {
                self.error_shown = Some((message.clone(), Instant::now()));
                self.notice_history
                    .push_back((message.clone(), Instant::now()));
                while self.notice_history.len() > 50 {
                    self.notice_history.pop_front();
                }
            }
            (None, _) => self.error_shown = None,
        }
        let message = message?;

        Some(
            div()
                .absolute()
                .top(px(12.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    div()
                        .id("app-toast")
                        .flex()
                        .items_center()
                        .gap_3()
                        .w(px(420.))
                        .pl_4()
                        .pr_2()
                        .py_3()
                        .rounded(px(10.))
                        .bg(theme::elevated())
                        .border_1()
                        .border_color(theme::border())
                        .shadow_lg()
                        .text_color(theme::text())
                        .text_size(px(12.))
                        .child(
                            div()
                                .size(px(8.))
                                .rounded_full()
                                .flex_none()
                                .bg(theme::accent()),
                        )
                        .child(div().flex_1().min_w_0().child(message))
                        .child(
                            Button::new("toast-dismiss", "")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .icon("x")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.error = None;
                                    this.error_shown = None;
                                    this.toast_hovered = false;
                                    cx.notify();
                                })),
                        )
                        .on_hover(cx.listener(|this, hovered, _, cx| {
                            this.toast_hovered = *hovered;
                            if !*hovered {
                                if let Some((_, shown)) = this.error_shown.as_mut() {
                                    // Restart the countdown after the pointer leaves.
                                    *shown = Instant::now();
                                }
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}
