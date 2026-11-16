use crate::{app::MintoolApp, model::session::SessionKey};
use gpui::*;

pub(crate) fn scoped<F>(
    owner: Option<(SessionKey, u64)>,
    callback: F,
) -> impl FnOnce(&mut MintoolApp, &mut Context<MintoolApp>)
where
    F: FnOnce(&mut MintoolApp, &mut Context<MintoolApp>),
{
    move |app, cx| {
        let Some(owner) = owner.filter(|key| app.modules.ssh.panels.contains_key(key)) else {
            return;
        };
        let previous = app.modules.ssh.active_panel.replace(owner);
        callback(app, cx);
        app.modules.ssh.active_panel = previous;
    }
}

impl MintoolApp {
    pub(crate) fn activate_ssh_panel(&mut self, owner: (SessionKey, u64), cx: &mut Context<Self>) {
        if !self.modules.ssh.panels.contains_key(&owner) {
            let filter = cx.new(|cx| {
                adabraka_ui::components::input_state::InputState::new(cx)
                    .placeholder("筛选远端文件…")
            });
            let subscription = cx.subscribe(&filter, |_, _, _, cx| cx.notify());
            self.keep_subscription(subscription);
            let preference_key = format!("{}/{}", owner.0.id, owner.1);
            let preferences = self
                .modules
                .ssh
                .preferences
                .get(&preference_key)
                .or_else(|| {
                    self.modules
                        .ssh
                        .preferences
                        .get(&format!("{}/default", owner.0.id))
                })
                .cloned()
                .unwrap_or_default();
            let mut remote = super::browser::RemoteBrowser::new(owner.0.id.clone());
            remote.current = preferences.remote.clone();
            let local = super::local::LocalBrowser::new(
                preferences
                    .local
                    .clone()
                    .unwrap_or_else(super::local::LocalBrowser::home),
            );
            self.modules.ssh.panels.insert(
                owner.clone(),
                super::panel::SshPanelState {
                    remote: Some(remote),
                    local: Some(local),
                    local_visible: preferences.local_visible,
                    preferences,
                    navigation_remote: true,
                    conflict_policy: super::transfer_task::ConflictPolicy::Ask,
                    apply_conflicts: false,
                    row_bounds: Default::default(),
                    list_bounds: Default::default(),
                    selection_drag: None,
                    remote_filter: filter,
                    path_inputs: None,
                },
            );
        }
        self.modules.ssh.active_panel = Some(owner);
    }
}
