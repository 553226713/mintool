use crate::{
    app::MintoolApp,
    modules::SessionKey,
    services::mutex::lock_unpoisoned,
    ui::dialogs::{Dialog, UnsavedDialog},
};
use gpui::*;

impl MintoolApp {
    pub(crate) fn clear_preview(&mut self, key: &SessionKey, id: u64) {
        self.previews.remove(&(key.clone(), id));
        self.image_previews.remove(&(key.clone(), id));
        self.modules.ssh.remote_previews.remove(&(key.clone(), id));
    }

    pub(crate) fn stop_terminal(&mut self, key: &SessionKey, id: u64, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspaces.get_mut(key) {
            if let Some(session) = workspace.terminals.remove(&id) {
                lock_unpoisoned(&session).shutdown();
            }
            workspace.errors.remove(&id);
        }
        self.terminals.remove(&(key.clone(), id));
        if let Some(terminal) = self.system_terminals.remove(&(key.clone(), id)) {
            terminal.update(cx, |view, _| view.stop());
        }
        self.clear_preview(key, id);
    }

    pub(crate) fn stop_session_terminals(&mut self, key: &SessionKey, cx: &mut Context<Self>) {
        let mut ids = Vec::new();
        if let Some(workspace) = self.workspaces.get(key) {
            workspace.state.layout.ids(&mut ids);
        }
        for id in ids {
            self.stop_terminal(key, id, cx);
        }
    }

    /// Close a tab, asking first when its buffer has unsaved changes.
    pub(crate) fn request_close_pane(
        &mut self,
        key: &SessionKey,
        pane: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let unsaved = self
            .previews
            .get(&(key.clone(), pane))
            .is_some_and(|(_, _, state)| state.read(cx).has_unsaved_changes());
        if unsaved {
            self.show_dialog(
                Dialog::Unsaved(UnsavedDialog::close(key.clone(), pane)),
                window,
                cx,
            );
            return;
        }
        self.close_pane(key, pane, cx);
    }

    pub(crate) fn close_pane(&mut self, key: &SessionKey, pane: u64, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspaces.get_mut(key) else {
            return;
        };
        let local_file = workspace.state.files.remove(&pane).is_some();
        let remote_file = workspace.state.remote_files.remove(&pane).is_some();
        if local_file || remote_file {
            self.dock_closes.push((key.clone(), pane));
            self.dock_file_tabs
                .retain(|(session, _, file_id)| session != key || *file_id != pane);
            self.clear_preview(key, pane);
            self.panel_states.retain(|key, _| key == "app-panels");
            self.changed(cx);
            return;
        }
        if workspace.state.layout.close(pane) {
            self.dock_closes.push((key.clone(), pane));
            if workspace.state.focused == pane {
                workspace.state.focused = workspace.state.layout.first_id();
            }
        } else if let Some(pane) = workspace.state.layout.find_mut(pane) {
            // Keep an empty workspace surface after its final terminal is closed.
            pane.terminal_closed = true;
            pane.showing_file = false;
            pane.file = None;
        } else {
            return;
        }
        self.stop_terminal(key, pane, cx);
        self.panel_states.retain(|key, _| key == "app-panels");
        self.changed(cx);
    }
    /// Returns false to keep the window open while the user decides.
    pub fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.quit_confirmed {
            return true;
        }
        let unsaved = self
            .previews
            .values()
            .any(|(_, _, state)| state.read(cx).has_unsaved_changes());
        if unsaved {
            self.show_dialog(Dialog::Unsaved(UnsavedDialog::quit()), window, cx);
            false
        } else {
            true
        }
    }
}
