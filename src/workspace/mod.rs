//! Module-neutral workspace runtime: dock, panes, terminal/file lifecycle and
//! the persisted per-session layout.

pub mod dock_layout;
pub mod dock_pane;
pub mod dock_workspace;
pub mod layout;
pub mod lifecycle;
pub mod render;
pub mod session;

use crate::app::MintoolApp;
use crate::model::session::{ModuleId, SessionKey, SessionView};
use crate::workspace::session::Workspace;
use gpui::Context;
use std::time::Instant;

impl MintoolApp {
    pub(crate) fn active_session_key(&self) -> Option<SessionKey> {
        self.modules.active_session_key()
    }
    pub(crate) fn session_view(&self, key: &SessionKey) -> Option<SessionView> {
        self.modules.session_view(key)
    }
    pub(crate) fn current_session(&self) -> Option<SessionView> {
        self.active_session_key()
            .and_then(|key| self.session_view(&key))
    }
    pub(crate) fn ensure_session(&mut self, key: &SessionKey) -> bool {
        if self.workspaces.contains_key(key) {
            return true;
        }
        match Workspace::load(&self.storage, key) {
            Ok(state) => {
                self.workspaces.insert(key.clone(), Workspace::new(state));
                true
            }
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                false
            }
        }
    }
    pub(crate) fn select_project(&mut self, id: String, cx: &mut Context<Self>) {
        self.select_session(SessionKey::project(id), cx);
    }
    /// Highlight a connection without opening a session.
    pub(crate) fn select_ssh_connection(&mut self, id: String, cx: &mut Context<Self>) {
        self.modules.select_ssh_connection(&id);
        self.settings.active_module = ModuleId::Ssh;
        self.dirty = Some(Instant::now());
        cx.notify();
    }
    /// Open (or focus) the SSH workspace for a connection.
    pub(crate) fn open_ssh_connection(&mut self, id: String, cx: &mut Context<Self>) {
        let key = SessionKey::ssh(id.clone());
        self.select_session(key.clone(), cx);
        self.modules.ssh.opened = Some(id);
        if !self.ensure_session(&key) {
            return;
        }
        let pane = self.workspaces[&key].state.focused;
        if self.workspaces[&key]
            .state
            .layout
            .find(pane)
            .is_some_and(|pane| pane.terminal_closed)
        {
            self.restart_terminal(&key, pane, cx);
        }
        self.activate_ssh_panel((key.clone(), pane), cx);
        self.start_remote_load(String::new(), cx);
        self.focus_pending = true;
        self.changed(cx);
    }
    pub(crate) fn select_session(&mut self, key: SessionKey, cx: &mut Context<Self>) {
        if self.active_session_key().as_ref() == Some(&key) {
            return;
        }
        let previous = self.active_session_key();
        if let Some(previous) = &previous {
            self.dirty_workspaces.insert(previous.clone());
            self.previews.retain(|(session, _), _| session != previous);
            self.image_previews
                .retain(|(session, _), _| session != previous);
            self.modules
                .ssh
                .remote_previews
                .retain(|(session, _), _| session != previous);
            if let Some(workspace) = self.workspaces.get_mut(previous) {
                workspace.cache.invalidate();
                workspace.git = crate::services::git::GitSnapshot::default();
            }
        }
        self.modules.select(&key);
        self.settings.active_module = key.module;
        if key.module == ModuleId::Projects {
            self.modules.projects.selected_files.clear();
            self.modules.projects.renaming = None;
        }
        self.ensure_session(&key);
        self.focus_pending = true;
        self.dirty = Some(Instant::now());
        cx.notify();
    }
    pub(crate) fn switch_module(&mut self, module: ModuleId, cx: &mut Context<Self>) {
        if self.modules.active == module {
            self.toggle_sidebar(module, cx);
            return;
        }
        if let Some(previous) = self.active_session_key() {
            self.dirty_workspaces.insert(previous.clone());
            self.previews.retain(|(session, _), _| session != &previous);
            self.image_previews
                .retain(|(session, _), _| session != &previous);
            self.modules
                .ssh
                .remote_previews
                .retain(|(session, _), _| session != &previous);
        }
        self.modules.active = module;
        self.settings.active_module = module;
        if !self.modules.expanded(module) {
            self.modules.set_expanded(module, true);
            self.settings.set_sidebar(module, true);
        }
        if let Some(key) = self.active_session_key() {
            self.ensure_session(&key);
        }
        if module == ModuleId::Docker {
            self.ensure_docker_loaded(cx);
        }
        self.focus_pending = true;
        self.changed(cx);
    }
    pub(crate) fn remove_session(&mut self, key: &SessionKey, cx: &mut Context<Self>) {
        self.stop_session_terminals(key, cx);
        self.modules.drop_session_tools(key);
        self.terminals.retain(|(session, _), _| session != key);
        self.system_terminals
            .retain(|(session, _), _| session != key);
        self.previews.retain(|(session, _), _| session != key);
        self.image_previews.retain(|(session, _), _| session != key);
        self.modules
            .ssh
            .remote_previews
            .retain(|(session, _), _| session != key);
        self.workspaces.remove(key);
        self.docks.remove(key);
        self.dock_splits.retain(|(session, ..)| session != key);
        self.dock_file_tabs.retain(|(session, ..)| session != key);
        self.dock_closes.retain(|(session, ..)| session != key);
        self.changed(cx);
    }
    pub(crate) fn toggle_sidebar(&mut self, module: ModuleId, cx: &mut Context<Self>) {
        let expanded = !self.modules.expanded(module);
        self.modules.set_expanded(module, expanded);
        self.settings.set_sidebar(module, expanded);
        self.changed(cx);
    }
}
