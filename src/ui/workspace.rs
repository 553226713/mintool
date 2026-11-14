use crate::ui::{
    file_preview::FilePreview,
    image_preview::{is_image_path, ImagePreview},
};
use crate::{
    app::MintoolApp,
    model::{
        layout::{Layout, Pane},
        project::Project,
    },
    services::mutex::lock_unpoisoned,
    ui::theme,
};
use adabraka_ui::components::button::{Button, ButtonSize};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MintoolApp {
    pub(crate) fn render_layout(
        &mut self,
        project: &Project,
        layout: &Layout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_dock(project, layout, window, cx)
    }
    pub(crate) fn render_pane(
        &mut self,
        project: &Project,
        pane: &Pane,
        bounds: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = pane.id;
        let focused = self
            .workspaces
            .get(&project.id)
            .is_some_and(|workspace| workspace.state.focused == id);
        let body_project = project.id.clone();
        let body =
            if pane.showing_file {
                self.file_preview(project, pane, bounds.1.max(34.), window, cx)
            } else if pane.terminal_closed {
                let project = project.id.clone();
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
                            .on_click(cx.listener(move |app, _, _, cx| {
                                app.restart_terminal(&project, id, cx)
                            })),
                    )
                    .into_any_element()
            } else if let Some(terminal) = self.system_terminal(project, id, window, cx) {
                terminal
            } else if let Some(terminal) = self.terminal(project, id, cx) {
                if focused && self.focus_pending {
                    let focus = terminal.read(cx).focus.clone();
                    focus.focus(window);
                    self.focus_pending = false;
                }
                let exited = {
                    let terminal = terminal.read(cx);
                    let session = lock_unpoisoned(&terminal.session);
                    session.has_exited()
                };
                let project_id = project.id.clone();
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
                                .child("终端已退出")
                                .child(
                                    Button::new(
                                        SharedString::from(format!("restart-{id}")),
                                        "重新启动",
                                    )
                                    .size(ButtonSize::Sm)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.restart_terminal(&project_id, id, cx)
                                        },
                                    )),
                                ),
                        )
                    })
                    .into_any_element()
            } else {
                let error = self
                    .workspaces
                    .get(&project.id)
                    .and_then(|w| w.errors.get(&id))
                    .cloned()
                    .unwrap_or_default();
                let project_id = project.id.clone();
                div()
                    .size_full()
                    .p_4()
                    .child(error)
                    .child(
                        Button::new(SharedString::from(format!("retry-{id}")), "重试").on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.restart_terminal(&project_id, id, cx)
                            }),
                        ),
                    )
                    .into_any_element()
            };
        div()
            .id(SharedString::from(format!("pane-{}-{id}", project.id)))
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
                    if let Some(w) = this.workspaces.get_mut(&body_project) {
                        if w.state.focused != id {
                            w.state.focused = id;
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
    fn restart_terminal(&mut self, project: &str, id: u64, cx: &mut Context<Self>) {
        self.stop_terminal(project, id, cx);
        if let Some(w) = self.workspaces.get_mut(project) {
            if let Some(pane) = w.state.layout.find_mut(id) {
                pane.terminal_closed = false;
            }
        }
        self.focus_pending = true;
        self.changed(cx);
    }
    fn file_preview(
        &mut self,
        project: &Project,
        pane: &Pane,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(path) = &pane.file else {
            return div().into_any_element();
        };
        let key = (project.id.clone(), pane.id);
        if is_image_path(path) {
            self.previews.remove(&key);
            return self.image_preview(path, key, height, cx);
        }
        self.image_previews.remove(&key);
        let text_changed = {
            let cache = &mut self.workspaces.get_mut(&project.id).unwrap().cache;
            match cache.document_fast(path) {
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
                .get_mut(&project.id)
                .unwrap()
                .cache
                .cached_document(path)
                .unwrap()
                .to_string();
            if self
                .previews
                .get(&key)
                .is_some_and(|(old_path, _, _)| old_path == path)
            {
                let (_, old_text, state) = self.previews.get_mut(&key).unwrap();
                state.update(cx, |state, cx| state.replace(text.clone(), cx));
                *old_text = text;
            } else {
                let state = cx.new(|cx| FilePreview::new(path.clone(), text.clone(), height, cx));
                self.previews
                    .insert(key.clone(), (path.clone(), text, state));
            }
        }
        let changes = self
            .workspaces
            .get(&project.id)
            .and_then(|w| w.git.lines.get(path))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let changes_changed = !self.previews[&key].2.read(cx).changes_equal(changes);
        let changes = changes_changed.then(|| changes.to_vec());
        let (_, _, state) = self.previews.get_mut(&key).unwrap();
        state.update(cx, |state, cx| {
            state.resize(height, cx);
            if let Some(changes) = changes {
                state.set_changes(changes, cx);
            }
        });
        let _ = window;
        state.clone().into_any_element()
    }

    fn image_preview(
        &mut self,
        path: &std::path::Path,
        key: (String, u64),
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
}
