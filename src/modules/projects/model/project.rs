use crate::model::session::{
    ModuleId, SessionCapabilities, SessionKey, SessionView, TerminalLauncher,
};
use crate::modules::common::grouped::GroupedItem;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub group: Option<String>,
    /// Optional command executed by every newly created terminal.
    #[serde(default)]
    pub run_command: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default)]
    pub last_opened: Option<u64>,
}
impl SessionView {
    /// Neutral workspace description owned by the projects module.
    pub fn project(project: &Project) -> Self {
        Self {
            key: SessionKey::new(ModuleId::Projects, project.id.clone()),
            title: project.name.clone(),
            subtitle: project.path.display().to_string(),
            cwd: project.path.clone(),
            launcher: TerminalLauncher::LocalShell,
            run_command: project.run_command.clone(),
            capabilities: SessionCapabilities::PROJECT,
        }
    }
}

impl GroupedItem for Project {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }
    fn set_group(&mut self, group: Option<String>) {
        self.group = group;
    }
}
