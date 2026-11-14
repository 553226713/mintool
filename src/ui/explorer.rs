use crate::{
    app::MintoolApp,
    ui::{file_icon::file_icon, theme},
};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::Path;

impl MintoolApp {
    pub(crate) fn render_explorer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(project) = self.current() else {
            return div().size_full().bg(theme::panel()).into_any_element();
        };
        let mut entries = Vec::new();
        let changes = self
            .workspaces
            .get(&project.id)
            .map_or(0, |w| w.git.changes.len());
        if self.changes {
            if let Some(w) = self.workspaces.get(&project.id) {
                if w.git.root.is_none() {
                    entries.push(
                        div()
                            .p_4()
                            .text_color(theme::muted())
                            .child("当前目录不在 Git 仓库中")
                            .into_any_element(),
                    );
                } else if w.git.changes.is_empty() {
                    entries.push(
                        div()
                            .p_4()
                            .text_color(theme::muted())
                            .child("工作区干净")
                            .into_any_element(),
                    );
                }
                for (path, status) in &w.git.changes {
                    let file = path.clone();
                    let menu_file = path.clone();
                    let name = path
                        .strip_prefix(w.git.root.as_deref().unwrap_or(&project.path))
                        .unwrap_or(path)
                        .display()
                        .to_string();
                    entries.push(
                        div()
                            .id(SharedString::from(format!("change-{}", path.display())))
                            .h(px(29.))
                            .px_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .text_color(theme::git_color(Some(status)))
                            .hover(|s| s.bg(theme::elevated()))
                            .child(file_icon(path, false, false))
                            .child(div().flex_1().truncate().child(name))
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family("DejaVu Sans Mono")
                                    .child(status.clone()),
                            )
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.open_file(file.clone(), cx)),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.file_menu(menu_file.clone(), false, event.position, cx);
                                    cx.stop_propagation();
                                }),
                            )
                            .into_any_element(),
                    );
                }
                if let Some(error) = &w.git.error {
                    entries.push(
                        div()
                            .p_3()
                            .text_color(theme::danger())
                            .child(error.clone())
                            .into_any_element(),
                    );
                }
            }
        } else {
            self.directory_rows(&project.id, &project.path, 0, &mut entries, cx);
        }
        let root = project.path.clone();
        div()
            .id("explorer-panel")
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.file_menu(root.clone(), true, event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .child(
                div()
                    .h(px(55.))
                    .flex_none()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new("files").size(px(15.)).text_color(theme::muted()))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("资源管理器"),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pb_3()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("files-tab", "文件")
                            .size(ButtonSize::Sm)
                            .map(|b| {
                                if self.changes {
                                    b.variant(ButtonVariant::Ghost)
                                } else {
                                    b.variant(ButtonVariant::Default)
                                }
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.changes = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("changes-tab", format!("变更 {changes}"))
                            .size(ButtonSize::Sm)
                            .map(|b| {
                                if self.changes {
                                    b.variant(ButtonVariant::Default)
                                } else {
                                    b.variant(ButtonVariant::Ghost)
                                }
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.changes = true;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .text_size(px(11.))
                    .text_color(theme::muted())
                    .border_t_1()
                    .border_color(theme::border())
                    .child(project.name),
            )
            .child(
                div()
                    .id("files-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .text_size(px(11.))
                    .children(entries),
            )
            .when(self.changes, |panel| {
                panel.child(
                    div()
                        .px_3()
                        .py_3()
                        .text_size(px(10.))
                        .text_color(theme::muted())
                        .child("两列：暂存区 / 工作区 · ?? 未跟踪"),
                )
            })
            .into_any_element()
    }
    fn directory_rows(
        &mut self,
        project_id: &str,
        path: &Path,
        depth: usize,
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        if depth > 32 {
            return;
        }
        let Some(workspace) = self.workspaces.get_mut(project_id) else {
            return;
        };
        let entries = match workspace.cache.entries(path) {
            Ok(entries) => entries,
            Err(error) => {
                rows.push(
                    div()
                        .p_3()
                        .text_color(theme::danger())
                        .child(error.to_string())
                        .into_any_element(),
                );
                return;
            }
        };
        for entry in entries.iter() {
            let workspace = self.workspaces.get(project_id).unwrap();
            let expanded = workspace.state.expanded.contains(&entry.path);
            let status = workspace.git.status(&entry.path).map(str::to_string);
            let color = theme::git_color(status.as_deref());
            let file = entry.path.clone();
            let project = project_id.to_string();
            let directory = entry.directory;
            let menu_file = entry.path.clone();
            rows.push(
                div()
                    .id(SharedString::from(format!("file-{}", entry.path.display())))
                    .h(px(28.))
                    .pr_3()
                    .pl(px(12. + depth as f32 * 14.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(color)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::elevated()))
                    .when(directory, |row| {
                        row.child(
                            Icon::new(if expanded {
                                "chevron-down"
                            } else {
                                "chevron-right"
                            })
                            .size(px(10.)),
                        )
                    })
                    .when(!directory, |row| row.child(div().w(px(10.))))
                    .child(file_icon(&entry.path, directory, expanded))
                    .child(div().flex_1().truncate().child(entry.name.clone()))
                    .when_some(status.filter(|_| !directory), |row, s| {
                        row.child(div().text_size(px(10.)).child(s))
                    })
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.file_menu(menu_file.clone(), directory, event.position, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if directory {
                            if let Some(w) = this.workspaces.get_mut(&project) {
                                if !w.state.expanded.remove(&file) {
                                    w.state.expanded.insert(file.clone());
                                }
                            }
                            this.changed(cx);
                        } else {
                            this.open_file(file.clone(), cx);
                        }
                    }))
                    .into_any_element(),
            );
            if directory && expanded {
                self.directory_rows(project_id, &entry.path, depth + 1, rows, cx);
            }
        }
    }
}
