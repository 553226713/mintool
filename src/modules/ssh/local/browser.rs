use super::LocalEntry;
use std::{fs, path::PathBuf};

#[derive(Clone)]
pub struct LocalBrowser {
    pub current: PathBuf,
    pub entries: Vec<LocalEntry>,
    pub back: Vec<PathBuf>,
    pub selected: Option<PathBuf>,
    pub error: Option<String>,
}

impl LocalBrowser {
    pub fn new(start: PathBuf) -> Self {
        let mut browser = Self {
            current: start.clone(),
            entries: Vec::new(),
            back: Vec::new(),
            selected: None,
            error: None,
        };
        browser.current = start;
        browser.reload();
        browser
    }

    pub fn home() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"))
    }

    pub fn navigate(&mut self, path: PathBuf) {
        let path = match path.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                self.error = Some(format!("无法访问 {}：{error}", path.display()));
                return;
            }
        };
        if !path.is_dir() {
            self.error = Some(format!("不是目录：{}", path.display()));
            return;
        }
        if self.current != path {
            self.back.push(self.current.clone());
        }
        self.current = path;
        self.selected = None;
        self.reload();
    }

    pub fn back(&mut self) {
        if let Some(path) = self.back.pop() {
            self.current = path;
            self.selected = None;
            self.reload();
        }
    }

    pub fn up(&mut self) {
        if let Some(parent) = self.current.parent() {
            self.navigate(parent.to_path_buf());
        }
    }

    pub fn refresh(&mut self) {
        self.reload();
    }

    pub fn selected_file(&self) -> Option<&LocalEntry> {
        let selected = self.selected.as_ref()?;
        self.entries
            .iter()
            .find(|entry| &entry.path == selected && !entry.directory)
    }

    fn reload(&mut self) {
        self.error = None;
        let mut entries = Vec::new();
        match fs::read_dir(&self.current) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let metadata = fs::metadata(&path);
                    let directory = metadata.as_ref().is_ok_and(|metadata| metadata.is_dir());
                    let size = metadata
                        .as_ref()
                        .map(|metadata| {
                            if metadata.is_file() {
                                metadata.len()
                            } else {
                                0
                            }
                        })
                        .unwrap_or(0);
                    entries.push(LocalEntry {
                        path,
                        name,
                        directory,
                        size,
                    });
                }
            }
            Err(error) => {
                self.error = Some(format!("无法读取 {}：{error}", self.current.display()));
            }
        }
        entries.sort_by(|left, right| {
            right
                .directory
                .cmp(&left.directory)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        self.entries = entries;
    }
}

#[cfg(test)]
mod tests {
    use super::LocalBrowser;

    #[test]
    fn lists_directories_first_and_navigates_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("folder")).unwrap();
        std::fs::write(dir.path().join("note.txt"), "hello").unwrap();
        let mut browser = LocalBrowser::new(dir.path().to_path_buf());
        assert_eq!(browser.entries[0].name, "folder");
        assert!(browser.entries[0].directory);
        assert_eq!(browser.entries[1].name, "note.txt");
        browser.navigate(dir.path().join("folder"));
        assert_eq!(
            browser.current,
            dir.path().join("folder").canonicalize().unwrap()
        );
        browser.back();
        assert_eq!(browser.current, dir.path());
    }
}
