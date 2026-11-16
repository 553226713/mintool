use crate::{
    app::MintoolApp,
    modules::SessionKey,
    ui::{
        drag_item::{DragItem, DragKind},
        file_icon::file_icon,
        theme,
    },
};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
    input::{Input, InputSize},
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::{Path, PathBuf};

impl MintoolApp {
    pub(crate) fn render_explorer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(project) = self.current() else {
            return div().size_full().bg(theme::panel()).into_any_element();
        };
        let mut entries = Vec::new();
        let changes = self
            .workspaces
            .get(&SessionKey::project(&project.id))
            .map_or(0, |w| w.git.changes.len());
        let git_ready = self
            .workspaces
            .get(&SessionKey::project(&project.id))
            .is_some_and(|w| w.git.root.is_some());
        let file_filter = self
            .modules
            .projects
            .file_filter
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let filter_row = (!self.changes).then(|| {
            div()
                .flex()
                .flex_col()
                .w_full()
                .flex_none()
                .px_2()
                .pb_2()
                .child(crate::ui::common::text_input::search_input(
                    &self.modules.projects.file_filter,
                ))
                .into_any_element()
        });
        let commit_ready = self.changes
            && changes > 0
            && !self
                .modules
                .projects
                .commit_input
                .read(cx)
                .value()
                .trim()
                .is_empty();
        let commit_row = (self.changes && git_ready).then(|| {
            div()
                .w_full()
                .flex_none()
                .px_2()
                .pb_2()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex().flex_col().flex_1().min_w_0().child(
                    crate::ui::common::text_input::text_input(&self.modules.projects.commit_input),
                ))
                .child(
                    Button::new("commit-git", "提交")
                        .flex_none()
                        .size(ButtonSize::Sm)
                        .disabled(!commit_ready)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.commit_git_changes(window, cx)),
                        ),
                )
                .into_any_element()
        });
        if self.changes {
            if let Some(w) = self.workspaces.get(&SessionKey::project(&project.id)) {
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
                    let stage_path = path.clone();
                    let unstage_path = path.clone();
                    let diff_path = path.clone();
                    let staged = w.git.is_staged(path);
                    let unstaged = w.git.is_unstaged(path);
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
                            .when(unstaged, |row| {
                                row.child(
                                    Button::new(
                                        SharedString::from(format!(
                                            "stage-{}",
                                            stage_path.display()
                                        )),
                                        "＋",
                                    )
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .tooltip("暂存")
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.stage_git_path(stage_path.clone(), true, cx);
                                            cx.stop_propagation();
                                        },
                                    )),
                                )
                            })
                            .when(staged, |row| {
                                row.child(
                                    Button::new(
                                        SharedString::from(format!(
                                            "unstage-{}",
                                            unstage_path.display()
                                        )),
                                        "－",
                                    )
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .tooltip("取消暂存")
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.stage_git_path(unstage_path.clone(), false, cx);
                                            cx.stop_propagation();
                                        },
                                    )),
                                )
                            })
                            .child(
                                Button::new(
                                    SharedString::from(format!("diff-{}", diff_path.display())),
                                    "Δ",
                                )
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .tooltip("查看差异")
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.open_git_diff(diff_path.clone(), staged, window, cx);
                                        cx.stop_propagation();
                                    },
                                )),
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
        } else if file_filter.is_empty() {
            self.directory_rows(&project.id, &project.path, 0, &mut entries, cx);
        } else {
            let cache_key = format!("{}:{}", project.id, file_filter);
            let cached = self
                .modules
                .projects
                .filter_cache
                .as_ref()
                .filter(|(key, _)| key == &cache_key)
                .map(|(_, entries)| entries.clone());
            let results = match cached {
                Some(entries) => entries,
                None => {
                    let mut found = Vec::new();
                    let mut visited = 0;
                    self.collect_filter_entries(
                        &project.id,
                        &project.path,
                        &file_filter,
                        0,
                        &mut found,
                        &mut visited,
                    );
                    self.modules.projects.filter_cache = Some((cache_key, found.clone()));
                    found
                }
            };
            for entry in &results {
                entries.push(self.filter_row(&project.id, &project.path, entry, cx));
            }
            if results.is_empty() {
                entries.push(
                    div()
                        .p_4()
                        .text_color(theme::muted())
                        .child("没有匹配的文件")
                        .into_any_element(),
                );
            }
        }
        let root = project.path.clone();
        let drop_root = project.path.clone();
        div()
            .id("explorer-panel")
            .on_drop(cx.listener(move |this, item: &DragItem, _, cx| {
                if let DragKind::Files(paths) = &item.kind {
                    this.move_files_to(paths.clone(), drop_root.clone(), cx);
                }
            }))
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
            .children(commit_row)
            .children(filter_row)
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
        let Some(workspace) = self.workspaces.get_mut(&SessionKey::project(project_id)) else {
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
            let workspace = self
                .workspaces
                .get(&SessionKey::project(project_id))
                .unwrap();
            let expanded = workspace.state.expanded.contains(&entry.path);
            let status = workspace.git.status(&entry.path).map(str::to_string);
            let color = theme::git_color(status.as_deref());
            let file = entry.path.clone();
            let project = project_id.to_string();
            let directory = entry.directory;
            let menu_file = entry.path.clone();
            let selected = self.modules.projects.selected_files.contains(&entry.path);
            let renaming = self.modules.projects.renaming.as_ref() == Some(&entry.path);
            let selected_paths: Vec<PathBuf> = if selected {
                self.modules
                    .projects
                    .selected_files
                    .iter()
                    .cloned()
                    .collect()
            } else {
                vec![entry.path.clone()]
            };
            let drag_name = if selected {
                format!("{} 项", selected_paths.len())
            } else {
                entry.name.clone()
            };
            let drop_target = entry.path.clone();
            let drag_paths = selected_paths.clone();
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
                    .when(selected, |row| row.bg(theme::selected()))
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
                    .when(renaming, |row| {
                        row.child(div().flex().flex_col().flex_1().min_w_0().child(
                            Input::new(&self.modules.projects.rename_input).size(InputSize::Sm),
                        ))
                    })
                    .when(!renaming, |row| {
                        row.child(div().flex_1().truncate().child(entry.name.clone()))
                    })
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
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        let modifiers = event.modifiers();
                        if directory {
                            if let Some(w) = this.workspaces.get_mut(&SessionKey::project(&project))
                            {
                                if !w.state.expanded.remove(&file) {
                                    w.state.expanded.insert(file.clone());
                                }
                            }
                            this.changed(cx);
                        } else if modifiers.control || modifiers.shift {
                            let selected = &mut this.modules.projects.selected_files;
                            if !selected.remove(&file) {
                                selected.insert(file.clone());
                            }
                            cx.notify();
                        } else {
                            this.modules.projects.selected_files.clear();
                            this.modules.projects.selected_files.insert(file.clone());
                            if event.click_count() >= 2 {
                                this.modules.projects.renaming = Some(file.clone());
                                let name = file
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned();
                                this.modules.projects.rename_input.update(cx, |state, cx| {
                                    state.set_value(name, window, cx);
                                    state.focus_handle(cx).focus(window);
                                });
                                cx.notify();
                            } else {
                                this.open_file(file.clone(), cx);
                            }
                        }
                    }))
                    .on_drag(
                        DragItem {
                            kind: DragKind::Files(drag_paths),
                            name: drag_name,
                        },
                        |item, _, _, cx| cx.new(|_| item.clone()),
                    )
                    .when(directory, |row| {
                        row.on_drop(cx.listener(move |this, item: &DragItem, _, cx| {
                            if let DragKind::Files(paths) = &item.kind {
                                this.move_files_to(paths.clone(), drop_target.clone(), cx);
                            }
                        }))
                    })
                    .into_any_element(),
            );
            if directory && expanded {
                self.directory_rows(project_id, &entry.path, depth + 1, rows, cx);
            }
        }
    }

    fn collect_filter_entries(
        &self,
        project_id: &str,
        path: &Path,
        filter: &str,
        depth: usize,
        found: &mut Vec<crate::services::files::Entry>,
        visited: &mut usize,
    ) {
        if depth > 16 || *visited >= 5000 || found.len() >= 300 {
            return;
        }
        let _ = project_id;
        let Ok(directory) = std::fs::read_dir(path) else {
            return;
        };
        let mut entries: Vec<crate::services::files::Entry> = Vec::new();
        for entry in directory.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".git" || name == "target" || name == "node_modules" || name == ".cache" {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            entries.push(crate::services::files::Entry {
                path: entry.path(),
                name,
                directory: file_type.is_dir(),
            });
        }
        entries.sort_by(|left, right| {
            right
                .directory
                .cmp(&left.directory)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        for entry in entries.iter() {
            *visited += 1;
            if entry.name.to_lowercase().contains(filter) {
                found.push(entry.clone());
            }
            if entry.directory {
                self.collect_filter_entries(
                    project_id,
                    &entry.path,
                    filter,
                    depth + 1,
                    found,
                    visited,
                );
            }
            if found.len() >= 300 || *visited >= 5000 {
                break;
            }
        }
    }

    fn filter_row(
        &mut self,
        project_id: &str,
        root: &Path,
        entry: &crate::services::files::Entry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.modules.projects.selected_files.contains(&entry.path);
        let directory = entry.directory;
        let file = entry.path.clone();
        let menu_file = entry.path.clone();
        let project = project_id.to_string();
        let root = root.to_path_buf();
        let relative = entry
            .path
            .strip_prefix(&root)
            .unwrap_or(&entry.path)
            .display()
            .to_string();
        let parent = Path::new(&relative)
            .parent()
            .map(|parent| parent.display().to_string())
            .unwrap_or_default();
        div()
            .id(SharedString::from(format!(
                "search-file-{}",
                entry.path.display()
            )))
            .h(px(30.))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(|row| row.bg(theme::elevated()))
            .when(selected, |row| row.bg(theme::selected()))
            .child(file_icon(&entry.path, directory, false))
            .child(div().flex_none().child(entry.name.clone()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(10.))
                    .text_color(theme::muted())
                    .child(parent),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.file_menu(menu_file.clone(), directory, event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                if directory {
                    if let Some(workspace) = this.workspaces.get_mut(&SessionKey::project(&project))
                    {
                        expand_ancestors(workspace, &root, &file);
                    }
                    this.modules.projects.file_filter.update(cx, |state, cx| {
                        state.set_value("", window, cx);
                    });
                    this.changed(cx);
                } else {
                    this.modules.projects.selected_files.clear();
                    this.modules.projects.selected_files.insert(file.clone());
                    this.open_file(file.clone(), cx);
                }
            }))
            .into_any_element()
    }
}

fn expand_ancestors(
    workspace: &mut crate::workspace::session::Workspace,
    root: &Path,
    path: &Path,
) {
    if path.is_dir() {
        workspace.state.expanded.insert(path.to_path_buf());
    }
    let mut current = path.parent();
    while let Some(parent) = current {
        if parent == root {
            break;
        }
        workspace.state.expanded.insert(parent.to_path_buf());
        current = parent.parent();
    }
}
