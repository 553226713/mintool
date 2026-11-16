//! Full-window editor for remote files; saving writes a temp file and uploads it.

use super::model::SshConnection;
use super::sftp::RemoteEntry;
use crate::app::MintoolApp;
use crate::ui::preview::file_preview::language_name;
use crate::ui::theme;
use gpui::*;
use gpui_component::input::{Input, InputState, TabSize};
use std::path::Path;

pub struct RemoteEdit {
    pub token: uuid::Uuid,
    pub owner: Option<(crate::model::session::SessionKey, u64)>,
    pub connection: SshConnection,
    pub remote: String,
    pub name: String,
    pub remote_dir: String,
    pub original: String,
    pub editor: Option<Entity<InputState>>,
    pub pending_text: Option<String>,
    pub loading: bool,
    pub saving: bool,
}

impl MintoolApp {
    pub(crate) fn remote_edit_dirty(&self, cx: &Context<Self>) -> bool {
        self.remote_edit.as_ref().is_some_and(|state| {
            state
                .editor
                .as_ref()
                .is_some_and(|editor| editor.read(cx).value().as_str() != state.original)
        })
    }

    pub(crate) fn open_remote_edit(&mut self, entry: RemoteEntry, cx: &mut Context<Self>) {
        if self.remote_edit.is_some() {
            return;
        }
        let Some(remote) = &self.modules.ssh.panel().remote else {
            return;
        };
        let Some(connection) = self.connection_of(&remote.connection) else {
            return;
        };
        let token = uuid::Uuid::new_v4();
        self.remote_edit = Some(RemoteEdit {
            token,
            owner: self.modules.ssh.active_panel.clone(),
            connection: connection.clone(),
            remote: entry.path.clone(),
            name: entry.name,
            remote_dir: remote.current.clone(),
            original: String::new(),
            editor: None,
            pending_text: None,
            loading: true,
            saving: false,
        });
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { super::remote_save::read_text(&connection, &entry.path) })
                .await;
            let _ = this.update(cx, |app, cx| {
                if let Some(state) = app
                    .remote_edit
                    .as_mut()
                    .filter(|state| state.token == token)
                {
                    state.loading = false;
                    match result {
                        Ok(text) => {
                            state.original = text.clone();
                            state.pending_text = Some(text);
                        }
                        Err(error) => {
                            app.error = Some(format!("无法编辑远程文件：{error:#}"));
                            app.remote_edit = None;
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn close_remote_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.remote_edit.as_ref().is_some_and(|state| state.saving) {
            return;
        }
        if self.remote_edit_dirty(cx) {
            self.show_dialog(
                crate::ui::dialogs::Dialog::RemoteEdit(super::edit_dialog::RemoteEditDialog {
                    quit: false,
                }),
                window,
                cx,
            );
        } else {
            self.remote_edit = None;
            cx.notify();
        }
    }

    pub(crate) fn save_remote_edit(&mut self, quit: bool, cx: &mut Context<Self>) {
        let Some(state) = &self.remote_edit else {
            return;
        };
        if state.saving {
            return;
        }
        let Some(editor) = &state.editor else {
            return;
        };
        let text = editor.read(cx).value().to_string();
        let token = state.token;
        let owner = state.owner.clone();
        let connection = state.connection.clone();
        let remote = state.remote.clone();
        let original = state.original.clone();
        let directory = state.remote_dir.clone();
        self.remote_edit.as_mut().unwrap().saving = true;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(
                    async move { super::remote_save::save(&connection, &remote, &original, &text) },
                )
                .await;
            let _ = this.update(cx, |app, cx| {
                if !app
                    .remote_edit
                    .as_ref()
                    .is_some_and(|state| state.token == token)
                {
                    return;
                }
                match result {
                    Ok(()) => {
                        app.remote_edit = None;
                        if let Some(panel) = owner
                            .as_ref()
                            .and_then(|key| app.modules.ssh.panels.get_mut(key))
                        {
                            if let Some(browser) = &mut panel.remote {
                                browser.entries.remove(&directory);
                            }
                        }
                        if quit {
                            app.quit_confirmed = true;
                            cx.quit();
                        } else {
                            app.error = Some("远程文件已保存".into());
                        }
                    }
                    Err(error) => {
                        app.remote_edit.as_mut().unwrap().saving = false;
                        app.error = Some(format!("保存失败，编辑内容已保留：{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn render_remote_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.remote_edit.as_mut()?;
        if state.editor.is_none() {
            if let Some(text) = state.pending_text.take() {
                let language = language_name(Path::new(&state.name));
                let editor = cx.new(|cx| {
                    let mut editor = InputState::new(window, cx)
                        .code_editor(language)
                        .soft_wrap(false)
                        .tab_size(TabSize {
                            tab_size: 4,
                            hard_tabs: false,
                        });
                    editor.set_value(text, window, cx);
                    editor
                });
                editor.update(cx, |editor, cx| editor.focus(window, cx));
                state.editor = Some(editor);
            }
        }
        let title = state.name.clone();
        let loading = state.loading;
        let saving = state.saving;
        let editor = state.editor.clone();
        let body = match &editor {
            Some(editor) => div()
                .flex_1()
                .min_h_0()
                .p_2()
                .child(
                    Input::new(editor)
                        .appearance(false)
                        .bordered(false)
                        .font_family("JetBrains Mono")
                        .text_size(px(13.))
                        .h_full(),
                )
                .into_any_element(),
            None => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme::muted())
                .child(if loading {
                    "正在下载远程文件…"
                } else {
                    "无法打开编辑器"
                })
                .into_any_element(),
        };
        Some(
            div()
                .id("remote-edit-backdrop")
                .absolute()
                .inset_0()
                .bg(rgba(0x080b13cc))
                .flex()
                .flex_col()
                .p(px(40.))
                .occlude()
                .child(
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .rounded_xl()
                        .border_1()
                        .border_color(theme::border())
                        .shadow_2xl()
                        .bg(theme::panel())
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(46.))
                                .px_4()
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap_3()
                                .border_b_1()
                                .border_color(theme::border())
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.))
                                        .text_color(theme::muted())
                                        .child("保存前检查远端修改"),
                                )
                                .child(
                                    adabraka_ui::components::button::Button::new(
                                        "save-remote-edit",
                                        if saving { "保存中…" } else { "保存" },
                                    )
                                    .variant(
                                        adabraka_ui::components::button::ButtonVariant::Default,
                                    )
                                    .size(adabraka_ui::components::button::ButtonSize::Sm)
                                    .disabled(saving || editor.is_none())
                                    .on_click(cx.listener(
                                        |this, _, _, cx| this.save_remote_edit(false, cx),
                                    )),
                                )
                                .child(
                                    adabraka_ui::components::button::Button::new(
                                        "close-remote-edit",
                                        "关闭",
                                    )
                                    .variant(adabraka_ui::components::button::ButtonVariant::Ghost)
                                    .size(adabraka_ui::components::button::ButtonSize::Sm)
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.close_remote_edit(window, cx),
                                    )),
                                ),
                        )
                        .child(body),
                )
                .into_any_element(),
        )
    }
}
