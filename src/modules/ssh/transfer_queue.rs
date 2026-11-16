use super::{
    explorer::{unique_download_path, unique_remote_name},
    path::{basename, join_remote, parent_of},
    sftp,
    transfer_task::{ConflictPolicy, TransferKind, TransferState, TransferTask},
};
use crate::app::MintoolApp;
use gpui::*;
use std::{path::PathBuf, sync::atomic::Ordering};

#[derive(Debug, PartialEq, Eq)]
pub enum TransferOutcome {
    Finished(String),
    Conflict,
    Skipped,
}

pub fn run_task(task: &TransferTask) -> anyhow::Result<TransferOutcome> {
    if task.cancel.load(Ordering::Relaxed) {
        anyhow::bail!("已取消");
    }
    let actual_destination = match &task.kind {
        TransferKind::Upload { local, remote } => {
            let entries = sftp::read_directory(&task.connection, parent_of(remote))?.entries;
            let existing = entries.iter().find(|entry| entry.name == basename(remote));
            let exists = existing.is_some();
            if exists && task.policy == ConflictPolicy::Ask {
                return Ok(TransferOutcome::Conflict);
            }
            if exists && task.policy == ConflictPolicy::Skip {
                return Ok(TransferOutcome::Skipped);
            }
            if exists
                && matches!(
                    task.policy,
                    ConflictPolicy::Overwrite | ConflictPolicy::Resume
                )
            {
                anyhow::ensure!(
                    existing.unwrap().directory == task.directory,
                    "目标的文件/目录类型不同，请选择重命名或跳过"
                );
            }
            let destination = if exists && task.policy == ConflictPolicy::Rename {
                join_remote(
                    parent_of(remote),
                    &unique_remote_name(&entries, basename(remote)),
                )
            } else {
                remote.clone()
            };
            if exists && task.policy == ConflictPolicy::Resume {
                anyhow::ensure!(
                    !task.directory,
                    "目录不支持续传，请选择合并覆盖、跳过或重命名"
                );
                sftp::resume_upload(
                    &task.connection,
                    local,
                    &destination,
                    task.cancel.clone(),
                    task.progress.clone(),
                )?;
            } else {
                sftp::upload_to_with_progress(
                    &task.connection,
                    local,
                    &destination,
                    task.cancel.clone(),
                    task.progress.clone(),
                )?;
            }
            destination
        }
        TransferKind::Download { remote, local } => {
            let exists = local.exists();
            if exists && task.policy == ConflictPolicy::Ask {
                return Ok(TransferOutcome::Conflict);
            }
            if exists && task.policy == ConflictPolicy::Skip {
                return Ok(TransferOutcome::Skipped);
            }
            if exists
                && matches!(
                    task.policy,
                    ConflictPolicy::Overwrite | ConflictPolicy::Resume
                )
            {
                anyhow::ensure!(
                    local.is_dir() == task.directory,
                    "目标的文件/目录类型不同，请选择重命名或跳过"
                );
            }
            let destination = if exists && task.policy == ConflictPolicy::Rename {
                unique_download_path(local.clone())
            } else {
                local.clone()
            };
            if exists && task.policy == ConflictPolicy::Resume {
                anyhow::ensure!(
                    !task.directory,
                    "目录不支持续传，请选择合并覆盖、跳过或重命名"
                );
                sftp::resume_download(
                    &task.connection,
                    remote,
                    &destination,
                    task.cancel.clone(),
                    task.progress.clone(),
                )?;
            } else {
                sftp::download_to_with_progress(
                    &task.connection,
                    remote,
                    &destination,
                    task.cancel.clone(),
                    task.progress.clone(),
                )?;
            }
            destination.display().to_string()
        }
    };
    Ok(TransferOutcome::Finished(actual_destination))
}

