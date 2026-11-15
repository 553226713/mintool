use super::explorer::{default_download_directory, unique_download_path, unique_remote_name};
use super::{
    local::LocalEntry,
    model::SshConnection,
    module::TransferStatus,
    path::{join_remote, parent_of},
    sftp::{self, RemoteEntry},
};
use crate::app::MintoolApp;
use gpui::Context;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

impl MintoolApp {
    pub(crate) fn upload_local_directory(&mut self, entry: LocalEntry, cx: &mut Context<Self>) {
        let Some(remote) = &self.modules.ssh.remote else {
            return;
        };
        let Some(connection) = self.connection_of(&remote.connection) else {
            return;
        };
        let remote_dir = remote.current.clone();
        let existing = remote
            .entries
            .get(&remote_dir)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let remote_name = unique_remote_name(existing, &entry.name);
        let remote_path = join_remote(&remote_dir, &remote_name);
        let display = format!("~/{}", remote_path);
        let cancel = Arc::new(AtomicBool::new(false));
        let task_cancel = cancel.clone();
        let check_cancel = cancel.clone();
        let progress = Arc::new(sftp::TransferProgress::default());
        self.modules.ssh.transfer_status = Some(TransferStatus::running_progress(
            format!("正在上传目录 {}…", entry.name),
            cancel,
            progress.clone(),
        ));
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let local = entry.path.clone();
            let result = executor
                .spawn(async move {
                    sftp::upload_to_with_progress(
                        &connection,
                        &local,
                        &remote_path,
                        task_cancel,
                        progress,
                    )
                })
                .await;
            let cancelled = check_cancel.load(Ordering::Relaxed);
            let _ = this.update(cx, |this, cx| {
                this.modules.ssh.transfer_status = Some(match result {
                    Ok(()) => TransferStatus::info(format!("已上传目录到 {display}")),
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

    pub(crate) fn download_remote_directory(&mut self, entry: RemoteEntry, cx: &mut Context<Self>) {
        let Some(connection) = self.remote_connection() else {
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
        let display = destination.display().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        let task_cancel = cancel.clone();
        let check_cancel = cancel.clone();
        let progress = Arc::new(sftp::TransferProgress::default());
        self.modules.ssh.transfer_status = Some(TransferStatus::running_progress(
            format!("正在下载目录 {}…", entry.name),
            cancel,
            progress.clone(),
        ));
        let executor = cx.background_executor().clone();
        let selected = destination.clone();
        cx.spawn(async move |this, cx| {
            let remote = entry.path.clone();
            let target = destination.clone();
            let result = executor
                .spawn(async move {
                    sftp::download_to_with_progress(
                        &connection,
                        &remote,
                        &target,
                        task_cancel,
                        progress,
                    )
                })
                .await;
            let cancelled = check_cancel.load(Ordering::Relaxed);
            let _ = this.update(cx, |this, cx| {
                let success = result.is_ok();
                this.modules.ssh.transfer_status = Some(match result {
                    Ok(()) => TransferStatus::info(format!("已下载目录到 {display}")),
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

    fn remote_connection(&self) -> Option<SshConnection> {
        let id = self.modules.ssh.remote.as_ref()?.connection.clone();
        self.connection_of(&id)
    }

    fn remote_refresh_after_change(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.remote {
            let current = browser.current.clone();
            browser.entries.remove(&current);
            browser.loading.remove(&current);
            browser.selected = None;
            self.start_remote_load(current, cx);
        }
        cx.notify();
    }

    fn remote_file_op<F>(
        &mut self,
        success: &'static str,
        failure: &'static str,
        operation: F,
        cx: &mut Context<Self>,
    ) where
        F: FnOnce(SshConnection) -> anyhow::Result<()> + Send + 'static,
    {
        let Some(connection) = self.remote_connection() else {
            self.error = Some("没有可用的 SSH 连接".into());
            cx.notify();
            return;
        };
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor.spawn(async move { operation(connection) }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.error = Some(success.into());
                        this.remote_refresh_after_change(cx);
                    }
                    Err(error) => {
                        this.error = Some(format!("{failure}：{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn remote_mkdir(&mut self, parent: String, name: String, cx: &mut Context<Self>) {
        let name = name.trim().to_string();
        if !valid_remote_name(&name) {
            self.error = Some("名称不能为空，也不能包含 /、. 或 ..".into());
            cx.notify();
            return;
        }
        let path = join_remote(&parent, &name);
        self.remote_file_op(
            "已新建远程文件夹",
            "新建文件夹失败",
            move |connection| sftp::mkdir(&connection, &path),
            cx,
        );
    }

    pub(crate) fn remote_rename(&mut self, path: String, name: String, cx: &mut Context<Self>) {
        let name = name.trim().to_string();
        if !valid_remote_name(&name) {
            self.error = Some("名称不能为空，也不能包含 /、. 或 ..".into());
            cx.notify();
            return;
        }
        let target = join_remote(parent_of(&path), &name);
        self.remote_file_op(
            "已重命名远程文件",
            "重命名失败",
            move |connection| sftp::rename(&connection, &path, &target),
            cx,
        );
    }

    pub(crate) fn remote_delete(&mut self, path: String, directory: bool, cx: &mut Context<Self>) {
        self.remote_file_op(
            if directory {
                "已删除远程目录"
            } else {
                "已删除远程文件"
            },
            "删除失败",
            move |connection| {
                if directory {
                    sftp::remove_directory(&connection, &path)
                } else {
                    sftp::remove_remote(&connection, &path)
                }
            },
            cx,
        );
    }

    pub(crate) fn remote_chmod(&mut self, path: String, mode: String, cx: &mut Context<Self>) {
        let mode = mode.trim().to_string();
        self.remote_file_op(
            "已修改远程权限",
            "修改权限失败",
            move |connection| sftp::chmod(&connection, &path, &mode),
            cx,
        );
    }
}

fn valid_remote_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/')
}
