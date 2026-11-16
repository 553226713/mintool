use super::explorer::default_download_directory;
use super::{
    local::LocalEntry,
    model::SshConnection,
    path::{join_remote, parent_of},
    sftp::{self, RemoteEntry},
};
use crate::app::MintoolApp;
use gpui::Context;

impl MintoolApp {
    pub(crate) fn upload_local_directory(&mut self, entry: LocalEntry, cx: &mut Context<Self>) {
        let Some(remote) = &self.modules.ssh.panel().remote else {
            return;
        };
        self.enqueue_uploads(vec![entry.path], remote.current.clone(), cx);
    }
    pub(crate) fn download_remote_directory(&mut self, entry: RemoteEntry, cx: &mut Context<Self>) {
        let destination = self
            .modules
            .ssh
            .panel()
            .local
            .as_ref()
            .map(|browser| browser.current.clone())
            .unwrap_or_else(default_download_directory);
        self.enqueue_downloads(vec![(entry.path, true)], destination, cx);
    }
    fn remote_connection(&self) -> Option<SshConnection> {
        let id = self.modules.ssh.panel().remote.as_ref()?.connection.clone();
        self.connection_of(&id)
    }

    fn remote_refresh_after_change(&mut self, cx: &mut Context<Self>) {
        if let Some(browser) = &mut self.modules.ssh.panel_mut().remote {
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
        let owner = self.modules.ssh.active_panel.clone();
        cx.spawn(async move |this, cx| {
            let result = executor.spawn(async move { operation(connection) }).await;
            let _ = this.update(
                cx,
                super::scope::scoped(owner, |this, cx| {
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
                }),
            );
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