impl MintoolApp {
    fn enqueue_transfer(&mut self, kind: TransferKind, directory: bool, cx: &mut Context<Self>) {
        let Some(owner) = self.modules.ssh.active_panel.clone() else {
            return;
        };
        let Some(connection) = self.connection_of(&owner.0.id) else {
            return;
        };
        let panel = self.modules.ssh.panel();
        let policy = if panel.apply_conflicts {
            panel.conflict_policy
        } else {
            ConflictPolicy::Ask
        };
        self.modules.ssh.transfers.push(TransferTask::new(
            owner, connection, kind, directory, policy,
        ));
        self.pump_ssh_transfers(cx);
        cx.notify();
    }
    pub(crate) fn enqueue_uploads(
        &mut self,
        paths: Vec<PathBuf>,
        destination: String,
        cx: &mut Context<Self>,
    ) {
        for local in paths {
            let Some(name) = local
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
            else {
                continue;
            };
            let directory = local.is_dir();
            self.enqueue_transfer(
                TransferKind::Upload {
                    local,
                    remote: join_remote(&destination, &name),
                },
                directory,
                cx,
            );
        }
    }
    pub(crate) fn enqueue_downloads(
        &mut self,
        paths: Vec<(String, bool)>,
        destination: PathBuf,
        cx: &mut Context<Self>,
    ) {
        for (remote, directory) in paths {
            let local = destination.join(basename(&remote));
            self.enqueue_transfer(TransferKind::Download { remote, local }, directory, cx);
        }
    }
    pub(crate) fn pump_ssh_transfers(&mut self, cx: &mut Context<Self>) {
        // Serial transfers avoid conflicting writes and still allow all UI operations.
        if self
            .modules
            .ssh
            .transfers
            .iter()
            .any(|task| task.state == TransferState::Running)
        {
            return;
        }
        let Some(task) = self
            .modules
            .ssh
            .transfers
            .iter_mut()
            .find(|task| task.state == TransferState::Ready)
        else {
            return;
        };
        task.state = TransferState::Running;
        task.started = Some(std::time::Instant::now());
        let work = task.clone();
        let id = task.id;
        let attempt = task.attempt;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let result = executor.spawn(async move { run_task(&work) }).await;
            let _ = this.update(cx, |app, cx| {
                let Some(task) = app
                    .modules
                    .ssh
                    .transfers
                    .iter_mut()
                    .find(|task| task.id == id && task.attempt == attempt)
                else {
                    return;
                };
                task.state = match result {
                    Ok(TransferOutcome::Finished(destination)) => {
                        task.actual_destination = Some(destination);
                        TransferState::Finished
                    }
                    Ok(TransferOutcome::Skipped) => TransferState::Skipped,
                    Ok(TransferOutcome::Conflict) => TransferState::Conflict,
                    Err(error) => {
                        task.error = Some(format!("{error:#}"));
                        if task.cancel.load(Ordering::Relaxed) {
                            TransferState::Cancelled
                        } else {
                            TransferState::Failed
                        }
                    }
                };
                if task.state == TransferState::Finished {
                    if let Some(panel) = app.modules.ssh.panels.get_mut(&task.owner) {
                        if let Some(remote) = &mut panel.remote {
                            remote.entries.clear();
                        }
                        if let Some(local) = &mut panel.local {
                            local.refresh();
                        }
                    }
                }
                app.pump_ssh_transfers(cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn resolve_transfer(
        &mut self,
        id: uuid::Uuid,
        policy: ConflictPolicy,
        cx: &mut Context<Self>,
    ) {
        let Some(task) = self
            .modules
            .ssh
            .transfers
            .iter_mut()
            .find(|task| task.id == id)
        else {
            return;
        };
        let owner = task.owner.clone();
        task.policy = policy;
        task.retry();
        let apply_all = self
            .modules
            .ssh
            .panels
            .get(&owner)
            .is_some_and(|panel| panel.apply_conflicts);
        if apply_all {
            if let Some(panel) = self.modules.ssh.panels.get_mut(&owner) {
                panel.conflict_policy = policy;
            }
            for task in &mut self.modules.ssh.transfers {
                if task.owner == owner
                    && matches!(task.state, TransferState::Ready | TransferState::Conflict)
                {
                    task.policy = policy;
                    if task.state == TransferState::Conflict {
                        task.retry();
                    }
                }
            }
        }
        self.pump_ssh_transfers(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{run_task, TransferOutcome};
    use crate::{
        model::session::SessionKey,
        modules::ssh::{
            model::SshConnection,
            transfer_task::{ConflictPolicy, TransferKind, TransferTask},
        },
    };
    #[test]
    fn download_conflicts_and_skip_leave_the_existing_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing");
        std::fs::write(&path, "keep me").unwrap();
        let mut task = TransferTask::new(
            (SessionKey::ssh("owner"), 7),
            SshConnection::new("unused", "invalid.invalid", None),
            TransferKind::Download {
                remote: "/x".into(),
                local: path.clone(),
            },
            false,
            ConflictPolicy::Ask,
        );
        assert_eq!(run_task(&task).unwrap(), TransferOutcome::Conflict);
        task.policy = ConflictPolicy::Skip;
        assert_eq!(run_task(&task).unwrap(), TransferOutcome::Skipped);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "keep me");
        task.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(run_task(&task).is_err());
    }
}
