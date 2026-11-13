use crate::ui::file_preview::FilePreview;
use crate::{
    app::MintoolApp,
    model::{
        layout::{Axis, Layout, Pane},
        project::Project,
    },
    ui::theme,
};
use adabraka_ui::components::resizable::{h_resizable, resizable_panel, v_resizable};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
};
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
        let viewport = window.viewport_size();
        let (left_width, right_width) = self.settings.panel_widths(viewport.width.into());
        let width = f32::from(viewport.width) - left_width - right_width;
        let height = f32::from(viewport.height) - 91.;
        div()
            .size_full()
            .child(self.layout_node(project, layout, (width, height), window, cx))
            .into_any_element()
    }
    fn layout_node(
        &mut self,
        project: &Project,
        layout: &Layout,
        bounds: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match layout {
            Layout::Pane(pane) => self.render_pane(project, pane, bounds, window, cx),
            Layout::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let first_id = first.first_id();
                let second_id = second.first_id();
                let ratio = ratio.clamp(0.1, 0.9);
                let horizontal = *axis == Axis::Horizontal;
                let total = if horizontal { bounds.0 } else { bounds.1 };
                let size_a = if horizontal {
                    (bounds.0 * ratio, bounds.1)
                } else {
                    (bounds.0, bounds.1 * ratio)
                };
                let size_b = if horizontal {
                    (bounds.0 * (1. - ratio), bounds.1)
                } else {
                    (bounds.0, bounds.1 * (1. - ratio))
                };
                let a = self.layout_node(project, first, size_a, window, cx);
                let b = self.layout_node(project, second, size_b, window, cx);
                let key =
                    SharedString::from(format!("split-{}-{first_id}-{second_id}", project.id));
                let state = self.resize_state(
                    key.to_string(),
                    Some((project.id.clone(), first_id, second_id)),
                    cx,
                );
                let group = if horizontal {
                    h_resizable(key, state)
                } else {
                    v_resizable(key, state)
                };
                group
                    .child(
                        resizable_panel()
                            .size(px((total * ratio).max(100.)))
                            .child(a),
                    )
                    .child(
                        resizable_panel()
                            .size(px((total * (1. - ratio)).max(100.)))
                            .child(b),
                    )
                    .into_any_element()
            }
        }
    }
    fn render_pane(
        &mut self,
        project: &Project,
        pane: &Pane,
        bounds: (f32, f32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let focused = self
            .workspaces
            .get(&project.id)
            .is_some_and(|w| w.state.focused == pane.id);
        let closable = self
            .workspaces
            .get(&project.id)
            .is_some_and(|w| matches!(w.state.layout, Layout::Split { .. }));
        let id = pane.id;
        let compact = bounds.0 < 420.;
        let project_id = project.id.clone();
        let file_project = project.id.clone();
        let close_file_project = project.id.clone();
        let body_project = project.id.clone();
        let terminal_tab = div()
            .id(SharedString::from(format!("term-tab-{id}")))
            .h_full()
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .when(compact, |tab| tab.px_1().gap_1())
            .cursor_pointer()
            .text_color(if !pane.showing_file {
                theme::text()
            } else {
                theme::muted()
            })
            .when(!pane.showing_file, |tab| {
                tab.border_b_2().border_color(theme::accent())
            })
            .child(Icon::new("square-terminal").size(px(14.)))
            .when(!compact, |tab| tab.child(format!("终端 {id}")))
            .on_click(cx.listener(move |this, _, window, cx| {
                if let Some(w) = this.workspaces.get_mut(&project_id) {
                    w.state.focused = id;
                    if let Some(p) = w.state.layout.find_mut(id) {
                        p.showing_file = false;
                    }
                }
                if let Some(terminal) = this.terminals.get(&(project_id.clone(), id)) {
                    let focus = terminal.read(cx).focus.clone();
                    focus.focus(window);
                }
                this.changed(cx);
            }));
        let mut tabs = vec![terminal_tab.into_any_element()];
        if let Some(path) = &pane.file {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            tabs.push(
                div()
                    .id(SharedString::from(format!("file-tab-{id}")))
                    .h_full()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(compact, |tab| tab.px_1().gap_1())
                    .cursor_pointer()
                    .when(pane.showing_file, |tab| {
                        tab.border_b_2().border_color(theme::accent())
                    })
                    .text_color(if pane.showing_file {
                        theme::text()
                    } else {
                        theme::muted()
                    })
                    .child(crate::ui::file_icon::file_icon(path, false, false))
                    .when(!compact, |tab| {
                        tab.child(div().max_w(px(160.)).truncate().child(name))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(w) = this.workspaces.get_mut(&file_project) {
                            w.state.focused = id;
                            if let Some(p) = w.state.layout.find_mut(id) {
                                p.showing_file = true;
                            }
                        }
                        this.changed(cx);
                    }))
                    .child(
                        Button::new(SharedString::from(format!("close-file-{id}")), "")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .w(px(24.))
                            .min_w(px(24.))
                            .px_0()
                            .flex_none()
                            .icon("x")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                if let Some(w) = this.workspaces.get_mut(&close_file_project) {
                                    if let Some(p) = w.state.layout.find_mut(id) {
                                        p.file = None;
                                        p.showing_file = false;
                                    }
                                }
                                this.changed(cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        let header = div()
            .h(px(39.))
            .flex_none()
            .flex()
            .items_center()
            .text_size(px(11.))
            .bg(theme::panel())
            .border_b_1()
            .border_color(if focused {
                theme::accent()
            } else {
                theme::border()
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .children(tabs),
            )
            .child(
                Button::new(SharedString::from(format!("horizontal-{id}")), "")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .w(px(32.))
                    .min_w(px(32.))
                    .px_0()
                    .flex_none()
                    .icon("columns-2")
                    .tooltip("左右拆分")
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.split(id, Axis::Horizontal, cx)),
                    ),
            )
            .child(
                Button::new(SharedString::from(format!("vertical-{id}")), "")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .w(px(32.))
                    .min_w(px(32.))
                    .px_0()
                    .flex_none()
                    .icon("rows-2")
                    .tooltip("上下拆分")
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.split(id, Axis::Vertical, cx)),
                    ),
            )
            .child(
                Button::new(SharedString::from(format!("close-{id}")), "")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .w(px(32.))
                    .min_w(px(32.))
                    .px_0()
                    .flex_none()
                    .icon("x")
                    .tooltip("关闭面板")
                    .disabled(!closable)
                    .on_click(cx.listener(move |this, _, _, cx| this.close_pane(id, cx))),
            );
        let body = if pane.showing_file {
            self.file_preview(project, pane, (bounds.1 - 39.).max(34.), window, cx)
        } else if let Some(terminal) = self.terminal(project, id, cx) {
            if focused && self.focus_pending {
                let focus = terminal.read(cx).focus.clone();
                focus.focus(window);
                self.focus_pending = false;
            }
            let exited = *terminal
                .read(cx)
                .session
                .lock()
                .unwrap()
                .exited
                .lock()
                .unwrap();
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
            .child(header)
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
        self.terminals.remove(&(project.into(), id));
        if let Some(w) = self.workspaces.get_mut(project) {
            w.terminals.remove(&id);
            w.errors.remove(&id);
        }
        cx.notify();
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
        let text = match self
            .workspaces
            .get_mut(&project.id)
            .unwrap()
            .cache
            .document(path)
        {
            Ok(text) => text.to_string(),
            Err(error) => {
                return div()
                    .p_4()
                    .text_color(theme::danger())
                    .child(error.to_string())
                    .into_any_element()
            }
        };
        let changes = self
            .workspaces
            .get(&project.id)
            .and_then(|w| w.git.lines.get(path))
            .cloned()
            .unwrap_or_default();
        let key = (project.id.clone(), pane.id);
        if self
            .previews
            .get(&key)
            .is_none_or(|(old_path, _, _)| old_path != path)
        {
            let state = cx.new(|cx| FilePreview::new(text.clone(), height, cx));
            self.previews
                .insert(key.clone(), (path.clone(), text.clone(), state));
        }
        let (_, old_text, state) = self.previews.get_mut(&key).unwrap();
        if *old_text != text {
            state.update(cx, |state, cx| state.replace(text.clone(), cx));
            *old_text = text;
        }
        state.update(cx, |state, cx| {
            state.resize(height, cx);
            state.set_changes(changes, cx);
        });
        let _ = window;
        state.clone().into_any_element()
    }
}
