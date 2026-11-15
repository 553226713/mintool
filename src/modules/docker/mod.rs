pub mod actions;
pub mod cli;
pub mod dialogs;
pub mod menu;
pub mod model;
pub mod panel;
mod resource_summary;
pub mod resources;
pub mod sidebar;

use adabraka_ui::components::input_state::InputState;
use gpui::Entity;
use model::{
    ComposeProject, DockerContainer, DockerImage, DockerNetwork, DockerResource, DockerView,
    DockerVolume,
};
use std::time::Instant;

/// Inputs backing the "run from image" dialog.
pub struct DockerRunForm {
    pub name: Entity<InputState>,
    pub ports: Entity<InputState>,
    pub env: Entity<InputState>,
    pub mounts: Entity<InputState>,
    pub command: Entity<InputState>,
}

/// Live state for the local Docker engine. Containers are ephemeral, so only
/// UI preferences and per-container session layouts are persisted.
pub struct DockerModule {
    pub containers: Vec<DockerContainer>,
    pub images: Vec<DockerImage>,
    pub volumes: Vec<DockerVolume>,
    pub networks: Vec<DockerNetwork>,
    pub compose: Vec<ComposeProject>,
    pub compose_loading: bool,
    pub compose_loaded: bool,
    pub filter: Entity<InputState>,
    pub resource: DockerResource,
    pub selected: Option<String>,
    pub opened: Option<(String, DockerView)>,
    pub show_resources: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub last_refresh: Option<Instant>,
    pub expanded: bool,
    pub log_tail: u32,
    pub log_timestamps: bool,
    pub run_form: DockerRunForm,
}
impl DockerModule {
    pub fn new(filter: Entity<InputState>, expanded: bool, run_form: DockerRunForm) -> Self {
        Self {
            containers: Vec::new(),
            images: Vec::new(),
            volumes: Vec::new(),
            networks: Vec::new(),
            compose: Vec::new(),
            compose_loading: false,
            compose_loaded: false,
            filter,
            resource: DockerResource::Containers,
            selected: None,
            opened: None,
            show_resources: true,
            loading: false,
            error: None,
            last_refresh: None,
            expanded,
            log_tail: 500,
            log_timestamps: false,
            run_form,
        }
    }
    pub fn container(&self, id: &str) -> Option<&DockerContainer> {
        self.containers.iter().find(|container| container.id == id)
    }
    pub fn selected_container(&self) -> Option<&DockerContainer> {
        self.selected.as_deref().and_then(|id| self.container(id))
    }
    pub fn compose_project(&self, name: &str) -> Option<&ComposeProject> {
        self.compose.iter().find(|project| project.name == name)
    }
}
