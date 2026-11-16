use super::dialogs::SshDialog;
use super::{
    local::{LocalBrowser, LocalEntry},
    model::SshConnection,
    sftp::{self, RemoteEntry},
};
use crate::app::MintoolApp;
use crate::modules::ssh::path::parent_of;
use crate::modules::SessionKey;
use crate::ui::dialogs::Dialog;
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;
use std::path::{Path, PathBuf};

impl MintoolApp {
    pub(crate) fn ensure_remote_browser(&mut self) {
        let Some(connection) = self.modules.ssh.opened.clone() else {
            return;
        };
        let key = SessionKey::ssh(connection);
        let Some(workspace) = self.workspaces.get(&key) else {
            return;
        };
        let pane = workspace.state.focused;
        if self.modules.ssh.panels.contains_key(&(key.clone(), pane)) {
            self.modules.ssh.active_panel = Some((key, pane));
        }
    }

    pub(crate) fn start_remote_load(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(browser) = &self.modules.ssh.panel().remote else {
            return;
        };
        if browser.entries.contains_key(&path) || browser.loading.contains(&path) {
            return;
        }
        let browser_connection = browser.connection.clone();
        let Some(connection) = self
            .modules
            .ssh
            .registry
            .connections
            .iter()
            .find(|connection| connection.id == browser_connection)
            .cloned()
        else {
            return;
        };
        if let Some(browser) = &mut self.modules.ssh.panel_mut().remote {
            browser.loading.insert(path.clone());
            browser.error = None;
        }
        let executor = cx.background_executor().clone();
        let owner = self.modules.ssh.active_panel.clone();
        cx.spawn(async move |this, cx| {
            let load_path = path.clone();
            let result = executor
                .spawn(async move { sftp::read_directory(&connection, &load_path) })
                .await;
            let _ = this.update(
                cx,
                super::scope::scoped(owner, |this, cx| {
                    if let Some(browser) = &mut this.modules.ssh.panel_mut().remote {
                        if browser.connection != browser_connection {
                            return;
                        }
                        browser.loading.remove(&path);
                        match result {
                            Ok(directory) => {
                                browser.home = Some(directory.home);
                                if browser.current == path {
                                    browser.current = directory.path.clone();
                                }
                                browser.entries.insert(directory.path, directory.entries);
                            }
                            Err(error) if browser.current == path => {
                                browser.error = Some(format!("{error:#}"))
                            }
                            Err(_) => {}
                        }
                    }
                    cx.notify();
                }),
            );
        })
        .detach();
    }

    pub(crate) fn navigate_remote(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(browser) = &mut self.modules.ssh.panel_mut().remote else {
            return;
        };
        if browser.current != path {
            browser.back.push(browser.current.clone());
            browser.forward.clear();
        }
        browser.current = path.clone();
        browser.selected = None;
        browser.selected_paths.clear();
        browser.error = None;
        self.start_remote_load(path, cx);
        self.changed(cx);
    }

    pub(crate) fn remote_back(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = &mut self.modules.ssh.panel_mut().remote else {
            return;
        };
        let Some(previous) = browser.back.pop() else {
            return;
        };
        browser.forward.push(browser.current.clone());
        browser.current = previous.clone();
        browser.selected = None;
        browser.selected_paths.clear();
        browser.error = None;
        self.start_remote_load(previous, cx);
        cx.notify();
    }

