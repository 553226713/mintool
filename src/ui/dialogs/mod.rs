//! Generic modal dialog shell.
//!
//! Feature modules own their dialog enums and business logic. This file only
//! knows the common frame (title, input, footer) and delegates everything else
//! through [`DialogContent`].

mod conflict;
mod diff;
pub mod directory_picker;
mod notices;
mod settings;
mod unsaved;

use crate::app::MintoolApp;
use crate::modules::docker::dialogs::DockerDialog;
use crate::modules::projects::dialogs::ProjectDialog;
use crate::modules::ssh::dialogs::SshDialog;
use crate::ui::theme;

use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use adabraka_ui::components::input::Input;
use adabraka_ui::components::input_state::SelectAll;
pub use conflict::ConflictDialog;
pub use diff::DiffDialog;
use gpui::prelude::FluentBuilder;
use gpui::*;
pub use notices::NoticeHistoryDialog;
pub use unsaved::UnsavedDialog;

#[derive(Clone)]
pub enum Dialog {
    Project(ProjectDialog),
    Ssh(SshDialog),
    Docker(DockerDialog),
    Settings,
    Unsaved(UnsavedDialog),
    Conflict(ConflictDialog),
    Diff(DiffDialog),
    Notices(NoticeHistoryDialog),
}

/// Presentation contract implemented by every module-owned dialog.
pub trait DialogContent {
    fn title(&self) -> String;
    fn input_value(&self, app: &MintoolApp) -> String {
        let _ = app;
        String::new()
    }
    /// Label above the text input. `None` hides the input entirely.
    fn input_hint(&self) -> Option<&'static str> {
        None
    }
    /// Dialogs such as confirmations can be submitted without text.
    fn accepts_empty_input(&self) -> bool {
        false
    }
    fn is_directory(&self) -> bool {
        false
    }
    /// Bold subject (project/connection/resource name).
    fn subject(&self) -> Option<String> {
        None
    }
    /// Small muted path line.
    fn path(&self) -> Option<String> {
        None
    }
    /// Description rendered before the subject.
    fn note(&self, app: &MintoolApp) -> Option<String> {
        let _ = app;
        None
    }
    /// Like [`Self::note`], but may inspect live entity state (for example
    /// unsaved preview buffers). Defaults to the static note.
    fn live_note(&self, app: &MintoolApp, cx: &Context<MintoolApp>) -> Option<String> {
        let _ = cx;
        self.note(app)
    }
    fn confirm_label(&self) -> &'static str {
        "保存"
    }
    fn enabled(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> bool {
        self.accepts_empty_input() || !app.dialog_input.read(cx).content().trim().is_empty()
    }
    fn render_extra(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        let _ = (app, cx);
        None
    }
    /// Read-only dialogs hide the cancel/confirm footer.
    fn has_actions(&self) -> bool {
        true
    }
    /// Extra buttons placed before the cancel/confirm pair.
    fn render_footer(&self, app: &MintoolApp, cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        let _ = (app, cx);
        None
    }
    fn prepare(&self, app: &mut MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
        let _ = (app, window, cx);
    }
    fn focus(&self, app: &MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
        app.dialog_input.read(cx).focus_handle(cx).focus(window);
    }
    fn discard_label(&self) -> Option<&'static str> {
        None
    }
    fn discard(&self, app: &mut MintoolApp, cx: &mut Context<MintoolApp>) {
        let _ = (app, cx);
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()>;
}

impl Dialog {
    fn content(&self) -> &dyn DialogContent {
        match self {
            Dialog::Project(dialog) => dialog,
            Dialog::Ssh(dialog) => dialog,
            Dialog::Docker(dialog) => dialog,
            Dialog::Settings => settings::dialog(),
            Dialog::Unsaved(dialog) => dialog,
            Dialog::Conflict(dialog) => dialog,
            Dialog::Diff(dialog) => dialog,
            Dialog::Notices(dialog) => dialog,
        }
    }
}

impl MintoolApp {
    pub(crate) fn show_dialog(
        &mut self,
        dialog: Dialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialog_generation += 1;
        self.dialog_error = None;
        dialog.content().prepare(self, window, cx);
        let value = dialog.content().input_value(self);
        self.dialog_input.update(cx, |state, cx| {
            state.select_all(&SelectAll, window, cx);
            state.set_value(value, window, cx);
        });
        self.dialog = Some(dialog);
        cx.notify();
        let dialog = self.dialog.clone().expect("dialog was just stored");
        dialog.content().focus(self, window, cx);
    }

