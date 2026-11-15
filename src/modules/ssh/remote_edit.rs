//! Full-window editor for remote files; saving writes a temp file and uploads it.

use super::model::SshConnection;
use super::sftp::{self, RemoteEntry};
use crate::app::MintoolApp;
use crate::ui::preview::file_preview::language_name;
use crate::ui::theme;
use gpui::*;
use gpui_component::input::{Input, InputState, TabSize};
use std::path::Path;

pub struct RemoteEdit {
    pub connection: SshConnection,
    pub remote: String,
    pub name: String,
    pub remote_dir: String,
    pub editor: Option<Entity<InputState>>,
    pub pending_text: Option<String>,
    pub loading: bool,
    pub saving: bool,
}

impl MintoolApp {
    pub(crate) fn open_remote_edit(&mut self, entry: RemoteEntry, cx: &mut Context<Self>) {
        let Some(remote) = &self.modules.ssh.remote else {
            return;
        };
        let Some(connection) = self.connection_of(&remote.connection) else {
            return;
        };
        let remote_dir = remote.current.clone();
        self.remote_edit = Some(RemoteEdit {
            connection: connection.clone(),
            remote: entry.path.clone(),
            name: entry.name.clone(),
            remote_dir,
            editor: None,
            pending_text: None,
            loading: true,
            saving: false,
        });
        cx.notify();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let remote_path = entry.path.clone();
            let result = executor
                .spawn(async move {
                    let temp = sftp::download_to_temp(&connection, &remote_path)?;
                    let text = std::fs::read_to_string(&temp)?;
                    let _ = std::fs::remove_file(&temp);
                    Ok::<_, anyhow::Error>(text)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(text) => {
                        if let Some(state) = &mut this.remote_edit {
                            state.loading = false;
                            state.pending_text = Some(text);
                        }
                    }
                    Err(error) => {
                        this.remote_edit = None;
                        this.error = Some(format!("无法编辑远程文件：{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn close_remote_edit(&mut self, cx: &mut Context<Self>) {
        self.remote_edit = None;
        cx.notify();
    }

    fn save_remote_edit(&mut self, cx: &mut Context<Self>) {
        let Some(state) = &self.remote_edit else {
            return;
        };
        let Some(editor) = &state.editor else {
            return;
        };
        let text = editor.read(cx).value().to_string();
        let connection = state.connection.clone();
        let remote = state.remote.clone();
        let remote_dir = state.remote_dir.clone();
        let name = state.name.clone();
        if let Some(state) = &mut self.remote_edit {
            state.saving = true;
        }
        cx.notify();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move {
                    let temp = std::env::temp_dir()
                        .join("mintool-remote-edit")
                        .join(format!("{}.tmp", uuid::Uuid::new_v4()));
                    if let Some(parent) = temp.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&temp, text.as_bytes())?;
                    let result = sftp::upload_to(&connection, &temp, &remote);
                    let _ = std::fs::remove_file(&temp);
                    result
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.remote_edit = None;
                        this.error = Some(format!("已保存远程文件 {name}"));
                        if let Some(browser) = &mut this.modules.ssh.remote {
                            browser.entries.remove(&remote_dir);
                            browser.loading.remove(&remote_dir);
                        }
                        this.start_remote_load(remote_dir.clone(), cx);
                    }
                    Err(error) => {
                        if let Some(state) = &mut this.remote_edit {
                            state.saving = false;
                        }
                        this.error = Some(format!("保存远程文件失败：{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
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
                                        .child("保存后上传覆盖远端文件"),
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
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.save_remote_edit(cx)),
                                    ),
                                )
                                .child(
                                    adabraka_ui::components::button::Button::new(
                                        "close-remote-edit",
                                        "关闭",
                                    )
                                    .variant(adabraka_ui::components::button::ButtonVariant::Ghost)
                                    .size(adabraka_ui::components::button::ButtonSize::Sm)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.close_remote_edit(cx)),
                                    ),
                                ),
                        )
                        .child(body),
                )
                .into_any_element(),
        )
    }
}
