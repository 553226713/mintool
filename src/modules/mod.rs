//! Feature modules and the module-neutral session identity shared by all of them.
//!
//! Each module owns its registry, sidebar UI and persistence. The shared
//! `workspace` layer only deals with `SessionKey` and `SessionView`.

pub mod common;
pub mod database;
pub mod docker;
pub mod projects;
pub mod ssh;

use crate::model::session::{SessionView, TerminalLauncher};
use crate::services::storage::Storage;
use database::DatabaseModule;
use docker::DockerModule;
use projects::ProjectModule;
use ssh::SshModule;

pub use crate::model::session::{ModuleId, SessionKey, MODULES};

pub struct Modules {
    pub projects: ProjectModule,
    pub ssh: SshModule,
    pub docker: DockerModule,
    pub database: DatabaseModule,
    pub active: ModuleId,
}
impl Modules {
    pub(crate) fn stop_transfers(&self) {
        self.ssh.cancel_all_transfers();
    }

    pub(crate) fn close_pane_tools(&mut self, key: &SessionKey, pane: u64) {
        if key.module == ModuleId::Ssh {
            self.ssh.cancel_panel_tasks(key, Some(pane));
        }
    }
    pub(crate) fn drop_session_tools(&mut self, key: &SessionKey) {
        match key.module {
            ModuleId::Ssh => self.ssh.drop_session(key),
            ModuleId::Database => self.database.drop_session(key),
            _ => {}
        }
    }

    pub(crate) fn render_pane_tools(
        app: &mut crate::app::MintoolApp,
        session: &SessionView,
        pane: &crate::workspace::layout::Pane,
        width: f32,
        content: gpui::AnyElement,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<crate::app::MintoolApp>,
    ) -> gpui::AnyElement {
        match session.key.module {
            ModuleId::Ssh => app.render_ssh_pane(session, pane, width, content, window, cx),
            _ => content,
        }
    }

