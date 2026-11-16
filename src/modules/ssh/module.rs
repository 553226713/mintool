use super::{form::SshForm, model::SshRegistry, remote_preview::RemotePreview};
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
    pub(crate) preferences: super::preferences::SavedBrowsers,
    pub(crate) transfers: Vec<super::transfer_task::TransferTask>,
    pub(crate) active_panel: Option<(SessionKey, u64)>,
    pub(crate) panels: HashMap<(SessionKey, u64), super::panel::SshPanelState>,
    /// Remote preview panels keyed by owning session and pane.
    pub remote_previews: HashMap<(SessionKey, u64), Entity<RemotePreview>>,
    pub(crate) pane_splits: HashMap<
        (SessionKey, u64),
        (
            Entity<adabraka_ui::components::resizable::ResizableState>,
            gpui::Subscription,
        ),
    >,
}

impl SshModule {
    pub(crate) fn cancel_panel_tasks(&mut self, key: &SessionKey, pane: Option<u64>) {
        use super::transfer_task::TransferState;
        for task in &mut self.transfers {
            if &task.owner.0 == key
                && pane.is_none_or(|id| task.owner.1 == id)
                && matches!(
                    task.state,
                    TransferState::Ready | TransferState::Running | TransferState::Conflict
                )
            {
                task.request_cancel();
                if task.state != TransferState::Running {
                    task.state = TransferState::Cancelled;
                }
            }
        }
    }
    pub(crate) fn drop_session(&mut self, key: &SessionKey) {
        self.cancel_panel_tasks(key, None);
        self.panels.retain(|(session, _), _| session != key);
        self.pane_splits.retain(|(session, _), _| session != key);
        if self
            .active_panel
            .as_ref()
            .is_some_and(|(session, _)| session == key)
        {
            self.active_panel = None;
        }
        let prefix = format!("{}/", key.id);
        self.preferences
            .retain(|name, _| !name.starts_with(&prefix));
    }
    pub(crate) fn cancel_all_transfers(&self) {
        for task in &self.transfers {
            task.request_cancel();
        }
    }
    pub(crate) fn panel(&self) -> &super::panel::SshPanelState {
        &self.panels[self.active_panel.as_ref().expect("SSH panel is active")]
    }
    pub(crate) fn panel_mut(&mut self) -> &mut super::panel::SshPanelState {
        self.panels
            .get_mut(self.active_panel.as_ref().expect("SSH panel is active"))
            .expect("SSH panel exists")
    }
}
