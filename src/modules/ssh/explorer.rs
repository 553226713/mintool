use super::dialogs::SshDialog;
use super::{
    browser::RemoteBrowser,
    local::{LocalBrowser, LocalEntry},
    model::SshConnection,
    module::TransferStatus,
    sftp::{self, RemoteEntry},
};
use crate::app::MintoolApp;
use crate::modules::ssh::path::{basename, join_remote, parent_of};
use crate::modules::SessionKey;
use crate::services::mutex::lock_unpoisoned;
use crate::ui::dialogs::Dialog;
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

impl MintoolApp {
    pub(crate) fn ensure_remote_browser(&mut self) {
        let Some(connection) = self.modules.ssh.opened.clone() else {
            self.modules.ssh.remote = None;
            return;
        };
        if self
            .modules
            .ssh
            .remote
            .as_ref()
            .is_none_or(|browser| browser.connection != connection)
        {
            self.modules.ssh.remote = Some(RemoteBrowser::new(connection));
            self.modules.ssh.transfer_status = None;
        }
    }

    pub(crate) fn start_remote_load(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(browser) = &self.modules.ssh.remote else {
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
        if let Some(browser) = &mut self.modules.ssh.remote {
            browser.loading.insert(path.clone());
            browser.error = None;
        }
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let load_path = path.clone();
            let result = executor
                .spawn(async move { sftp::list_directory(&connection, &load_path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(browser) = &mut this.modules.ssh.remote {
                    if browser.connection != browser_connection {
                        return;
                    }
                    browser.loading.remove(&path);
                    match result {
                        Ok(entries) => {
                            browser.entries.insert(path, entries);
                        }
                        Err(error) => browser.error = Some(format!("{error:#}")),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn navigate_remote(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(browser) = &mut self.modules.ssh.remote else {
            return;
        };
        if browser.current != path {
            browser.back.push(browser.current.clone());
        }
        browser.current = path.clone();
        browser.selected = None;
        if self.modules.ssh.sync_terminal {
            self.sync_ssh_terminal(&path, cx);
        }
        self.start_remote_load(path, cx);
        cx.notify();
    }

    pub(crate) fn remote_back(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = &mut self.modules.ssh.remote else {
            return;
        };
        let Some(previous) = browser.back.pop() else {
            return;
        };
        browser.current = previous.clone();
        browser.selected = None;
        self.start_remote_load(previous, cx);
        cx.notify();
    }

    pub(crate) fn remote_up(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = &self.modules.ssh.remote else {
            return;
        };
        let parent = parent_of(&browser.current).to_string();
        self.navigate_remote(parent, cx);
    }

    pub(crate) fn remote_home(&mut self, cx: &mut Context<Self>) {
        self.navigate_remote(String::new(), cx);
    }

    pub(crate) fn refresh_remote_current(&mut self, cx: &mut Context<Self>) {
        let Some(browser) = &mut self.modules.ssh.remote else {
            return;
        };
        let current = browser.current.clone();
        browser.entries.remove(&current);
        browser.loading.remove(&current);
        browser.selected = None;
        self.start_remote_load(current, cx);
        cx.notify();
    }

    pub(crate) fn select_remote(&mut self, path: String, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.remote {
            browser.selected = Some(path);
        }
        cx.notify();
    }

    pub(crate) fn select_local(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.local {
            browser.selected = Some(path);
        }
        cx.notify();
    }

    pub(crate) fn navigate_local(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.local {
            browser.navigate(path);
        }
        cx.notify();
    }

    pub(crate) fn local_back(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.local {
            browser.back();
        }
        cx.notify();
    }

    pub(crate) fn local_up(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.local {
            browser.up();
        }
        cx.notify();
    }

    pub(crate) fn local_home(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.local {
            browser.navigate(LocalBrowser::home());
        }
        cx.notify();
    }

    pub(crate) fn refresh_local(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.local {
            browser.refresh();
        }
        cx.notify();
    }

    fn sync_ssh_terminal(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(key) = self.active_session_key() else {
            return;
        };
        if key.module != crate::modules::ModuleId::Ssh {
            return;
        }
        let Some(pane) = self
            .workspaces
            .get(&key)
            .map(|workspace| workspace.state.focused)
        else {
            return;
        };
        let Some(terminal) = self.terminals.get(&(key, pane)) else {
            return;
        };
        let quoted = path.replace('\'', "'\\''");
        let command = format!("cd -- '{quoted}' 2>/dev/null\n");
        let view = terminal.read(cx);
        let mut session = lock_unpoisoned(&view.session);
        let _ = session.send(command.as_bytes());
    }

    pub(crate) fn toggle_local_panel(&mut self, cx: &mut Context<Self>) {
        self.modules.ssh.local_visible = !self.modules.ssh.local_visible;
        if self.modules.ssh.local_visible && self.modules.ssh.local.is_none() {
            self.modules.ssh.local = Some(LocalBrowser::new(LocalBrowser::home()));
        }
        cx.notify();
    }

    pub(crate) fn open_remote_file(&mut self, remote: String, cx: &mut Context<Self>) {
        let Some(browser) = &self.modules.ssh.remote else {
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
        let Some(browser) = &self.modules.ssh.remote else {
            return;
        };
        let Some(connection) = self.connection_of(&browser.connection) else {
            return;
        };
        let home = LocalBrowser::home();
        let downloads = home.join("Downloads");
        let directory = if downloads.is_dir() { downloads } else { home };
        let name = basename(&remote).to_string();
        let destination = unique_download_path(directory.join(&name));
        self.transfer_remote_file(connection, remote, name, destination, cx);
    }

    pub(crate) fn upload_selected_local(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = &self.modules.ssh.remote else {
            return;
        };
        let Some(local) = &self.modules.ssh.local else {
            return;
        };
        let Some(entry) = local.selected_file().cloned() else {
            self.modules.ssh.transfer_status = Some(TransferStatus::error("请先选择本地文件"));
            cx.notify();
            return;
        };
        let Some(connection) = self.connection_of(&remote.connection) else {
            return;
        };
        if !remote.entries.contains_key(&remote.current) {
            self.modules.ssh.transfer_status =
                Some(TransferStatus::error("远程目录尚未加载完成，请稍后重试"));
            cx.notify();
            return;
        }
        let remote_dir = remote.current.clone();
        let existing = remote
            .entries
            .get(&remote_dir)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let remote_name = unique_remote_name(existing, &entry.name);
        let remote_path = join_remote(&remote_dir, &remote_name);
        let display = format!("~/{}", remote_path);
        let executor = cx.background_executor().clone();
        let local_path = entry.path.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let task_cancel = cancel.clone();
        let check_cancel = cancel.clone();
        let progress = Arc::new(sftp::TransferProgress::default());
        self.modules.ssh.transfer_status = Some(TransferStatus::running_progress(
            format!("正在上传 {}…", entry.name),
            cancel,
            progress.clone(),
        ));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move {
                    sftp::upload_to_with_progress(
                        &connection,
                        &local_path,
                        &remote_path,
                        task_cancel,
                        progress,
                    )
                })
                .await;
            let cancelled = check_cancel.load(Ordering::Relaxed);
            let _ = this.update(cx, |this, cx| {
                this.modules.ssh.transfer_status = Some(match result {
                    Ok(()) => TransferStatus::info(format!("已上传到 {display}")),
                    Err(_) if cancelled => TransferStatus::error("已取消上传"),
                    Err(error) => TransferStatus::error(format!("上传失败：{error:#}")),
                });
                if let Some(browser) = &mut this.modules.ssh.remote {
                    browser.entries.remove(&remote_dir);
                    browser.loading.remove(&remote_dir);
                }
                this.start_remote_load(remote_dir.clone(), cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn download_selected_remote(&mut self, cx: &mut Context<Self>) {
        let Some(remote) = &self.modules.ssh.remote else {
            return;
        };
        let Some(entry) = selected_remote_file(remote).cloned() else {
            self.modules.ssh.transfer_status = Some(TransferStatus::error("请先选择远程文件"));
            cx.notify();
            return;
        };
        let Some(connection) = self.connection_of(&remote.connection) else {
            return;
        };
        let directory = self
            .modules
            .ssh
            .local
            .as_ref()
            .map(|browser| browser.current.clone())
            .unwrap_or_else(default_download_directory);
        let destination = unique_download_path(directory.join(&entry.name));
        self.transfer_remote_file(connection, entry.path, entry.name, destination, cx);
    }

    fn transfer_remote_file(
        &mut self,
        connection: SshConnection,
        remote: String,
        name: String,
        destination: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let executor = cx.background_executor().clone();
        let display = destination.display().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        let task_cancel = cancel.clone();
        let check_cancel = cancel.clone();
        let progress = Arc::new(sftp::TransferProgress::default());
        self.modules.ssh.transfer_status = Some(TransferStatus::running_progress(
            format!("正在下载 {name}…"),
            cancel,
            progress.clone(),
        ));
        cx.notify();
        let selected = destination.clone();
        cx.spawn(async move |this, cx| {
            let task_destination = destination.clone();
            let result = executor
                .spawn(async move {
                    sftp::download_to_with_progress(
                        &connection,
                        &remote,
                        &task_destination,
                        task_cancel,
                        progress,
                    )
                })
                .await;
            let cancelled = check_cancel.load(Ordering::Relaxed);
            let _ = this.update(cx, |this, cx| {
                let success = result.is_ok();
                this.modules.ssh.transfer_status = Some(match result {
                    Ok(()) => TransferStatus::info(format!("已下载到 {display}")),
                    Err(_) if cancelled => TransferStatus::error("已取消下载"),
                    Err(error) => TransferStatus::error(format!("下载失败：{error:#}")),
                });
                if success {
                    if let Some(local) = &mut this.modules.ssh.local {
                        local.refresh();
                        local.selected = Some(selected);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn enter_remote_directory_in_terminal(
        &mut self,
        path: String,
        cx: &mut Context<Self>,
    ) {
        let Some(browser) = &self.modules.ssh.remote else {
            return;
        };
        let key = SessionKey::ssh(browser.connection.clone());
        let Some(workspace) = self.workspaces.get_mut(&key) else {
            return;
        };
        let mut ids = Vec::new();
        workspace.state.layout.ids(&mut ids);
        let preferred = ids.contains(&workspace.state.focused);
        let ordered = if preferred {
            let mut ordered = vec![workspace.state.focused];
            ordered.extend(
                ids.iter()
                    .copied()
                    .filter(|id| *id != workspace.state.focused),
            );
            ordered
        } else {
            ids.clone()
        };
        let Some((pane_id, terminal)) = ordered.iter().find_map(|id| {
            self.terminals
                .get(&(key.clone(), *id))
                .map(|terminal| (*id, terminal.clone()))
        }) else {
            self.error = Some("当前 SSH 会话没有可用终端".into());
            cx.notify();
            return;
        };
        let target = if path.is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", path)
        };
        let escaped = target.replace('\'', "'\\''");
        terminal.update(cx, |view, cx| {
            view.send(format!("cd -- '{escaped}'\n").as_bytes(), cx);
        });
        workspace.state.focused = pane_id;
        self.focus_pending = true;
        self.changed(cx);
    }

    pub(super) fn ssh_remote_menu(
        &mut self,
        entry: RemoteEntry,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let path = entry.path.clone();
        let mut items = Vec::new();
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
                "在终端中进入此目录",
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
                    if let Some(browser) = &mut this.modules.ssh.remote {
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
                cx.write_to_clipboard(ClipboardItem::new_string(format!("~/{}", copy_path)));
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
                    if let Some(browser) = &mut this.modules.ssh.local {
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

pub(super) fn selected_remote_file(browser: &RemoteBrowser) -> Option<&RemoteEntry> {
    let selected = browser.selected.as_deref()?;
    browser
        .entries
        .get(&browser.current)?
        .iter()
        .find(|entry| entry.path == selected && !entry.directory)
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
    name.to_string()
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
    path
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
