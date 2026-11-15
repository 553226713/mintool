use super::{browser::RemoteBrowser, explorer::selected_remote_file, local::LocalBrowser};
use crate::app::MintoolApp;
use crate::ui::{file_icon::file_icon, format::format_size, theme};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    input::Input,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::{
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

impl MintoolApp {
    pub(crate) fn render_ssh_explorer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(remote) = self.modules.ssh.remote.clone() else {
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
        let remote = self.modules.ssh.remote.clone().unwrap_or(remote);
        let local_visible = self.modules.ssh.local_visible;
        let local = local_visible.then(|| {
            self.modules
                .ssh
                .local
                .clone()
                .unwrap_or_else(|| LocalBrowser::new(LocalBrowser::home()))
        });
        let connection_name = self
            .connection_of(&remote.connection)
            .map(|connection| connection.name)
            .unwrap_or_else(|| "SSH".into());
        let status = self.modules.ssh.transfer_status.clone();
        let sync_terminal = self.modules.ssh.sync_terminal;
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
                    .when_some(status, |header, status| {
                        let color = if status.cancel.is_some() {
                            theme::warning()
                        } else if status.success {
                            theme::success()
                        } else {
                            theme::danger()
                        };
                        let cancel = status.cancel.clone();
                        let percent = status.percent();
                        header
                            .child(
                                div()
                                    .flex_none()
                                    .max_w(px(180.))
                                    .truncate()
                                    .text_size(px(10.))
                                    .text_color(color)
                                    .child(status.message),
                            )
                            .when_some(percent, |header, percent| {
                                header
                                    .child(
                                        div()
                                            .w(px(80.))
                                            .h(px(4.))
                                            .rounded_full()
                                            .bg(theme::border())
                                            .child(
                                                div()
                                                    .h_full()
                                                    .rounded_full()
                                                    .bg(theme::accent())
                                                    .w(px(80. * percent as f32 / 100.)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_none()
                                            .text_size(px(10.))
                                            .text_color(theme::accent())
                                            .child(format!("{percent}%")),
                                    )
                            })
                            .when_some(cancel, |header, cancel| {
                                header.child(
                                    Button::new("cancel-transfer", "取消")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm)
                                        .tooltip("取消当前传输")
                                        .on_click(move |_, _, _| {
                                            cancel.store(true, Ordering::Relaxed);
                                        }),
                                )
                            })
                    })
                    .child(
                        Button::new("toggle-ssh-sync", "同步终端")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .selected(sync_terminal)
                            .tooltip("浏览远端目录时在 SSH 终端中执行 cd")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.modules.ssh.sync_terminal = !this.modules.ssh.sync_terminal;
                                cx.notify();
                            })),
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
        for entry in &browser.entries {
            let selected = browser.selected.as_ref() == Some(&entry.path);
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
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() >= 2 && directory {
                        this.navigate_local(path.clone(), cx);
                    } else {
                        this.select_local(path.clone(), cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.select_local(row_path.clone(), cx);
                        this.ssh_local_menu(menu_entry.clone(), event.position, cx);
                        cx.stop_propagation();
                    }),
                )
                .into_any_element(),
            );
        }
        let upload_enabled = browser.selected_file().is_some() && self.modules.ssh.remote.is_some();
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(column_header(
                "本地",
                vec![
                    Button::new("local-back", "←")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("后退")
                        .on_click(cx.listener(|this, _, _, cx| this.local_back(cx))),
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
            .child(self.local_breadcrumbs(&browser.current, cx))
            .child(
                div()
                    .id("local-transfer-list")
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
            .remote_filter
            .read(cx)
            .content()
            .trim()
            .to_lowercase();
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
        for entry in browser
            .entries
            .get(&browser.current)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .filter(|entry| filter.is_empty() || entry.name.to_lowercase().contains(&filter))
        {
            let selected = browser.selected.as_deref() == Some(entry.path.as_str());
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
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    if event.click_count() >= 2 {
                        if directory {
                            this.navigate_remote(path.clone(), cx);
                        } else {
                            this.open_remote_file(path.clone(), cx);
                        }
                    } else {
                        this.select_remote(path.clone(), cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.select_remote(row_path.clone(), cx);
                        this.ssh_remote_menu(menu_entry.clone(), event.position, cx);
                        cx.stop_propagation();
                    }),
                )
                .into_any_element(),
            );
        }
        let download_enabled = selected_remote_file(browser).is_some();
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(column_header(
                "远程",
                vec![
                    Button::new("remote-back", "←")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("后退")
                        .on_click(cx.listener(|this, _, _, cx| this.remote_back(cx))),
                    Button::new("remote-up", "↑")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Sm)
                        .tooltip("上一级")
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
                        Input::new(&self.modules.ssh.remote_filter)
                            .map(|mut input| {
                                input.style().align_items = Some(AlignItems::Stretch);
                                input
                            })
                            .w_full(),
                    ),
            )
            .child(self.remote_breadcrumbs(&browser.current, cx))
            .child(
                div()
                    .id("remote-transfer-list")
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
        let mut row = breadcrumb_bar().child(
            breadcrumb_chip(SharedString::from("remote-crumb-home"), "~".to_string())
                .on_click(cx.listener(|this, _, _, cx| this.remote_home(cx))),
        );
        let mut cumulative = String::new();
        for segment in current.split('/').filter(|segment| !segment.is_empty()) {
            if !cumulative.is_empty() {
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
        .items_center()
        .gap_2()
        .rounded_sm()
        .cursor_pointer()
        .when(selected, |row| row.bg(theme::selected()))
        .hover(|row| row.bg(theme::elevated()))
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
        })
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
