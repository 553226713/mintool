//! Generic, crash-safe JSON/TOML persistence.
//!
//! This layer deliberately knows nothing about feature modules. It exposes a
//! locked root plus generic read/write helpers; each module owns its file
//! names, loaders and savers.

use crate::model::session::{ModuleId, SessionKey};
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
        fs::create_dir_all(root.join("logs"))?;
        fs::create_dir_all(root.join("modules"))?;
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
        Self::new(crate::services::desktop::home_dir().join(".mintool"))
    }
    pub fn module_dir(&self, module: ModuleId) -> PathBuf {
        Self::module_root(&self.root, module)
    }
    fn module_root(root: &Path, module: ModuleId) -> PathBuf {
        root.join("modules").join(module.slug())
    }
    pub fn registry_path(&self, module: ModuleId) -> PathBuf {
        self.module_dir(module).join("registry.json")
    }
    pub fn session_path(&self, key: &SessionKey) -> Result<PathBuf> {
        anyhow::ensure!(
            !key.id.is_empty()
                && key.id.len() <= 160
                && key
                    .id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric()
                        || character == '-'
                        || character == '_'),
            "会话 ID 无效"
        );
        Ok(self
            .module_dir(key.module)
            .join("sessions")
            .join(format!("{}.json", key.id)))
    }
    /// Read JSON or TOML depending on the file extension, falling back to the
    /// `.bak` copy and repairing a corrupt file when possible.
    pub fn read_json<T: DeserializeOwned + Default>(&self, path: &Path) -> Result<T> {
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
        self.write_inner(path, value, false)
    }
    /// Same as [`Self::write`], but keeps the file readable by the owner only.
    pub fn write_private<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        self.write_inner(path, value, true)
    }
    fn write_inner<T: Serialize>(&self, path: &Path, value: &T, private: bool) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = if path
            .extension()
            .is_some_and(|extension| extension == "toml")
        {
            toml::to_string_pretty(value)?.into_bytes()
        } else {
            serde_json::to_vec_pretty(value)?
        };
        let temp = path.with_extension("tmp");
        let mut file = fs::File::create(&temp)?;
        file.write_all(&data)?;
        file.sync_all()?;
        if private {
            fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
        }
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
        if private {
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        // Release explicitly: concurrently spawned PTYs may briefly inherit the descriptor before exec.
        let _ = self._lock.unlock();
    }
}
