//! Git actions from the project changes panel.

use crate::app::MintoolApp;
use crate::model::session::SessionKey;
use crate::services::{files, git};
use crate::ui::dialogs::{Dialog, DiffDialog};
use gpui::*;
use std::path::PathBuf;

impl MintoolApp {
    fn git_root(&self) -> Option<PathBuf> {
        let project = self.current()?;
        self.workspaces
            .get(&SessionKey::project(&project.id))
            .and_then(|workspace| workspace.git.root.clone())
    }

    pub(crate) fn stage_git_path(&mut self, path: PathBuf, stage: bool, cx: &mut Context<Self>) {
        let Some(root) = self.git_root() else {
            return;
        };
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move {
                    if stage {
                        git::stage(&root, &[path])
                    } else {
                        git::unstage(&root, &[path])
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.error = Some(format!("Git 操作失败：{error:#}"));
                }
                this.monitor.force();
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn commit_git_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self
            .modules
            .projects
            .commit_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if message.is_empty() {
            self.error = Some("请填写提交说明".into());
            cx.notify();
            return;
        }
        let Some(root) = self.git_root() else {
            self.error = Some("当前项目不在 Git 仓库中".into());
            cx.notify();
            return;
        };
        self.modules.projects.commit_input.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { git::commit(&root, &message) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.error = Some(match result {
                    Ok(()) => "已提交".into(),
                    Err(error) => format!("提交失败：{error:#}"),
                });
                this.monitor.force();
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_git_diff(
        &mut self,
        path: PathBuf,
        staged: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self.git_root() else {
            return;
        };
        let untracked = self
            .current()
            .and_then(|project| {
                self.workspaces
                    .get(&SessionKey::project(&project.id))
                    .map(|workspace| workspace.git.status(&path).map(str::to_string))
            })
            .flatten()
            .as_deref()
            == Some("??");
        let text = if untracked {
            files::read_text(&path)
                .map(|text| {
                    text.lines()
                        .map(|line| format!("+{line}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|error| format!("无法读取新文件：{error:#}"))
        } else {
            let mut text = git::diff(&root, &path, staged)
                .unwrap_or_else(|error| format!("差异读取失败：{error:#}"));
            if text.trim().is_empty() {
                text = git::diff(&root, &path, !staged)
                    .unwrap_or_else(|error| format!("差异读取失败：{error:#}"));
            }
            if text.trim().is_empty() {
                "没有可显示的差异".into()
            } else {
                text
            }
        };
        self.show_dialog(Dialog::Diff(DiffDialog::new(path, text)), window, cx);
    }
}
