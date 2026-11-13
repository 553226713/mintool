use crate::{
    model::layout::WorkspaceState,
    services::{files::FileCache, git::GitSnapshot, terminal::TerminalSession},
};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

pub struct Workspace {
    pub state: WorkspaceState,
    pub terminals: HashMap<u64, Arc<Mutex<TerminalSession>>>,
    pub errors: HashMap<u64, String>,
    pub git: GitSnapshot,
    pub cache: FileCache,
}
impl Workspace {
    pub fn new(state: WorkspaceState) -> Self {
        Self {
            state,
            terminals: HashMap::new(),
            errors: HashMap::new(),
            git: GitSnapshot::default(),
            cache: FileCache::default(),
        }
    }
    pub fn relocate(&mut self, old: &Path, new: &Path) {
        self.terminals.clear();
        self.errors.clear();
        self.git = GitSnapshot::default();
        self.cache = FileCache::default();
        let mut ids = Vec::new();
        self.state.layout.ids(&mut ids);
        for id in ids {
            if let Some(pane) = self.state.layout.find_mut(id) {
                if let Some(file) = &pane.file {
                    if let Ok(relative) = file.strip_prefix(old) {
                        pane.file = Some(new.join(relative));
                    }
                }
            }
        }
        self.state.expanded = self
            .state
            .expanded
            .iter()
            .filter_map(|path| path.strip_prefix(old).ok().map(|path| new.join(path)))
            .collect();
    }
}
