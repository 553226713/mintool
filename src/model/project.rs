use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub group: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub collapsed: bool,
}
#[derive(Default, Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ProjectRegistry {
    pub projects: Vec<Project>,
    pub groups: Vec<Group>,
    pub selected: Option<String>,
}
impl ProjectRegistry {
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
        if let Some(project) = self.projects.iter().find(|p| p.path == path) {
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
        let Some(index) = self.projects.iter().position(|p| p.id == id) else {
            return;
        };
        let peers: Vec<usize> = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| p.group == self.projects[index].group)
            .map(|(i, _)| i)
            .collect();
        let position = peers.iter().position(|&i| i == index).unwrap();
        if let Some(next) = position
            .checked_add_signed(direction)
            .and_then(|i| peers.get(i))
        {
            self.projects.swap(index, *next);
        }
    }
    pub fn shift_group(&mut self, id: &str, direction: isize) {
        let Some(index) = self.groups.iter().position(|g| g.id == id) else {
            return;
        };
        if let Some(next) = index
            .checked_add_signed(direction)
            .filter(|&i| i < self.groups.len())
        {
            self.groups.swap(index, next);
        }
    }
    pub fn add_group(&mut self, name: &str) -> anyhow::Result<String> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "分组名称不能为空");
        anyhow::ensure!(
            !self.groups.iter().any(|g| g.name == name),
            "分组名称已存在"
        );
        let id = Uuid::new_v4().to_string();
        self.groups.push(Group {
            id: id.clone(),
            name: name.into(),
            collapsed: false,
        });
        Ok(id)
    }
    pub fn remove_group(&mut self, id: &str) {
        self.groups.retain(|g| g.id != id);
        for project in &mut self.projects {
            if project.group.as_deref() == Some(id) {
                project.group = None;
            }
        }
    }
    pub fn remove(&mut self, id: &str) {
        self.projects.retain(|p| p.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = self.projects.first().map(|p| p.id.clone());
        }
    }
    pub fn move_project(&mut self, id: &str, group: Option<String>, before: Option<&str>) {
        if before == Some(id) {
            return;
        }
        if let Some(index) = self.projects.iter().position(|p| p.id == id) {
            let mut project = self.projects.remove(index);
            project.group = group;
            let target = before
                .and_then(|b| self.projects.iter().position(|p| p.id == b))
                .unwrap_or(self.projects.len());
            self.projects.insert(target, project);
        }
    }
    pub fn move_group(&mut self, id: &str, before: &str) {
        if id == before {
            return;
        }
        if let Some(index) = self.groups.iter().position(|g| g.id == id) {
            let group = self.groups.remove(index);
            let target = self
                .groups
                .iter()
                .position(|g| g.id == before)
                .unwrap_or(self.groups.len());
            self.groups.insert(target, group);
        }
    }
}
