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
    DockerContainer, DockerImage, DockerNetwork, DockerResource, DockerView, DockerVolume,
};
use std::time::Instant;

/// Live state for the local Docker engine. Containers are ephemeral, so only
/// UI preferences and per-container session layouts are persisted.
pub struct DockerModule {
    pub containers: Vec<DockerContainer>,
    pub images: Vec<DockerImage>,
    pub volumes: Vec<DockerVolume>,
    pub networks: Vec<DockerNetwork>,
    pub filter: Entity<InputState>,
    pub resource: DockerResource,
    pub selected: Option<String>,
    pub opened: Option<(String, DockerView)>,
    pub show_resources: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub last_refresh: Option<Instant>,
    pub expanded: bool,
}
impl DockerModule {
    pub fn new(filter: Entity<InputState>, expanded: bool) -> Self {
        Self {
            containers: Vec::new(),
            images: Vec::new(),
            volumes: Vec::new(),
            networks: Vec::new(),
            filter,
            resource: DockerResource::Containers,
            selected: None,
            opened: None,
            show_resources: true,
            loading: false,
            error: None,
            last_refresh: None,
            expanded,
        }
    }
    pub fn container(&self, id: &str) -> Option<&DockerContainer> {
        self.containers.iter().find(|container| container.id == id)
    }
    pub fn selected_container(&self) -> Option<&DockerContainer> {
        self.selected.as_deref().and_then(|id| self.container(id))
    }
}
