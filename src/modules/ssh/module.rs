use super::{
    browser::RemoteBrowser, form::SshForm, local::LocalBrowser, model::SshRegistry,
    remote_preview::RemotePreview, sftp::TransferProgress,
};
use crate::model::session::SessionKey;
use adabraka_ui::components::{input_state::InputState, select::Select};
use gpui::Entity;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// Progress/result of the current file transfer. A cancel handle is present
/// only while a transfer is running.
#[derive(Clone)]
pub struct TransferStatus {
    pub success: bool,
    pub message: String,
    pub cancel: Option<Arc<AtomicBool>>,
    pub progress: Option<Arc<TransferProgress>>,
}

impl TransferStatus {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            cancel: None,
            progress: None,
        }
    }
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            cancel: None,
            progress: None,
        }
    }
    pub fn running(message: impl Into<String>, cancel: Arc<AtomicBool>) -> Self {
        Self {
            success: true,
            message: message.into(),
            cancel: Some(cancel),
            progress: None,
        }
    }
    pub fn running_progress(
        message: impl Into<String>,
        cancel: Arc<AtomicBool>,
        progress: Arc<TransferProgress>,
    ) -> Self {
        Self {
            success: true,
            message: message.into(),
            cancel: Some(cancel),
            progress: Some(progress),
        }
    }
    pub fn percent(&self) -> Option<u8> {
        self.progress
            .as_ref()
            .and_then(|progress| progress.percent())
    }
    pub fn is_running(&self) -> bool {
        self.cancel.is_some()
    }
    pub fn cancel(&self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// Sidebar and form state owned by the SSH module.
pub struct SshModule {
    pub registry: SshRegistry,
    pub search: Entity<InputState>,
    pub remote_filter: Entity<InputState>,
    pub group_select: Entity<Select<Option<String>>>,
    pub form: SshForm,
    pub expanded: bool,
    /// Connection whose workspace is currently open (selected != opened).
    pub opened: Option<String>,
    pub remote: Option<RemoteBrowser>,
    pub local: Option<LocalBrowser>,
    pub local_visible: bool,
    pub sync_terminal: bool,
    pub transfer_status: Option<TransferStatus>,
    /// Remote preview panels keyed by owning session and pane.
    pub remote_previews: HashMap<(SessionKey, u64), Entity<RemotePreview>>,
}