    pub fn expanded(&self, module: ModuleId) -> bool {
        match module {
            ModuleId::Projects => self.projects.expanded,
            ModuleId::Ssh => self.ssh.expanded,
            ModuleId::Docker => self.docker.expanded,
            ModuleId::Database => self.database.expanded,
        }
    }
    pub fn set_expanded(&mut self, module: ModuleId, expanded: bool) {
        match module {
            ModuleId::Projects => self.projects.expanded = expanded,
            ModuleId::Ssh => self.ssh.expanded = expanded,
            ModuleId::Docker => self.docker.expanded = expanded,
            ModuleId::Database => self.database.expanded = expanded,
        }
    }
    /// Session identity for the active module, resolved entirely inside the
    /// module registry so neutral layers do not match on `ModuleId`.
    pub fn active_session_key(&self) -> Option<SessionKey> {
        match self.active {
            ModuleId::Projects => self
                .projects
                .registry
                .selected
                .clone()
                .map(SessionKey::project),
            ModuleId::Ssh => self.ssh.opened.clone().map(SessionKey::ssh),
            ModuleId::Docker => self
                .docker
                .opened
                .as_ref()
                .map(|(id, view)| SessionKey::docker(docker::model::session_id(id, *view))),
            ModuleId::Database => self
                .database
                .registry
                .selected
                .clone()
                .map(SessionKey::database),
        }
    }
    /// Neutral description of an open session, built by its owning module.
    pub fn session_view(&self, key: &SessionKey) -> Option<SessionView> {
        match key.module {
            ModuleId::Projects => self
                .projects
                .registry
                .projects
                .iter()
                .find(|project| project.id == key.id)
                .map(SessionView::project),
            ModuleId::Ssh => self
                .ssh
                .registry
                .connections
                .iter()
                .find(|connection| connection.id == key.id)
                .map(SessionView::ssh),
            ModuleId::Docker => {
                let (container_id, view) = docker::model::parse_session_id(&key.id)?;
                if let Some(container) = self.docker.container(&container_id) {
                    let mut session = SessionView::docker(container, view);
                    if view == docker::model::DockerView::Logs {
                        if let TerminalLauncher::Docker { args, .. } = &mut session.launcher {
                            *args = docker::cli::logs_spec_with(
                                &container.id,
                                self.docker.log_tail,
                                self.docker.log_timestamps,
                            );
                        }
                    }
                    Some(session)
                } else if let Some(project) = container_id.strip_prefix("compose-") {
                    Some(SessionView::docker_compose(
                        project,
                        self.docker.log_tail,
                        self.docker.log_timestamps,
                    ))
                } else {
                    None
                }
            }
            ModuleId::Database => self
                .database
                .registry
                .connections
                .iter()
                .find(|connection| connection.id == key.id)
                .map(SessionView::database),
        }
    }
    /// Select the module-owned record behind a session key without any
    /// knowledge leaking into the workspace layer.
    pub fn select(&mut self, key: &SessionKey) {
        match key.module {
            ModuleId::Projects => {
                self.projects.registry.selected = Some(key.id.clone());
                let group = self
                    .projects
                    .registry
                    .projects
                    .iter()
                    .find(|project| project.id == key.id)
                    .and_then(|project| project.group.clone());
                if let Some(group) = group {
                    if let Some(group) = self
                        .projects
                        .registry
                        .groups
                        .iter_mut()
                        .find(|candidate| candidate.id == group)
                    {
                        group.collapsed = false;
                    }
                }
            }
            ModuleId::Ssh => {
                self.ssh.registry.selected = Some(key.id.clone());
                self.expand_ssh_group(&key.id);
            }
            ModuleId::Docker => {
                if let Some((container_id, _)) = docker::model::parse_session_id(&key.id) {
                    self.docker.selected = Some(container_id);
                }
            }
            ModuleId::Database => {
                self.database.registry.selected = Some(key.id.clone());
            }
        }
        self.active = key.module;
    }
    /// Highlight an SSH connection (without opening its workspace).
    pub fn select_ssh_connection(&mut self, id: &str) {
        self.ssh.registry.selected = Some(id.to_string());
        self.expand_ssh_group(id);
        self.active = ModuleId::Ssh;
    }
    fn expand_ssh_group(&mut self, id: &str) {
        let group = self
            .ssh
            .registry
            .connections
            .iter()
            .find(|connection| connection.id == id)
            .and_then(|connection| connection.group.clone());
        if let Some(group) = group {
            if let Some(group) = self
                .ssh
                .registry
                .groups
                .iter_mut()
                .find(|candidate| candidate.id == group)
            {
                group.collapsed = false;
            }
        }
    }
    /// The Docker session currently shown in the auxiliary panel, if any.
    pub fn docker_session(&self) -> Option<SessionView> {
        let (id, view) = self.docker.opened.as_ref()?;
        let key = SessionKey::docker(docker::model::session_id(id, *view));
        self.session_view(&key)
    }
    /// Persist every module-owned registry; the storage layer stays generic.
    pub fn save(&self, storage: &Storage) -> anyhow::Result<()> {
        self.projects.registry.save(storage)?;
        self.ssh.registry.save(storage)?;
        let mut saved = self.ssh.preferences.clone();
        for ((session, pane), panel) in &self.ssh.panels {
            let mut preferences = panel.preferences.clone();
            preferences.remote = panel
                .remote
                .as_ref()
                .map(|remote| remote.current.clone())
                .unwrap_or_default();
            preferences.local = panel.local.as_ref().map(|local| local.current.clone());
            preferences.local_visible = panel.local_visible;
            saved.insert(format!("{}/{}", session.id, pane), preferences);
        }
        storage.write(
            &storage.module_dir(ModuleId::Ssh).join("browsers.json"),
            &saved,
        )?;
        self.database.registry.save(storage)?;
        Ok(())
    }
}
