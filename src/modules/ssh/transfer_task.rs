use super::{model::SshConnection, sftp::TransferProgress};
use crate::model::session::SessionKey;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConflictPolicy {
    #[default]
    Ask,
    Overwrite,
    Skip,
    Rename,
    Resume,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferState {
    Ready,
    Running,
    Conflict,
    Finished,
    Failed,
    Cancelled,
    Skipped,
}
#[derive(Clone, Debug)]
pub enum TransferKind {
    Upload { local: PathBuf, remote: String },
    Download { remote: String, local: PathBuf },
}

#[derive(Clone)]
pub struct TransferTask {
    pub id: uuid::Uuid,
    pub owner: (SessionKey, u64),
    pub connection: SshConnection,
    pub kind: TransferKind,
    pub directory: bool,
    pub policy: ConflictPolicy,
    pub state: TransferState,
    pub error: Option<String>,
    pub actual_destination: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<TransferProgress>,
    pub started: Option<Instant>,
    pub attempt: u64,
}
impl TransferTask {
    pub fn new(
        owner: (SessionKey, u64),
        connection: SshConnection,
        kind: TransferKind,
        directory: bool,
        policy: ConflictPolicy,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            owner,
            connection,
            kind,
            directory,
            policy,
            state: TransferState::Ready,
            error: None,
            actual_destination: None,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(TransferProgress::default()),
            started: None,
            attempt: 0,
        }
    }
    pub fn name(&self) -> String {
        match &self.kind {
            TransferKind::Upload { local, .. } => format!(
                "上传 {}",
                local.file_name().unwrap_or_default().to_string_lossy()
            ),
            TransferKind::Download { remote, .. } => {
                format!("下载 {}", super::path::basename(remote))
            }
        }
    }
    pub fn request_cancel(&self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.progress.terminate_process();
    }
    pub fn destination(&self) -> String {
        if let Some(path) = &self.actual_destination {
            return path.clone();
        }
        match &self.kind {
            TransferKind::Upload { remote, .. } => remote.clone(),
            TransferKind::Download { local, .. } => local.display().to_string(),
        }
    }
    pub fn retry(&mut self) {
        if matches!(
            self.state,
            TransferState::Failed | TransferState::Cancelled | TransferState::Conflict
        ) {
            self.state = TransferState::Ready;
            self.error = None;
            self.actual_destination = None;
            self.cancel = Arc::new(AtomicBool::new(false));
            self.progress = Arc::new(TransferProgress::default());
            self.started = None;
            self.attempt += 1;
        }
    }
    pub fn metrics(&self) -> (u64, Option<u64>) {
        let elapsed = self
            .started
            .map_or(0., |started| started.elapsed().as_secs_f64());
        let speed = if elapsed > 0. {
            (self.progress.transferred_since_start() as f64 / elapsed) as u64
        } else {
            0
        };
        let remaining = if speed > 0 && self.progress.total() > 0 {
            Some(
                self.progress
                    .total()
                    .saturating_sub(self.progress.transferred())
                    / speed,
            )
        } else {
            None
        };
        (speed, remaining)
    }
}
