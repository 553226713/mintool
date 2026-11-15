use super::SshConnection;
use crate::model::session::ModuleId;
use crate::modules::common::grouped::{self, Group};
use crate::services::storage::Storage;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct SshRegistry {
    pub connections: Vec<SshConnection>,
    pub groups: Vec<Group>,
    pub selected: Option<String>,
}
impl SshRegistry {
    pub fn load(storage: &Storage) -> Result<Self> {
        storage.read_json(&storage.registry_path(ModuleId::Ssh))
    }
    pub fn save(&self, storage: &Storage) -> Result<()> {
        storage.write_private(&storage.registry_path(ModuleId::Ssh), self)
    }
    pub fn add(&mut self, connection: SshConnection) -> String {
        let id = connection.id.clone();
        self.connections.push(connection);
        self.selected = Some(id.clone());
        id
    }
    pub fn update(&mut self, connection: SshConnection) {
        if let Some(existing) = self
            .connections
            .iter_mut()
            .find(|existing| existing.id == connection.id)
        {
            *existing = connection;
        }
    }
    pub fn remove(&mut self, id: &str) {
        self.connections.retain(|connection| connection.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = self
                .connections
                .first()
                .map(|connection| connection.id.clone());
        }
    }
    pub fn selected(&self) -> Option<&SshConnection> {
        let id = self.selected.as_deref()?;
        self.connections
            .iter()
            .find(|connection| connection.id == id)
    }
    pub fn shift_connection(&mut self, id: &str, direction: isize) {
        grouped::shift_item(&mut self.connections, id, direction);
    }
    pub fn shift_group(&mut self, id: &str, direction: isize) {
        grouped::shift_group(&mut self.groups, id, direction);
    }
    pub fn add_group(&mut self, name: &str) -> anyhow::Result<String> {
        grouped::add_group(&mut self.groups, name)
    }
    pub fn remove_group(&mut self, id: &str) {
        grouped::remove_group(&mut self.groups, &mut self.connections, id);
    }
    pub fn move_connection(&mut self, id: &str, group: Option<String>, before: Option<&str>) {
        grouped::move_item(&mut self.connections, id, group, before);
    }
    pub fn move_group(&mut self, id: &str, before: &str) {
        grouped::move_group(&mut self.groups, id, before);
    }
}
