use super::{expand_path, DatabaseConnection, DatabaseEngine};
use crate::model::session::ModuleId;
use crate::modules::common::grouped::{self, Group};
use crate::services::storage::Storage;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct DatabaseRegistry {
    pub connections: Vec<DatabaseConnection>,
    pub groups: Vec<Group>,
    pub selected: Option<String>,
}

impl DatabaseRegistry {
    pub fn load(storage: &Storage) -> Result<Self> {
        storage.read_json(&storage.registry_path(ModuleId::Database))
    }

    pub fn save(&self, storage: &Storage) -> Result<()> {
        storage.write_private(&storage.registry_path(ModuleId::Database), self)
    }

    pub fn add_sqlite(&mut self, path: &str) -> Result<String> {
        self.add_sqlite_in_group(path, None)
    }

    pub fn add_sqlite_in_group(&mut self, path: &str, group: Option<String>) -> Result<String> {
        anyhow::ensure!(
            group
                .as_ref()
                .is_none_or(|id| self.groups.iter().any(|group| &group.id == id)),
            "所选分组不存在"
        );
        let requested = expand_path(path);
        anyhow::ensure!(!requested.as_os_str().is_empty(), "请填写 SQLite 文件路径");
        let parent = requested
            .parent()
            .filter(|parent| parent.is_dir())
            .with_context(|| format!("数据库目录不存在：{}", requested.display()))?;
        if !requested.exists() {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&requested)
                .with_context(|| format!("无法创建 SQLite 数据库：{}", requested.display()))?;
        }
        let path = parent
            .canonicalize()?
            .join(requested.file_name().context("数据库文件名无效")?);
        anyhow::ensure!(path.is_file(), "SQLite 路径不是文件");
        if let Some(existing) = self
            .connections
            .iter()
            .find(|connection| connection.path == path)
        {
            let id = existing.id.clone();
            if group.is_some() {
                if let Some(connection) = self
                    .connections
                    .iter_mut()
                    .find(|connection| connection.id == id)
                {
                    connection.group = group;
                }
            }
            self.selected = Some(id.clone());
            return Ok(id);
        }
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let id = Uuid::new_v4().to_string();
        self.connections.push(DatabaseConnection {
            id: id.clone(),
            name,
            engine: DatabaseEngine::Sqlite,
            path,
            group,
            ..DatabaseConnection::default()
        });
        self.selected = Some(id.clone());
        Ok(id)
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

    pub fn add_group(&mut self, name: &str) -> Result<String> {
        grouped::add_group(&mut self.groups, name)
    }

    pub fn remove_group(&mut self, id: &str) {
        grouped::remove_group(&mut self.groups, &mut self.connections, id);
    }

    pub fn rename_group(&mut self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "分组名称不能为空");
        anyhow::ensure!(
            !self
                .groups
                .iter()
                .any(|group| group.id != id && group.name == name),
            "分组名称已存在"
        );
        let group = self
            .groups
            .iter_mut()
            .find(|group| group.id == id)
            .context("分组不存在")?;
        group.name = name.to_string();
        Ok(())
    }

    pub fn move_connection(&mut self, id: &str, group: Option<String>, before: Option<&str>) {
        grouped::move_item(&mut self.connections, id, group, before);
    }

    pub fn shift_connection(&mut self, id: &str, direction: isize) {
        grouped::shift_item(&mut self.connections, id, direction);
    }

    pub fn move_group(&mut self, id: &str, before: &str) {
        grouped::move_group(&mut self.groups, id, before);
    }

    pub fn shift_group(&mut self, id: &str, direction: isize) {
        grouped::shift_group(&mut self.groups, id, direction);
    }

    pub fn upsert(&mut self, mut connection: DatabaseConnection) -> Result<String> {
        anyhow::ensure!(!connection.name.trim().is_empty(), "连接名称不能为空");
        anyhow::ensure!(
            connection
                .group
                .as_ref()
                .is_none_or(|id| self.groups.iter().any(|group| &group.id == id)),
            "所选分组不存在"
        );
        if connection.engine.is_file() {
            anyhow::ensure!(connection.path.is_file(), "SQLite 数据库文件不存在");
        } else {
            anyhow::ensure!(!connection.host.trim().is_empty(), "请填写数据库主机");
            anyhow::ensure!(connection.port > 0, "数据库端口无效");
            anyhow::ensure!(!connection.user.trim().is_empty(), "请填写数据库用户名");
        }
        if connection.id.is_empty() {
            connection.id = Uuid::new_v4().to_string();
        }
        if let Some(existing) = self
            .connections
            .iter_mut()
            .find(|existing| existing.id == connection.id)
        {
            *existing = connection.clone();
        } else {
            self.connections.push(connection.clone());
        }
        self.selected = Some(connection.id.clone());
        Ok(connection.id)
    }
}
