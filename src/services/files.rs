use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

const DOCUMENT_CHECK_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
}

struct CachedDocument {
    modified: Option<SystemTime>,
    checked: Instant,
    text: String,
}

#[derive(Default)]
pub struct FileCache {
    directories: HashMap<PathBuf, Arc<Vec<Entry>>>,
    documents: HashMap<PathBuf, CachedDocument>,
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
    pub fn entries(&mut self, path: &Path) -> Result<Arc<Vec<Entry>>> {
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
        let entries = Arc::new(entries);
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
        if self
            .documents
            .get(path)
            .is_some_and(|document| document.modified == modified)
        {
            let document = self.documents.get_mut(path).unwrap();
            document.checked = Instant::now();
            return Ok(&document.text);
        }
        let bytes = fs::read(path)?;
        anyhow::ensure!(!bytes.contains(&0), "二进制文件暂不支持预览");
        let text = String::from_utf8(bytes).context("暂仅支持 UTF-8 文本预览")?;
        let document = self.documents.entry(path.into()).or_insert(CachedDocument {
            modified,
            checked: Instant::now(),
            text: String::new(),
        });
        document.modified = modified;
        document.checked = Instant::now();
        document.text = text;
        Ok(&document.text)
    }
    pub fn document_fast(&mut self, path: &Path) -> Result<&str> {
        if self
            .documents
            .get(path)
            .is_some_and(|document| document.checked.elapsed() < DOCUMENT_CHECK_INTERVAL)
        {
            return Ok(&self.documents[path].text);
        }
        self.document(path)
    }
    pub fn cached_document(&self, path: &Path) -> Option<&str> {
        self.documents
            .get(path)
            .map(|document| document.text.as_str())
    }
}