    pub(crate) fn remote_forward(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = &mut self.modules.ssh.panel_mut().remote else {
            return;
        };
        let Some(next) = browser.forward.pop() else {
            return;
        };
        browser.back.push(browser.current.clone());
        browser.current = next.clone();
        browser.error = None;
        browser.selected = None;
        browser.selected_paths.clear();
        self.start_remote_load(next, cx);
        self.changed(cx);
    }
    pub(crate) fn local_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.panel_mut().local {
            browser.forward();
        }
        self.changed(cx);
    }
    pub(crate) fn remote_up(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = &self.modules.ssh.panel().remote else {
            return;
        };
        let parent = parent_of(&browser.current).to_string();
        self.navigate_remote(parent, cx);
    }

    pub(crate) fn remote_home(&mut self, cx: &mut Context<Self>) {
        let home = self
            .modules
            .ssh
            .panel()
            .remote
            .as_ref()
            .and_then(|browser| browser.home.clone())
            .unwrap_or_default();
        self.navigate_remote(home, cx);
    }

    pub(crate) fn refresh_remote_current(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = &mut self.modules.ssh.panel_mut().remote else {
            return;
        };
        let current = browser.current.clone();
        browser.entries.remove(&current);
        browser.loading.remove(&current);
        browser.selected = None;
        browser.selected_paths.clear();
        self.start_remote_load(current, cx);
        cx.notify();
    }

    pub(crate) fn select_remote(&mut self, path: String, cx: &mut Context<Self>) {
        self.select_remote_with(path, Modifiers::default(), cx);
    }
    pub(crate) fn select_remote_with(
        &mut self,
        path: String,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let filter = self
            .modules
            .ssh
            .panel()
            .remote_filter
            .read(cx)
            .content()
            .to_lowercase();
        let ordered = self
            .ssh_remote_entries()
            .into_iter()
            .filter(|entry| filter.is_empty() || entry.name.to_lowercase().contains(&filter))
            .collect::<Vec<_>>();
        if let Some(browser) = &mut self.modules.ssh.panel_mut().remote {
            crate::modules::common::selection::select(
                &ordered
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect::<Vec<_>>(),
                &mut browser.selected_paths,
                browser.selected.as_ref(),
                path.clone(),
                modifiers.control || modifiers.platform,
                modifiers.shift,
            );
            if !modifiers.shift {
                browser.selected = Some(path);
            }
        }
        cx.notify();
    }
    pub(crate) fn select_local(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.select_local_with(path, Modifiers::default(), cx);
    }
    pub(crate) fn select_local_with(
        &mut self,
        path: PathBuf,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let ordered = self.ssh_local_entries();
        if let Some(browser) = &mut self.modules.ssh.panel_mut().local {
            crate::modules::common::selection::select(
                &ordered
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect::<Vec<_>>(),
                &mut browser.selected_paths,
                browser.selected.as_ref(),
                path.clone(),
                modifiers.control || modifiers.platform,
                modifiers.shift,
            );
            if !modifiers.shift {
                browser.selected = Some(path);
            }
        }
        cx.notify();
    }

    pub(crate) fn navigate_local(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.panel_mut().local {
            browser.navigate(path);
        }
        self.changed(cx);
    }

    pub(crate) fn local_back(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.panel_mut().local {
            browser.back();
        }
        self.changed(cx);
    }

    pub(crate) fn local_up(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.panel_mut().local {
            browser.up();
        }
        self.changed(cx);
    }

    pub(crate) fn local_home(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.panel_mut().local {
            browser.navigate(LocalBrowser::home());
        }
        self.changed(cx);
    }

    pub(crate) fn refresh_local(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.panel_mut().local {
            browser.refresh();
        }
        self.changed(cx);
    }

    pub(crate) fn toggle_local_panel(&mut self, cx: &mut Context<Self>) {
        self.modules.ssh.panel_mut().local_visible = !self.modules.ssh.panel().local_visible;
        if self.modules.ssh.panel().local_visible && self.modules.ssh.panel().local.is_none() {
            self.modules.ssh.panel_mut().local = Some(LocalBrowser::new(LocalBrowser::home()));
        }
        self.changed(cx);
    }

    pub(crate) fn open_remote_file(&mut self, remote: String, cx: &mut Context<Self>) {
        let Some(browser) = &self.modules.ssh.panel().remote else {
            return;
        };
        let key = SessionKey::ssh(browser.connection.clone());
        let Some(workspace) = self.workspaces.get_mut(&key) else {
            return;
        };
        let anchor = if workspace.state.file_pane(workspace.state.focused).is_some() {
            workspace.state.focused
        } else {
            workspace.state.layout.first_id()
        };
        let id = workspace.state.next_id;
        workspace.state.next_id = workspace.state.next_id.saturating_add(1);
        workspace.state.remote_files.insert(id, remote);
        self.dock_file_tabs.push((key, anchor, id));
        self.changed(cx);
    }

    pub(crate) fn download_remote_file(&mut self, remote: String, cx: &mut Context<Self>) {
        self.enqueue_downloads(vec![(remote, false)], default_download_directory(), cx);
    }
    pub(crate) fn upload_selected_local(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = &self.modules.ssh.panel().remote else {
            return;
        };
        if !remote.entries.contains_key(&remote.current) {
            self.error = Some("远程目录尚未加载完成".into());
            cx.notify();
            return;
        }
        let destination = remote.current.clone();
        let paths = self
            .modules
            .ssh
            .panel()
            .local
            .as_ref()
            .map(|browser| {
                if browser.selected_paths.is_empty() {
                    browser.selected.iter().cloned().collect()
                } else {
                    browser.selected_paths.iter().cloned().collect()
                }
            })
            .unwrap_or_default();
        self.enqueue_uploads(paths, destination, cx);
    }
    pub(crate) fn download_selected_remote(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = &self.modules.ssh.panel().remote else {
            return;
        };
        let paths = remote
            .entries
            .get(&remote.current)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| {
                        remote.selected_paths.contains(&entry.path)
                            || (remote.selected_paths.is_empty()
                                && remote.selected.as_ref() == Some(&entry.path))
                    })
                    .map(|entry| (entry.path.clone(), entry.directory))
                    .collect()
            })
            .unwrap_or_default();
        let destination = self
            .modules
            .ssh
            .panel()
            .local
            .as_ref()
            .map(|browser| browser.current.clone())
            .unwrap_or_else(default_download_directory);
        self.enqueue_downloads(paths, destination, cx);
    }

    pub(crate) fn enter_remote_directory_in_terminal(
        &mut self,
        path: String,
        cx: &mut Context<Self>,
    ) {
        let command = super::path::directory_command(&path);
        cx.write_to_clipboard(ClipboardItem::new_string(command));
        self.error = Some("目录命令已复制；回到 Shell 提示符后粘贴执行".into());
        cx.notify();
    }

    pub(super) fn ssh_remote_menu(
        &mut self,
        entry: RemoteEntry,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let path = entry.path.clone();
        let mut items = Vec::new();
        if self
            .modules
            .ssh
            .panel()
            .remote
            .as_ref()
            .is_some_and(|browser| browser.selected_paths.len() > 1)
        {
            items.push(self.menu_action(
                "remote-download-selection",
                "下载所有选中项",
                cx,
                |app, _, cx| app.download_selected_remote(cx),
            ));
        }
        if entry.directory {
            let enter_path = path.clone();
            items.push(
                self.menu_action("remote-enter", "进入目录", cx, move |this, _, cx| {
                    this.navigate_remote(enter_path.clone(), cx)
                }),
            );
            let terminal_path = path.clone();
            items.push(self.menu_action(
                "remote-terminal-here",
                "复制进入目录命令",
                cx,
                move |this, _, cx| {
                    this.enter_remote_directory_in_terminal(terminal_path.clone(), cx)
                },
            ));
            let download_dir = entry.clone();
            items.push(self.menu_action(
                "remote-download-dir",
                "下载此目录",
                cx,
                move |this, _, cx| {
                    this.download_remote_directory(download_dir.clone(), cx);
                },
            ));
        } else {
            let open_path = path.clone();
            items.push(
                self.menu_action("remote-open", "打开预览", cx, move |this, _, cx| {
                    this.open_remote_file(open_path.clone(), cx)
                }),
            );
            let edit_entry = entry.clone();
            items.push(self.menu_action(
                "remote-edit",
                "编辑远程文件…",
                cx,
                move |this, _, cx| this.open_remote_edit(edit_entry.clone(), cx),
            ));
            let download_path = path.clone();
            items.push(self.menu_action(
                "remote-download",
                "下载到本地下载目录",
                cx,
                move |this, _, cx| this.download_remote_file(download_path.clone(), cx),
            ));
            let select_path = path.clone();
            items.push(self.menu_action(
                "remote-download-here",
                "下载到本地目录",
                cx,
                move |this, _, cx| {
                    if let Some(browser) = &mut this.modules.ssh.panel_mut().remote {
                        browser.selected_paths.clear();
                        browser.selected_paths.insert(select_path.clone());
                        browser.selected = Some(select_path.clone());
                    }
                    this.download_selected_remote(cx);
                },
            ));
        }
        if entry.directory {
            let parent = path.clone();
            items.push(self.menu_action(
                "remote-mkdir",
                "新建文件夹…",
                cx,
                move |this, window, cx| {
                    this.show_dialog(
                        Dialog::Ssh(SshDialog::RemoteMkdir {
                            parent: parent.clone(),
                        }),
                        window,
                        cx,
                    )
                },
            ));
        }
        items.push(MenuItem::separator());
        let rename_path = path.clone();
        let rename_name = entry.name.clone();
        items.push(self.menu_action(
            "remote-rename",
            "重命名…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::RemoteRename {
                        path: rename_path.clone(),
                        name: rename_name.clone(),
                    }),
                    window,
                    cx,
                )
            },
        ));
        let delete_path = path.clone();
        let delete_name = entry.name.clone();
        let directory = entry.directory;
        items.push(
            self.menu_action("remote-delete", "删除…", cx, move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::RemoteDelete {
                        path: delete_path.clone(),
                        name: delete_name.clone(),
                        directory,
                    }),
                    window,
                    cx,
                )
            }),
        );
        let chmod_path = path.clone();
        let chmod_name = entry.name.clone();
        items.push(self.menu_action(
            "remote-chmod",
            "修改权限…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::RemoteChmod {
                        path: chmod_path.clone(),
                        name: chmod_name.clone(),
                    }),
                    window,
                    cx,
                )
            },
        ));
        let copy_path = path.clone();
        items.push(MenuItem::separator());
        items.push(self.menu_action(
            "remote-copy-path",
            "复制远程路径",
            cx,
            move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()));
            },
        ));
        self.menu = Some((position, items));
        cx.notify();
    }

    pub(super) fn ssh_local_menu(
        &mut self,
        entry: LocalEntry,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let path = entry.path.clone();
        let mut items = Vec::new();
        if self
            .modules
            .ssh
            .panel()
            .local
            .as_ref()
            .is_some_and(|browser| browser.selected_paths.len() > 1)
        {
            items.push(self.menu_action(
                "local-upload-selection",
                "上传所有选中项",
                cx,
                |app, _, cx| app.upload_selected_local(cx),
            ));
        }
        if entry.directory {
            let enter_path = path.clone();
            items.push(
                self.menu_action("local-enter", "进入目录", cx, move |this, _, cx| {
                    this.navigate_local(enter_path.clone(), cx)
                }),
            );
            let directory_entry = entry.clone();
            items.push(self.menu_action(
                "local-upload-dir",
                "上传此目录",
                cx,
                move |this, _, cx| {
                    this.upload_local_directory(directory_entry.clone(), cx);
                },
            ));
        } else {
            let upload_path = path.clone();
            items.push(self.menu_action(
                "local-upload",
                "上传到远程",
                cx,
                move |this, _, cx| {
                    if let Some(browser) = &mut this.modules.ssh.panel_mut().local {
                        browser.selected_paths.clear();
                        browser.selected_paths.insert(upload_path.clone());
                        browser.selected = Some(upload_path.clone());
                    }
                    this.upload_selected_local(cx);
                },
            ));
        }
        let copy_path = path.clone();
        items.push(MenuItem::separator());
        items.push(self.menu_action(
            "local-copy-path",
            "复制本地路径",
            cx,
            move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_path.display().to_string()));
            },
        ));
        self.menu = Some((position, items));
        cx.notify();
    }

    pub(super) fn connection_of(&self, id: &str) -> Option<SshConnection> {
        self.modules
            .ssh
            .registry
            .connections
            .iter()
            .find(|connection| connection.id == id)
            .cloned()
    }
}

pub(super) fn unique_remote_name(entries: &[RemoteEntry], name: &str) -> String {
    if !entries.iter().any(|entry| entry.name == name) {
        return name.to_string();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    for index in 1..10_000 {
        let candidate = format!("{stem} ({index}){extension}");
        if !entries.iter().any(|entry| entry.name == candidate) {
            return candidate;
        }
    }
    format!("{stem}-{}{extension}", uuid::Uuid::new_v4())
}

pub(super) fn unique_download_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or(Path::new("."));
    for index in 1..10_000 {
        let candidate = parent.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem}-{}{extension}", uuid::Uuid::new_v4()))
}

pub(super) fn default_download_directory() -> PathBuf {
    let home = LocalBrowser::home();
    let downloads = home.join("Downloads");
    if downloads.is_dir() {
        downloads
    } else {
        home
    }
}
