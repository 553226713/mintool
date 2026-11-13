use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
}
#[derive(Default)]
pub struct FileCache {
    directories: HashMap<PathBuf, Vec<Entry>>,
    documents: HashMap<PathBuf, (Option<SystemTime>, String)>,
}
impl FileCache {
    pub fn invalidate(&mut self) {
        self.directories.clear();
        self.documents.clear();
    }
    pub fn invalidate_paths(&mut self, paths: &std::collections::BTreeSet<PathBuf>) {
        self.directories.retain(|cached, _| {
            !paths
                .iter()
                .any(|path| cached.starts_with(path) || path.parent() == Some(cached.as_path()))
        });
        self.documents
            .retain(|cached, _| !paths.iter().any(|path| cached.starts_with(path)));
    }
    pub fn entries(&mut self, path: &Path) -> Result<Vec<Entry>> {
        if let Some(entries) = self.directories.get(path) {
            return Ok(entries.clone());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(path).with_context(|| format!("无法读取 {}", path.display()))?
        {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".git" {
                continue;
            }
            entries.push(Entry {
                path: entry.path(),
                name,
                directory: file_type.is_dir(),
            });
        }
        entries.sort_by(|a, b| {
            b.directory
                .cmp(&a.directory)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        self.directories.insert(path.into(), entries.clone());
        Ok(entries)
    }
    pub fn document(&mut self, path: &Path) -> Result<&str> {
        let metadata =
            fs::metadata(path).with_context(|| format!("无法打开 {}", path.display()))?;
        anyhow::ensure!(metadata.is_file(), "这不是普通文件");
        anyhow::ensure!(
            metadata.len() <= 2 * 1024 * 1024,
            "文件超过 2 MiB，请在外部程序中查看"
        );
        let modified = metadata.modified().ok();
        if !self
            .documents
            .get(path)
            .is_some_and(|(time, _)| *time == modified)
        {
            let bytes = fs::read(path)?;
            anyhow::ensure!(!bytes.contains(&0), "二进制文件暂不支持预览");
            let text = String::from_utf8(bytes).context("暂仅支持 UTF-8 文本预览")?;
            self.documents.insert(path.into(), (modified, text));
        }
        Ok(&self.documents.get(path).unwrap().1)
    }
}
