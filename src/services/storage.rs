use crate::model::{layout::WorkspaceState, project::ProjectRegistry, settings::Settings};
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub struct Storage {
    pub root: PathBuf,
    _lock: fs::File,
}
impl Storage {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("workspaces"))?;
        fs::create_dir_all(root.join("logs"))?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(root.join(".lock"))?;
        lock.try_lock()
            .context("该配置目录已有 Mintool 实例运行，请切换到已打开的窗口")?;
        Ok(Self { root, _lock: lock })
    }
    pub fn user() -> Result<Self> {
        if let Some(root) = std::env::var_os("MINTOOL_CONFIG_DIR") {
            return Self::new(PathBuf::from(root));
        }
        let home = std::env::var_os("HOME").context("找不到用户主目录 HOME")?;
        Self::new(PathBuf::from(home).join(".mintool"))
    }
    fn read<T: DeserializeOwned + Default>(&self, path: &Path) -> Result<T> {
        if !path.exists() {
            return Ok(T::default());
        }
        let parse = |path: &Path| -> Result<T> {
            let bytes = fs::read(path)?;
            if path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .contains(".toml")
            {
                Ok(toml::from_str(std::str::from_utf8(&bytes)?)?)
            } else {
                Ok(serde_json::from_slice(&bytes)?)
            }
        };
        match parse(path) {
            Ok(value) => Ok(value),
            Err(original) => {
                let backup = path.with_extension(format!(
                    "{}.bak",
                    path.extension().unwrap_or_default().to_string_lossy()
                ));
                let value = parse(&backup).with_context(|| {
                    format!("{} 配置与备份读取失败：{original}", path.display())
                })?;
                let corrupt = path.with_extension(format!("corrupt-{}", uuid::Uuid::new_v4()));
                fs::copy(path, corrupt)?;
                fs::copy(&backup, path)?;
                Ok(value)
            }
        }
    }
    pub fn write<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        let data = if path.extension().is_some_and(|e| e == "toml") {
            toml::to_string_pretty(value)?.into_bytes()
        } else {
            serde_json::to_vec_pretty(value)?
        };
        let temp = path.with_extension("tmp");
        let mut file = fs::File::create(&temp)?;
        file.write_all(&data)?;
        file.sync_all()?;
        if path.exists() {
            fs::copy(
                path,
                path.with_extension(format!(
                    "{}.bak",
                    path.extension().unwrap_or_default().to_string_lossy()
                )),
            )?;
        }
        fs::rename(temp, path)?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        self.read(&self.root.join("config.toml"))
    }
    pub fn projects(&self) -> Result<ProjectRegistry> {
        self.read(&self.root.join("projects.json"))
    }
    pub fn workspace(&self, id: &str) -> Result<WorkspaceState> {
        self.read(&self.workspace_path(id)?)
    }
    pub fn workspace_path(&self, id: &str) -> Result<PathBuf> {
        uuid::Uuid::parse_str(id).context("项目 ID 无效")?;
        Ok(self.root.join("workspaces").join(format!("{id}.json")))
    }
    pub fn save(&self, settings: &Settings, projects: &ProjectRegistry) -> Result<()> {
        self.write(&self.root.join("config.toml"), settings)?;
        self.write(&self.root.join("projects.json"), projects)
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        // Release explicitly: concurrently spawned PTYs may briefly inherit the descriptor before exec.
        let _ = self._lock.unlock();
    }
}
