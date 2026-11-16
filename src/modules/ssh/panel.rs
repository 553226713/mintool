use super::{browser::RemoteBrowser, local::LocalBrowser, navigation::PathInputs};
use adabraka_ui::components::input_state::InputState;
use gpui::Entity;

pub struct SshPanelState {
    pub preferences: super::preferences::BrowserPreferences,
    pub conflict_policy: super::transfer_task::ConflictPolicy,
    pub apply_conflicts: bool,
    pub list_bounds: std::collections::HashMap<bool, gpui::Bounds<gpui::Pixels>>,
    pub row_bounds: std::collections::HashMap<(bool, String), gpui::Bounds<gpui::Pixels>>,
    pub selection_drag: Option<(bool, gpui::Point<gpui::Pixels>, gpui::Point<gpui::Pixels>)>,
    pub remote: Option<RemoteBrowser>,
    pub local: Option<LocalBrowser>,
    pub local_visible: bool,
    pub navigation_remote: bool,
    pub remote_filter: Entity<InputState>,
    pub path_inputs: Option<PathInputs>,
}
