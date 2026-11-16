use crate::model::session::SessionKey;
use crate::model::session::{SessionSurface, SessionView, TerminalLauncher};
use crate::services::terminal::TerminalSession;
use crate::ui::terminal::TerminalView;
use crate::ui::{
    file_preview::FilePreview,
    image_preview::{is_image_path, ImagePreview},
};
use crate::{
    app::MintoolApp,
    services::mutex::lock_unpoisoned,
    ui::theme,
    workspace::layout::{Layout, Pane},
};
use adabraka_ui::components::button::{Button, ButtonSize};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::{Arc, Mutex};

impl MintoolApp {
    pub(crate) fn render_layout(
        &mut self,
        session: &SessionView,
        layout: &Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_dock(session, layout, window, cx)
    }
    pub(crate) fn render_pane(
        &mut self,
        session: &SessionView,
        pane: &Pane,
        bounds: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = pane.id;
        let focused = self
            .workspaces
            .get(&session.key)
            .is_some_and(|workspace| workspace.state.focused == id);
        let body_key = session.key.clone();
        let restart_key = session.key.clone();
        let retry_key = session.key.clone();
        let body = if session.surface == SessionSurface::Query {
            self.render_database_pane(session, pane, focused, window, cx)
        } else if let Some(remote) = &pane.remote {
            self.modules
                .render_remote_preview(session, pane.id, remote, bounds.1.max(34.), cx)
        } else if pane.showing_file {
            self.file_preview(session, pane, bounds.1.max(34.), window, cx)
        } else if pane.terminal_closed {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .text_color(theme::muted())
                .child("终端已关闭，关联进程已停止")
                .child(
                    Button::new(SharedString::from(format!("new-terminal-{id}")), "新建终端")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.restart_terminal(&restart_key, id, cx)
                        })),
                )
                .into_any_element()
        } else {
            let system_terminal = if matches!(session.launcher, TerminalLauncher::LocalShell) {
                self.system_terminal(session, id, window, cx)
            } else {
                None
            };
            if let Some(terminal) = system_terminal {
                terminal
            } else if let Some(terminal) = self.terminal(session, id, cx) {
                if focused && self.focus_pending {
                    let focus = terminal.read(cx).focus.clone();
                    focus.focus(window);
                    self.focus_pending = false;
                }
                let exited = {
                    let terminal = terminal.read(cx);
                    let terminal_session = lock_unpoisoned(&terminal.session);
                    terminal_session.has_exited()
                };
                let disconnected = match &session.launcher {
                    TerminalLauncher::Ssh(_) => "连接已断开",
                    TerminalLauncher::Docker { .. } => "容器进程已结束",
                    TerminalLauncher::LocalShell => "终端已退出",
                };
                div()
                    .size_full()
                    .relative()
                    .child(terminal)
                    .when(exited, |body| {
                        body.child(
                            div()
                                .absolute()
                                .bottom_3()
                                .left_3()
                                .px_3()
                                .py_2()
                                .bg(theme::elevated())
                                .rounded_md()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(disconnected)
                                .child(
                                    Button::new(
                                        SharedString::from(format!("restart-{id}")),
                                        "重新连接",
                                    )
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.restart_terminal(&retry_key, id, cx)
                                        },
                                    )),
                                ),
                        )
                    })
                    .into_any_element()
            } else {
                let error = self
                    .workspaces
                    .get(&session.key)
                    .and_then(|workspace| workspace.errors.get(&id))
                    .cloned()
                    .unwrap_or_default();
                div()
                    .size_full()
                    .p_4()
                    .child(error)
                    .child(
                        Button::new(SharedString::from(format!("retry-{id}")), "重试").on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.restart_terminal(&retry_key, id, cx)
                            }),
                        ),
                    )
                    .into_any_element()
            }
        };
        div()
            .id(SharedString::from(format!(
                "pane-{}-{id}",
                session.key.module.slug()
            )))
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(theme::background())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let mut changed = false;
                    if let Some(workspace) = this.workspaces.get_mut(&body_key) {
                        if workspace.state.focused != id {
                            workspace.state.focused = id;
                            changed = true;
                        }
                    }
                    if changed {
                        this.changed(cx);
                    }
                }),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(body),
            )
            .into_any_element()
    }
    pub(crate) fn restart_terminal(&mut self, key: &SessionKey, id: u64, cx: &mut Context<Self>) {
        self.stop_terminal(key, id, cx);
        if let Some(workspace) = self.workspaces.get_mut(key) {
            if let Some(pane) = workspace.state.layout.find_mut(id) {
                pane.terminal_closed = false;
            }
        }
        self.focus_pending = true;
        self.changed(cx);
    }
    fn file_preview(
        &mut self,
        session: &SessionView,
        pane: &Pane,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(path) = &pane.file else {
            return div().into_any_element();
        };
        let key = (session.key.clone(), pane.id);
        if is_image_path(path) {
            self.previews.remove(&key);
            return self.image_preview(path, key, height, cx);
        }
        self.image_previews.remove(&key);
        // A dirty buffer owns its content; only reload when there is nothing to lose.
        let unsaved = self
            .previews
            .get(&key)
            .is_some_and(|(_, _, state)| state.read(cx).has_unsaved_changes());
        if !unsaved {
            let text_changed = {
                let Some(workspace) = self.workspaces.get_mut(&session.key) else {
                    return div().into_any_element();
                };
                match workspace.cache.document_fast(path) {
                    Ok(text) => self
                        .previews
                        .get(&key)
                        .is_none_or(|(old_path, old_text, _)| old_path != path || old_text != text),
                    Err(error) => {
                        return div()
                            .p_4()
                            .text_color(theme::danger())
                            .child(error.to_string())
                            .into_any_element()
                    }
                }
            };
            if text_changed {
                let text = self
                    .workspaces
                    .get_mut(&session.key)
                    .and_then(|workspace| workspace.cache.cached_document(path))
                    .unwrap_or_default()
                    .to_string();
                if self
                    .previews
                    .get(&key)
                    .is_some_and(|(old_path, _, _)| old_path == path)
                {
                    let (_, old_text, state) = self.previews.get_mut(&key).unwrap();
                    state.update(cx, |state, cx| state.replace(text.clone(), window, cx));
                    *old_text = text;
                } else {
                    let state = cx
                        .new(|cx| FilePreview::new(path.clone(), text.clone(), height, window, cx));
                    self.previews
                        .insert(key.clone(), (path.clone(), text, state));
                }
            }
        }
        let state = self.previews[&key].2.clone();
        let changes = self
            .workspaces
            .get(&session.key)
            .and_then(|workspace| workspace.git.lines.get(path))
            .map(Vec::as_slice)
            .unwrap_or_default();
        let saved = state.update(cx, |state, cx| {
            state.set_changes(changes, cx);
            state.resize(height, cx);
            state.take_saved_text()
        });
        if let Some(text) = saved {
            if let Some((_, old_text, _)) = self.previews.get_mut(&key) {
                *old_text = text;
            }
            if let Some(workspace) = self.workspaces.get_mut(&session.key) {
                let mut paths = std::collections::BTreeSet::new();
                paths.insert(path.clone());
                workspace.cache.invalidate_paths(&paths);
            }
        }
        state.clone().into_any_element()
    }
    pub(crate) fn save_preview(
        &mut self,
        key: &SessionKey,
        pane: u64,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let preview_key = (key.clone(), pane);
        let Some(state) = self
            .previews
            .get(&preview_key)
            .map(|(_, _, state)| state.clone())
        else {
            return Ok(());
        };
        state
            .update(cx, |state, cx| state.save(cx))
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let saved = state.update(cx, |state, _| state.take_saved_text());
        if let Some(text) = saved {
            let mut paths = std::collections::BTreeSet::new();
            if let Some((path, old_text, _)) = self.previews.get_mut(&preview_key) {
                paths.insert(path.clone());
                *old_text = text;
            }
            if let Some(workspace) = self.workspaces.get_mut(key) {
                workspace.cache.invalidate_paths(&paths);
            }
        }
        Ok(())
    }
    pub(crate) fn save_all_previews(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let keys: Vec<(SessionKey, u64)> = self
            .previews
            .iter()
            .filter(|(_, (_, _, state))| state.read(cx).has_unsaved_changes())
            .map(|(key, _)| key.clone())
            .collect();
        for (session, pane) in keys {
            self.save_preview(&session, pane, cx)?;
        }
        Ok(())
    }

    fn image_preview(
        &mut self,
        path: &std::path::Path,
        key: (SessionKey, u64),
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self
            .image_previews
            .get(&key)
            .is_some_and(|(old_path, _)| old_path == path)
        {
            let state = self.image_previews[&key].1.clone();
            state.update(cx, |state, cx| state.resize(height, cx));
            return state.into_any_element();
        }
        let state = cx.new(|cx| ImagePreview::new(path.to_path_buf(), height, cx));
        self.image_previews
            .insert(key, (path.to_path_buf(), state.clone()));
        state.into_any_element()
    }
    pub(crate) fn terminal(
        &mut self,
        session: &SessionView,
        id: u64,
        cx: &mut Context<Self>,
    ) -> Option<Entity<TerminalView>> {
        let key = (session.key.clone(), id);
        if let Some(terminal) = self.terminals.get(&key) {
            return Some(terminal.clone());
        }
        let workspace = self.workspaces.get_mut(&session.key)?;
        if workspace.errors.contains_key(&id) {
            return None;
        }
        let terminal_session =
            match TerminalSession::spawn_spec(&session.terminal_spec(&self.settings.shell)) {
                Ok(session) => Arc::new(Mutex::new(session)),
                Err(error) => {
                    workspace.errors.insert(id, format!("{error:#}"));
                    return None;
                }
            };
        workspace.terminals.insert(id, terminal_session.clone());
        let terminal =
            cx.new(|cx| TerminalView::new(terminal_session, self.settings.font_size, cx));
        self.terminals.insert(key, terminal.clone());
        Some(terminal)
    }
}
