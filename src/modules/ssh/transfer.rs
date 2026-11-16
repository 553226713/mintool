use super::{browser::RemoteBrowser, local::LocalBrowser};
use crate::app::MintoolApp;
use crate::ui::common::drag_item::{DragItem, DragKind};
use crate::ui::{file_icon::file_icon, format::format_size, theme};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
    input::{Input, InputSize},
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::{Path, PathBuf};

impl MintoolApp {
    pub(crate) fn render_ssh_explorer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(remote) = self.modules.ssh.panel().remote.clone() else {
            return div()
                .size_full()
                .p_4()
                .text_color(theme::muted())
                .child("打开 SSH 连接后浏览远程文件")
                .into_any_element();
        };
        if !remote.entries.contains_key(&remote.current)
            && !remote.loading.contains(&remote.current)
            && remote.error.is_none()
        {
            self.start_remote_load(remote.current.clone(), cx);
        }
        let remote = self.modules.ssh.panel().remote.clone().unwrap_or(remote);
        let local_visible = self.modules.ssh.panel().local_visible;
        let local = local_visible.then(|| {
            self.modules
                .ssh
                .panel()
                .local
                .clone()
                .unwrap_or_else(|| LocalBrowser::new(LocalBrowser::home()))
        });
        let connection_name = self
            .connection_of(&remote.connection)
            .map(|connection| connection.name)
            .unwrap_or_else(|| "SSH".into());
        div()
            .id("ssh-transfer")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::panel())
            .child(
                div()
                    .h(px(38.))
                    .px_3()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("文件传输 · {connection_name}")),
                    )
                    .child(
                        Button::new(
                            "toggle-local-transfer",
                            if local_visible {
                                "隐藏本地"
                            } else {
                                "显示本地"
                            },
                        )
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .icon("folder-closed")
                        .tooltip(if local_visible {
                            "隐藏本地文件系统"
                        } else {
                            "显示本地文件系统"
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_local_panel(cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(local_visible, |columns| {
                        columns
                            .child(self.render_local_column(
                                local.as_ref().expect("visible local browser"),
                                cx,
                            ))
                            .child(div().w(px(1.)).h_full().flex_none().bg(theme::border()))
                    })
                    .child(self.render_remote_column(&remote, cx)),
            )
            .child(self.render_ssh_queue(cx))
            .into_any_element()
    }

    fn render_local_column(&self, browser: &LocalBrowser, cx: &mut Context<Self>) -> AnyElement {
        let mut rows = Vec::new();
        if let Some(error) = &browser.error {
            rows.push(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.))
                    .text_color(theme::danger())
                    .child(error.clone())
                    .into_any_element(),
            );
        }
        for entry in &self.ssh_local_entries() {
            let selected = browser.selected_paths.contains(&entry.path);
            let path = entry.path.clone();
            let directory = entry.directory;
            let menu_entry = entry.clone();
            let row_path = path.clone();
            rows.push(
                entry_row(
                    SharedString::from(format!("local-entry-{}", entry.path.display())),
                    &entry.name,
                    directory,
                    entry.size,
                    selected,
                )
                .h(px(42.))
                .when(directory, |row| {
                    let destination = entry.path.clone();
                    let owner = self.modules.ssh.active_panel.clone().unwrap();
                    row.on_drop(cx.listener(move |app, item: &DragItem, _, cx| {
                        if let DragKind::SshRemoteFiles {
                            owner: source,
                            paths,
                        } = &item.kind
                        {
                            app.activate_ssh_panel(source.clone(), cx);
                            app.enqueue_downloads(paths.clone(), destination.clone(), cx);
                            app.activate_ssh_panel(owner.clone(), cx);
                            cx.stop_propagation();
                        }
                    }))
                })
                .relative()
                .child(self.ssh_row_measure(false, entry.path.to_string_lossy().into_owned(), cx))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_drag(
                    DragItem {
                        kind: DragKind::Files(if selected {
                            browser.selected_paths.iter().cloned().collect()
                        } else {
                            vec![entry.path.clone()]
                        }),
                        name: entry.name.clone(),
                    },
                    |item, _, _, cx| cx.new(|_| item.clone()),
                )
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .pl(px(22.))
                        .flex_none()
                        .text_size(px(9.))
                        .text_color(theme::muted())
                        .child(format!(
                            "{} · {} · {}",
                            crate::ui::common::format::format_timestamp(entry.modified),
                            entry.permissions,
                            entry.owner
                        )),
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() >= 2 && directory {
                        this.navigate_local(path.clone(), cx);
                    } else {
                        this.select_local_with(path.clone(), event.modifiers(), cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        if !this
                            .modules
                            .ssh
                            .panel()
                            .local
                            .as_ref()
                            .is_some_and(|browser| browser.selected_paths.contains(&row_path))
                        {
                            this.select_local(row_path.clone(), cx);
                        }
                        this.ssh_local_menu(menu_entry.clone(), event.position, cx);
                        cx.stop_propagation();
                    }),
                )
                .into_any_element(),
            );
        }
        let owner = self.modules.ssh.active_panel.clone().unwrap();
        let drop_destination = browser.current.clone();
        let upload_enabled =
            !browser.selected_paths.is_empty() && self.modules.ssh.panel().remote.is_some();
        div()
            .id("local-file-column")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .capture_any_mouse_down(
                cx.listener(|app, _, _, _| app.modules.ssh.panel_mut().navigation_remote = false),
            )
            .child(column_header(
                "本地",
                vec![
                    Button::new("local-back", "←")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("后退")
                        .on_click(cx.listener(|this, _, _, cx| this.local_back(cx))),
                    Button::new("local-forward", "→")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("前进")
                        .disabled(browser.forward.is_empty())
                        .on_click(cx.listener(|app, _, _, cx| app.local_forward(cx))),
                    Button::new("local-up", "↑")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("上一级")
                        .on_click(cx.listener(|this, _, _, cx| this.local_up(cx))),
                    Button::new("local-home", "⌂")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("主目录")
                        .on_click(cx.listener(|this, _, _, cx| this.local_home(cx))),
                    Button::new("local-refresh", "⟳")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("刷新")
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_local(cx))),
                ],
            ))
            .child(self.ssh_listing_options(cx))
            .child(self.local_breadcrumbs(&browser.current, cx))
            .child(self.ssh_address_bar(false))
            .child(self.ssh_bookmarks(false, cx))
            .child(
                div()
                    .id("local-transfer-list")
                    .relative()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|app, event: &MouseDownEvent, _, cx| {
                            app.ssh_box_start(false, event.position, cx)
                        }),
                    )
                    .on_mouse_move(cx.listener(|app, event: &MouseMoveEvent, _, cx| {
                        app.ssh_box_move(false, event.position, cx)
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|app, _, _, cx| {
                            app.modules.ssh.panel_mut().selection_drag = None;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|app, _, _, cx| {
                            app.modules.ssh.panel_mut().selection_drag = None;
                            cx.notify();
                        }),
                    )
                    .child(self.ssh_list_measure(false, cx))
                    .child(self.ssh_box_overlay(false))
                    .on_drop(cx.listener(move |app, item: &DragItem, _, cx| {
                        if let DragKind::SshRemoteFiles {
                            owner: source,
                            paths,
                        } = &item.kind
                        {
                            app.activate_ssh_panel(source.clone(), cx);
                            app.enqueue_downloads(paths.clone(), drop_destination.clone(), cx);
                            app.activate_ssh_panel(owner.clone(), cx);
                        }
                    }))
                    .flex_1()
                    .min_h_0()
                    .py_1()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(
                div().p_2().flex_none().child(
                    Button::new("upload-selected", "上传 →")
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .w_full()
                        .disabled(!upload_enabled)
                        .on_click(cx.listener(|this, _, _, cx| this.upload_selected_local(cx))),
                ),
            )
            .into_any_element()
    }

    fn render_remote_column(&self, browser: &RemoteBrowser, cx: &mut Context<Self>) -> AnyElement {
        let filter = self
            .modules
            .ssh
            .panel()
            .remote_filter
            .read(cx)
            .content()
            .trim()
            .to_lowercase();
        let mut rows = Vec::new();
        if browser.current != "/" {
            rows.push(
                entry_row("remote-parent".into(), "..  上一级", true, 0, false)
                    .on_click(cx.listener(|this, _, _, cx| this.remote_up(cx)))
                    .into_any_element(),
            );
        }
        if let Some(error) = &browser.error {
            rows.push(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.))
                    .text_color(theme::danger())
                    .child(error.clone())
                    .into_any_element(),
            );
        }
        if browser.loading.contains(&browser.current)
            && !browser.entries.contains_key(&browser.current)
        {
            rows.push(
                div()
                    .px_2()
                    .py_2()
                    .text_size(px(10.))
                    .text_color(theme::muted())
                    .child("正在读取远程目录…")
                    .into_any_element(),
            );
        }
        for entry in self
            .ssh_remote_entries()
            .iter()
            .filter(|entry| filter.is_empty() || entry.name.to_lowercase().contains(&filter))
        {
            let selected = browser.selected_paths.contains(&entry.path);
            let path = entry.path.clone();
            let directory = entry.directory;
            let menu_entry = entry.clone();
            let row_path = path.clone();
            rows.push(
                entry_row(
                    SharedString::from(format!("remote-entry-{}", entry.path)),
                    &entry.name,
                    directory,
                    entry.size,
                    selected,
                )
                .h(px(42.))
                .when(directory, |row| {
                    let destination = entry.path.clone();
                    let external_destination = destination.clone();
                    let owner = self.modules.ssh.active_panel.clone().unwrap();
                    let external_owner = owner.clone();
                    row.on_drop(cx.listener(move |app, item: &DragItem, _, cx| {
                        if let DragKind::Files(paths) = &item.kind {
                            app.activate_ssh_panel(owner.clone(), cx);
                            app.enqueue_uploads(paths.clone(), destination.clone(), cx);
                            cx.stop_propagation();
                        }
                    }))
                    .on_drop(cx.listener(
                        move |app, paths: &ExternalPaths, _, cx| {
                            app.activate_ssh_panel(external_owner.clone(), cx);
                            app.enqueue_uploads(
                                paths.paths().to_vec(),
                                external_destination.clone(),
                                cx,
                            );
                            cx.stop_propagation();
                        },
                    ))
                })
                .relative()
                .child(self.ssh_row_measure(true, entry.path.clone(), cx))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_drag(
                    DragItem {
                        kind: DragKind::SshRemoteFiles {
                            owner: self.modules.ssh.active_panel.clone().unwrap(),
                            paths: if selected {
                                self.ssh_remote_entries()
                                    .iter()
                                    .filter(|entry| browser.selected_paths.contains(&entry.path))
                                    .map(|entry| (entry.path.clone(), entry.directory))
                                    .collect()
                            } else {
                                vec![(entry.path.clone(), entry.directory)]
                            },
                        },
                        name: entry.name.clone(),
                    },
                    |item, _, _, cx| cx.new(|_| item.clone()),
                )
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .pl(px(22.))
                        .flex_none()
                        .text_size(px(9.))
                        .text_color(theme::muted())
                        .child(format!(
                            "{} · {} · {}:{}",
                            entry.modified, entry.permissions, entry.owner, entry.group
                        )),
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() >= 2 {
                        if directory {
                            this.navigate_remote(path.clone(), cx);
                        } else {
                            this.open_remote_file(path.clone(), cx);
                        }
                    } else {
                        this.select_remote_with(path.clone(), event.modifiers(), cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        if !this
                            .modules
                            .ssh
                            .panel()
                            .remote
                            .as_ref()
                            .is_some_and(|browser| browser.selected_paths.contains(&row_path))
                        {
                            this.select_remote(row_path.clone(), cx);
                        }
                        this.ssh_remote_menu(menu_entry.clone(), event.position, cx);
                        cx.stop_propagation();
                    }),
                )
                .into_any_element(),
            );
        }
        let owner = self.modules.ssh.active_panel.clone().unwrap();
        let external_owner = owner.clone();
        let drop_destination = browser.current.clone();
        let external_destination = drop_destination.clone();
        let download_enabled = !browser.selected_paths.is_empty();
        div()
            .id("remote-file-column")
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .capture_any_mouse_down(
                cx.listener(|app, _, _, _| app.modules.ssh.panel_mut().navigation_remote = true),
            )
            .child(column_header(
                "远程",
                vec![
                    Button::new("remote-back", "←")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("后退")
                        .disabled(browser.back.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| this.remote_back(cx))),
                    Button::new("remote-forward", "→")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("前进")
                        .disabled(browser.forward.is_empty())
                        .on_click(cx.listener(|app, _, _, cx| app.remote_forward(cx))),
                    Button::new("remote-up", "上一级")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("上一级")
                        .disabled(browser.current == "/" || browser.current.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| this.remote_up(cx))),
                    Button::new("remote-home", "⌂")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("主目录")
                        .on_click(cx.listener(|this, _, _, cx| this.remote_home(cx))),
                    Button::new("remote-refresh", "⟳")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("刷新")
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_remote_current(cx))),
                ],
            ))
            .child(
                div()
                    .px_3()
                    .py_1()
                    .flex_none()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        Input::new(&self.modules.ssh.panel().remote_filter)
                            .map(|mut input| {
                                input.style().align_items = Some(AlignItems::Stretch);
                                input
                            })
                            .w_full()
                            .min_w(px(160.))
                            .size(InputSize::Sm)
                            .placeholder("筛选远端文件…")
                            .prefix(Icon::new("search").size(px(13.)).color(theme::muted())),
                    ),
            )
            .child(self.ssh_listing_options(cx))
            .child(self.remote_breadcrumbs(&browser.current, cx))
            .child(self.ssh_address_bar(true))
            .child(self.ssh_bookmarks(true, cx))
            .child(
                div()
                    .id("remote-transfer-list")
                    .relative()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|app, event: &MouseDownEvent, _, cx| {
                            app.ssh_box_start(true, event.position, cx)
                        }),
                    )
                    .on_mouse_move(cx.listener(|app, event: &MouseMoveEvent, _, cx| {
                        app.ssh_box_move(true, event.position, cx)
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|app, _, _, cx| {
                            app.modules.ssh.panel_mut().selection_drag = None;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|app, _, _, cx| {
                            app.modules.ssh.panel_mut().selection_drag = None;
                            cx.notify();
                        }),
                    )
                    .child(self.ssh_list_measure(true, cx))
                    .child(self.ssh_box_overlay(true))
                    .on_drop(cx.listener(move |app, item: &DragItem, _, cx| {
                        app.activate_ssh_panel(owner.clone(), cx);
                        if let DragKind::Files(paths) = &item.kind {
                            app.enqueue_uploads(paths.clone(), drop_destination.clone(), cx);
                        }
                    }))
                    .on_drop(cx.listener(move |app, paths: &ExternalPaths, _, cx| {
                        app.activate_ssh_panel(external_owner.clone(), cx);
                        app.enqueue_uploads(
                            paths.paths().to_vec(),
                            external_destination.clone(),
                            cx,
                        );
                    }))
                    .flex_1()
                    .min_h_0()
                    .py_1()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(
                div().p_2().flex_none().child(
                    Button::new("download-selected", "← 下载")
                        .variant(ButtonVariant::Outline)
                        .size(ButtonSize::Sm)
                        .w_full()
                        .disabled(!download_enabled)
                        .on_click(cx.listener(|this, _, _, cx| this.download_selected_remote(cx))),
                ),
            )
            .into_any_element()
    }

    fn local_breadcrumbs(&self, path: &Path, cx: &mut Context<Self>) -> AnyElement {
        let home = LocalBrowser::home();
        let mut segments: Vec<(String, PathBuf)> = Vec::new();
        if let Ok(relative) = path.strip_prefix(&home) {
            segments.push(("~".to_string(), home.clone()));
            let mut cumulative = home.clone();
            for component in relative.components() {
                cumulative.push(component);
                segments.push((
                    component.as_os_str().to_string_lossy().into_owned(),
                    cumulative.clone(),
                ));
            }
        } else {
            segments.push(("/".to_string(), PathBuf::from("/")));
            let mut cumulative = PathBuf::from("/");
            for component in path.components().skip(1) {
                cumulative.push(component);
                segments.push((
                    component.as_os_str().to_string_lossy().into_owned(),
                    cumulative.clone(),
                ));
            }
        }
        let mut row = breadcrumb_bar();
        for (index, (label, target)) in segments.into_iter().enumerate() {
            if index > 0 {
                row = row.child(breadcrumb_separator());
            }
            row = row.child(
                breadcrumb_chip(
                    SharedString::from(format!("local-crumb-{}", target.display())),
                    label,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.navigate_local(target.clone(), cx)),
                ),
            );
        }
        row.into_any_element()
    }

    fn remote_breadcrumbs(&self, current: &str, cx: &mut Context<Self>) -> AnyElement {
        let absolute = current.starts_with('/');
        let mut row = breadcrumb_bar().child(
            breadcrumb_chip(
                SharedString::from("remote-crumb-root"),
                if absolute { "/" } else { "~" }.to_string(),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if absolute {
                    this.navigate_remote("/".into(), cx);
                } else {
                    this.remote_home(cx);
                }
            })),
        );
        let mut cumulative = if absolute {
            "/".to_string()
        } else {
            String::new()
        };
        for segment in current.split('/').filter(|segment| !segment.is_empty()) {
            if !cumulative.is_empty() && cumulative != "/" {
                cumulative.push('/');
            }
            cumulative.push_str(segment);
            let target = cumulative.clone();
            row = row.child(breadcrumb_separator()).child(
                breadcrumb_chip(
                    SharedString::from(format!("remote-crumb-{target}")),
                    segment.to_string(),
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.navigate_remote(target.clone(), cx)),
                ),
            );
        }
        row.into_any_element()
    }
}

