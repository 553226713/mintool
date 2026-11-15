use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Pane {
    pub id: u64,
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub remote: Option<String>,
    pub showing_file: bool,
    #[serde(default = "default_true")]
    pub terminal: bool,
    #[serde(default)]
    pub terminal_closed: bool,
}

impl Pane {
    pub fn is_terminal(&self) -> bool {
        self.terminal && !self.showing_file && self.remote.is_none()
    }
}

fn default_true() -> bool {
    true
}
