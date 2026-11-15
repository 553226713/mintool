use super::Project;
use crate::model::session::ModuleId;
use crate::modules::common::grouped::{self, Group};
use crate::services::storage::Storage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ProjectRegistry {
    pub projects: Vec<Project>,
    pub groups: Vec<Group>,
    pub selected: Option<String>,
}
impl ProjectRegistry {
    pub fn load(storage: &Storage) -> Result<Self> {
        let path = storage.registry_path(ModuleId::Projects);
        let legacy = storage.root.join("projects.json");
        if !path.exists() && legacy.exists() {
            let registry: Self = storage.read_json(&legacy)?;
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&legacy, &path);
            return Ok(registry);
        }
        storage.read_json(&path)
    }
    pub fn save(&self, storage: &Storage) -> Result<()> {
        storage.write(&storage.registry_path(ModuleId::Projects), self)
    }
    pub fn add(&mut self, path: PathBuf) -> anyhow::Result<String> {
        self.add_in_group(path, None)
    }
    pub fn add_in_group(&mut self, path: PathBuf, group: Option<String>) -> anyhow::Result<String> {
        anyhow::ensure!(
            group
                .as_ref()
                .is_none_or(|id| self.groups.iter().any(|g| &g.id == id)),
            "所选分组不存在"
        );
        let path = path.canonicalize()?;
        anyhow::ensure!(path.is_dir(), "请选择一个目录");
        if let Some(project) = self.projects.iter().find(|project| project.path == path) {
            self.selected = Some(project.id.clone());
            return Ok(project.id.clone());
        }
        let id = Uuid::new_v4().to_string();
        self.projects.push(Project {
            name: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into(),
            path,
            id: id.clone(),
            group,
        });
        self.selected = Some(id.clone());
        Ok(id)
    }
    pub fn shift_project(&mut self, id: &str, direction: isize) {
        grouped::shift_item(&mut self.projects, id, direction);
    }
    pub fn shift_group(&mut self, id: &str, direction: isize) {
        grouped::shift_group(&mut self.groups, id, direction);
    }
    pub fn add_group(&mut self, name: &str) -> anyhow::Result<String> {
        grouped::add_group(&mut self.groups, name)
    }
    pub fn remove_group(&mut self, id: &str) {
        grouped::remove_group(&mut self.groups, &mut self.projects, id);
    }
    pub fn remove(&mut self, id: &str) {
        self.projects.retain(|project| project.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = self.projects.first().map(|project| project.id.clone());
        }
    }
    pub fn move_project(&mut self, id: &str, group: Option<String>, before: Option<&str>) {
        grouped::move_item(&mut self.projects, id, group, before);
    }
    pub fn move_group(&mut self, id: &str, before: &str) {
        grouped::move_group(&mut self.groups, id, before);
    }
}
