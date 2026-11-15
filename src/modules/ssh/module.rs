use super::{
    browser::RemoteBrowser, form::SshForm, local::LocalBrowser, model::SshRegistry,
    remote_preview::RemotePreview,
};
use crate::model::session::SessionKey;
use adabraka_ui::components::{input_state::InputState, select::Select};
use gpui::Entity;
use std::collections::HashMap;

/// Sidebar and form state owned by the SSH module.
pub struct SshModule {
    pub registry: SshRegistry,
    pub search: Entity<InputState>,
    pub group_select: Entity<Select<Option<String>>>,
    pub form: SshForm,
    pub expanded: bool,
    /// Connection whose workspace is currently open (selected != opened).
    pub opened: Option<String>,
    pub remote: Option<RemoteBrowser>,
    pub local: Option<LocalBrowser>,
    pub local_visible: bool,
    pub transfer_status: Option<(bool, String)>,
    /// Remote preview panels keyed by owning session and pane.
    pub remote_previews: HashMap<(SessionKey, u64), Entity<RemotePreview>>,
}