fn entry_row(
    id: SharedString,
    name: &str,
    directory: bool,
    size: u64,
    selected: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(24.))
        .px_2()
        .flex()
        .flex_col()
        .rounded_sm()
        .cursor_pointer()
        .when(selected, |row| row.bg(theme::selected()))
        .hover(|row| row.bg(theme::elevated()))
        .child(
            div()
                .w_full()
                .h(px(24.))
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(file_icon(Path::new(name), directory, false))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .child(name.to_string()),
                )
                .when(!directory, |row| {
                    row.child(
                        div()
                            .flex_none()
                            .text_size(px(9.))
                            .text_color(theme::muted())
                            .child(format_size(size)),
                    )
                }),
        )
}

fn column_header(title: &'static str, buttons: Vec<Button>) -> Div {
    div()
        .h(px(30.))
        .px_2()
        .flex_none()
        .flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .flex_1()
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::muted())
                .child(title),
        )
        .children(buttons)
}

fn breadcrumb_bar() -> Div {
    div()
        .h(px(26.))
        .px_2()
        .flex_none()
        .flex()
        .items_center()
        .overflow_hidden()
        .border_b_1()
        .border_color(theme::border())
        .text_size(px(10.))
        .text_color(theme::muted())
}

fn breadcrumb_chip(id: SharedString, label: String) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .cursor_pointer()
        .hover(|style| style.text_color(theme::text()))
        .child(label)
}

fn breadcrumb_separator() -> Div {
    div().flex_none().px_1().child("/")
}
