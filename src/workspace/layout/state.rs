use super::{Layout, Pane};
use crate::model::query::QueryPane;
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
    /// SQL query documents, keyed by pane id (database sessions only).
    #[serde(default)]
    pub queries: BTreeMap<u64, QueryPane>,
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
            queries: BTreeMap::new(),
        }
    }
}
impl WorkspaceState {
    pub fn query() -> Self {
        let mut state = Self {
            layout: Layout::query_pane(1),
            ..Self::default()
        };
        state.queries.insert(1, QueryPane::new("查询 1"));
        state
    }

    pub fn add_query(&mut self, id: u64) {
        self.queries
            .entry(id)
            .or_insert_with(|| QueryPane::new(format!("查询 {id}")));
    }

    pub fn terminal_number(&self, id: u64) -> usize {
        let mut ids = Vec::new();
        self.layout.ids(&mut ids);
        1 + ids
            .into_iter()
            .filter(|&other| {
                other < id
                    && self
                        .layout
                        .find(other)
                        .is_some_and(|pane| pane.terminal && !pane.terminal_closed)
            })
            .count()
    }

    pub fn pane_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        self.layout.ids(&mut ids);
        ids.extend(self.files.keys().copied());
        ids.extend(self.remote_files.keys().copied());
        for id in self.queries.keys().copied() {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }

    pub fn file_pane(&self, id: u64) -> Option<Pane> {
        if self.queries.contains_key(&id) {
            return Some(Pane {
                id,
                file: None,
                remote: None,
                showing_file: true,
                terminal: false,
                terminal_closed: false,
            });
        }
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
