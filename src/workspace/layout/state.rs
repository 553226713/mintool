use super::{Layout, Pane};
use crate::workspace::dock_layout::DockLayout;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct WorkspaceState {
    pub layout: Layout,
    pub dock_layout: Option<DockLayout>,
    pub focused: u64,
    pub next_id: u64,
    pub expanded: BTreeSet<PathBuf>,
    #[serde(default)]
    pub files: BTreeMap<u64, PathBuf>,
    /// Remote SFTP file tabs, keyed by pane id (SSH sessions only).
    #[serde(default)]
    pub remote_files: BTreeMap<u64, String>,
}
impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            layout: Layout::pane(1),
            dock_layout: None,
            focused: 1,
            next_id: 2,
            expanded: BTreeSet::new(),
            files: BTreeMap::new(),
            remote_files: BTreeMap::new(),
        }
    }
}
impl WorkspaceState {
    pub fn pane_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        self.layout.ids(&mut ids);
        ids.extend(self.files.keys().copied());
        ids.extend(self.remote_files.keys().copied());
        ids
    }

    pub fn file_pane(&self, id: u64) -> Option<Pane> {
        if let Some(path) = self.files.get(&id).cloned() {
            return Some(Pane {
                id,
                file: Some(path),
                remote: None,
                showing_file: true,
                terminal: false,
                terminal_closed: true,
            });
        }
        self.remote_files.get(&id).cloned().map(|path| Pane {
            id,
            file: None,
            remote: Some(path),
            showing_file: true,
            terminal: false,
            terminal_closed: true,
        })
    }

    pub fn migrate_nested_files(&mut self) {
        let mut nested = Vec::new();
        let mut ids = Vec::new();
        self.layout.ids(&mut ids);
        for id in ids {
            if let Some(path) = self.layout.find(id).and_then(|pane| pane.file.clone()) {
                nested.push((id, path));
            }
        }
        for (terminal_id, path) in nested {
            let file_id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            if let Some(pane) = self.layout.find_mut(terminal_id) {
                pane.file = None;
                pane.showing_file = false;
            }
            self.files.insert(file_id, path);
            if let Some(dock) = &mut self.dock_layout {
                dock.add_tab_after(terminal_id, file_id);
            }
        }
    }
}
