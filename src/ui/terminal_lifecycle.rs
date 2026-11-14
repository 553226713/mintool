use crate::{app::MintoolApp, services::mutex::lock_unpoisoned};
use gpui::*;

impl MintoolApp {
    pub(crate) fn clear_preview(&mut self, project: &str, id: u64) {
        self.previews.remove(&(project.to_string(), id));
        self.image_previews.remove(&(project.to_string(), id));
    }

    pub(crate) fn stop_terminal(&mut self, project: &str, id: u64, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspaces.get_mut(project) {
            if let Some(session) = workspace.terminals.remove(&id) {
                lock_unpoisoned(&session).shutdown();
            }
            workspace.errors.remove(&id);
        }
        self.terminals.remove(&(project.to_string(), id));
        if let Some(terminal) = self.system_terminals.remove(&(project.to_string(), id)) {
            terminal.update(cx, |view, _| view.stop());
        }
        self.clear_preview(project, id);
    }

    pub(crate) fn stop_project_terminals(&mut self, project: &str, cx: &mut Context<Self>) {
        let mut ids = Vec::new();
        if let Some(workspace) = self.workspaces.get(project) {
            workspace.state.layout.ids(&mut ids);
        }
        for id in ids {
            self.stop_terminal(project, id, cx);
        }
    }

    pub(crate) fn close_pane(&mut self, pane: u64, cx: &mut Context<Self>) {
        let Some(project) = self.projects.selected.clone() else {
            return;
        };
        let Some(workspace) = self.workspaces.get_mut(&project) else {
            return;
        };
        if workspace.state.files.remove(&pane).is_some() {
            self.dock_closes.push((project.clone(), pane));
            self.dock_file_tabs
                .retain(|(project_id, _, file_id)| project_id != &project || *file_id != pane);
            self.clear_preview(&project, pane);
            self.panel_states.retain(|key, _| key == "app-panels");
            self.changed(cx);
            return;
        }
        if workspace.state.layout.close(pane) {
            self.dock_closes.push((project.clone(), pane));
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
        self.stop_terminal(&project, pane, cx);
        self.panel_states.retain(|key, _| key == "app-panels");
        self.changed(cx);
    }
}
