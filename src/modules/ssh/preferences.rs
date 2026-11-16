use crate::{model::session::ModuleId, services::storage::Storage};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrowserPreferences {
    pub remote: String,
    pub local: Option<PathBuf>,
    pub bookmarks: Vec<String>,
    pub local_bookmarks: Vec<PathBuf>,
    pub recent_remote: Vec<String>,
    pub recent_local: Vec<PathBuf>,
    pub show_hidden: bool,
    pub sort: String,
    pub descending: bool,
    pub local_visible: bool,
}

pub type SavedBrowsers = BTreeMap<String, BrowserPreferences>;
pub fn load(storage: &Storage) -> anyhow::Result<SavedBrowsers> {
    storage.read_json(&storage.module_dir(ModuleId::Ssh).join("browsers.json"))
}

impl BrowserPreferences {
    pub fn remember_remote(&mut self, path: &str) -> bool {
        if self
            .recent_remote
            .first()
            .is_some_and(|recent| recent == path)
        {
            return false;
        }
        self.recent_remote.retain(|recent| recent != path);
        self.recent_remote.insert(0, path.into());
        self.recent_remote.truncate(20);
        true
    }
    pub fn remember_local(&mut self, path: &std::path::Path) -> bool {
        if self
            .recent_local
            .first()
            .is_some_and(|recent| recent == path)
        {
            return false;
        }
        self.recent_local.retain(|recent| recent != path);
        self.recent_local.insert(0, path.into());
        self.recent_local.truncate(20);
        true
    }
}
