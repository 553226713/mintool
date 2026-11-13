use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct DirectoryBrowser {
    pub path: PathBuf,
    pub directories: Vec<PathBuf>,
}
impl DirectoryBrowser {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("无法访问 {}", path.display()))?;
        anyhow::ensure!(path.is_dir(), "这不是目录：{}", path.display());
        let mut directories = Vec::new();
        for entry in fs::read_dir(&path).with_context(|| format!("无法读取 {}", path.display()))?
        {
            let entry = entry?;
            // Query the path itself: mount points and directory symlinks are
            // browsable regardless of the directory entry's reported file type.
            if entry.path().is_dir() {
                directories.push(entry.path());
            }
        }
        directories.sort_by_key(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        });
        Ok(Self { path, directories })
    }
    pub fn resolve(&self, value: &str) -> Result<PathBuf> {
        let value = value.trim();
        anyhow::ensure!(!value.is_empty(), "请输入目录路径");
        let path = if value == "~" || value.starts_with("~/") {
            let home = std::env::var_os("HOME").context("无法确定主目录")?;
            PathBuf::from(home).join(value.strip_prefix("~/").unwrap_or(""))
        } else {
            self.path.join(value)
        };
        let path = path
            .canonicalize()
            .with_context(|| format!("无法访问 {}", path.display()))?;
        anyhow::ensure!(path.is_dir(), "这不是目录：{}", path.display());
        Ok(path)
    }
}
