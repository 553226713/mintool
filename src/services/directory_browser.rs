use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct DirectoryBrowser {
    pub path: PathBuf,
    pub directories: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
}
impl DirectoryBrowser {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("无法访问 {}", path.display()))?;
        anyhow::ensure!(path.is_dir(), "这不是目录：{}", path.display());
        let mut directories = Vec::new();
        let mut files = Vec::new();
        for entry in fs::read_dir(&path).with_context(|| format!("无法读取 {}", path.display()))?
        {
            let entry = entry?;
            // Query the path itself: mount points and directory symlinks are
            // browsable regardless of the directory entry's reported file type.
            if entry.path().is_dir() {
                directories.push(entry.path());
            } else if entry.path().is_file() {
                files.push(entry.path());
            }
        }
        directories.sort_by_key(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        });
        files.sort_by_key(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        });
        Ok(Self {
            path,
            directories,
            files,
        })
    }
    pub fn resolve(&self, value: &str) -> Result<PathBuf> {
        let requested = self.resolve_path(value)?;
        let path = requested
            .canonicalize()
            .with_context(|| format!("无法访问 {}", requested.display()))?;
        anyhow::ensure!(path.is_dir(), "这不是目录：{}", path.display());
        Ok(path)
    }
    pub fn resolve_file(&self, value: &str) -> Result<PathBuf> {
        let path = self.resolve_path(value)?;
        anyhow::ensure!(path.is_file(), "请选择一个文件：{}", path.display());
        path.canonicalize()
            .with_context(|| format!("无法访问 {}", path.display()))
    }
    pub fn resolve_new_file(&self, value: &str) -> Result<PathBuf> {
        let path = self.resolve_path(value)?;
        anyhow::ensure!(!path.exists(), "文件已存在：{}", path.display());
        let parent = path.parent().context("文件目录无效")?;
        anyhow::ensure!(parent.is_dir(), "文件目录不存在：{}", parent.display());
        let name = path.file_name().context("文件名为空")?;
        Ok(parent.canonicalize()?.join(name))
    }
    fn resolve_path(&self, value: &str) -> Result<PathBuf> {
        let value = value.trim();
        anyhow::ensure!(!value.is_empty(), "请输入文件或目录路径");
        Ok(if value == "~" || value.starts_with("~/") {
            let home = std::env::var_os("HOME").context("无法确定主目录")?;
            PathBuf::from(home).join(value.strip_prefix("~/").unwrap_or(""))
        } else if Path::new(value).is_absolute() {
            PathBuf::from(value)
        } else {
            self.path.join(value)
        })
    }
}