    pub(crate) fn submit_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.clone() else {
            return;
        };
        let value = self.dialog_input.read(cx).content().trim().to_string();
        let content = dialog.content();
        if value.is_empty() && !content.accepts_empty_input() {
            self.dialog_error = Some("请填写内容后再保存".into());
            cx.notify();
            return;
        }
        match content.submit(self, value, cx) {
            Ok(()) => {
                self.dialog = None;
                self.dialog_error = None;
                self.changed(cx);
            }
            Err(error) => {
                self.dialog_error = Some(format!("{error:#}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn render_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.dialog.clone()?;
        let content = dialog.content();
        let title = content.title();
        let note = content.live_note(self, cx);
        let path = content.path();
        let subject = content.subject();
        let hint = content.input_hint();
        let extra = content.render_extra(self, cx);
        let footer = content.render_footer(self, cx);
        let enabled = content.enabled(self, cx);
        let has_actions = content.has_actions();
        let confirm = content.confirm_label();
        let directory = content.is_directory();
        let discard_label = content.discard_label();
        let app = cx.entity().downgrade();
        Some(
            div()
                .id("dialog-backdrop")
                .absolute()
                .inset_0()
                .bg(rgba(0x080b1399))
                .flex()
                .items_center()
                .justify_center()
                .occlude()
                .track_focus(&self.dialog_focus)
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.dialog = None;
                        cx.notify();
                        cx.stop_propagation();
                    } else if event.keystroke.key == "enter" {
                        this.submit_dialog(cx);
                        cx.stop_propagation();
                    }
                }))
                .child(
                    div()
                        .w(px(460.))
                        .rounded_xl()
                        .border_1()
                        .border_color(theme::border())
                        .shadow_2xl()
                        .bg(theme::panel())
                        .p_6()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(18.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .child(
                                    Button::new("cancel-icon", "")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm)
                                        .icon("x")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.dialog = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .when_some(note, |card, note| {
                            card.child(div().text_color(theme::muted()).child(note))
                        })
                        .when_some(path, |card, path| {
                            card.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme::muted())
                                    .child(path),
                            )
                        })
                        .when_some(subject, |card, subject| {
                            card.child(div().font_weight(FontWeight::SEMIBOLD).child(subject))
                        })
                        .when_some(hint, |card, hint| {
                            card.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme::muted())
                                    .child(hint),
                            )
                            .child(
                                Input::new(&self.dialog_input)
                                    .map(|mut input| {
                                        input.style().align_items = Some(AlignItems::Stretch);
                                        input
                                    })
                                    .w_full(),
                            )
                        })
                        .children(extra)
                        .when_some(self.dialog_error.clone(), |card, error| {
                            card.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme::danger())
                                    .child(error),
                            )
                        })
                        .when(has_actions, |card| {
                            card.child(
                                div()
                                    .pt_2()
                                    .flex()
                                    .gap_2()
                                    .items_center()
                                    .when(directory, |row| {
                                        row.child(
                                            Button::new("browse", "选择目录…")
                                                .icon("folder-open")
                                                .on_click(move |_, window, cx| {
                                                    let _ = app.update(cx, |this, cx| {
                                                        this.choose_directory(window, cx)
                                                    });
                                                }),
                                        )
                                    })
                                    .child(div().flex_1())
                                    .children(footer)
                                    .child(
                                        Button::new("cancel-dialog", "取消")
                                            .variant(ButtonVariant::Outline)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.dialog = None;
                                                cx.notify();
                                            })),
                                    )
                                    .when_some(discard_label, |row, label| {
                                        row.child(
                                            Button::new("discard-dialog", label)
                                                .variant(ButtonVariant::Destructive)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if let Some(dialog) = this.dialog.clone() {
                                                        dialog.content().discard(this, cx);
                                                    }
                                                    this.dialog = None;
                                                    this.changed(cx);
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new("submit-dialog", confirm)
                                            .variant(ButtonVariant::Default)
                                            .disabled(!enabled)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.submit_dialog(cx)
                                                }),
                                            ),
                                    ),
                            )
                        }),
                )
                .into_any_element(),
        )
    }
}

// ---------------------------------------------------------------------------
// App-level dialogs (not owned by a feature module)
// ---------------------------------------------------------------------------
