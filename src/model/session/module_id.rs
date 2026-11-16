use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleId {
    #[default]
    Projects,
    Ssh,
    Docker,
    Database,
}

impl ModuleId {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Projects => "projects",
            Self::Ssh => "ssh",
            Self::Docker => "docker",
            Self::Database => "database",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Projects => "项目",
            Self::Ssh => "SSH",
            Self::Docker => "Docker",
            Self::Database => "数据库",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Self::Projects => "folder-closed",
            Self::Ssh => "square-terminal",
            Self::Docker => "docker",
            Self::Database => "file-database",
        }
    }
    pub fn has_aux_panel(self) -> bool {
        matches!(self, Self::Projects | Self::Ssh)
    }
}

pub const MODULES: [ModuleId; 3] = [ModuleId::Projects, ModuleId::Ssh, ModuleId::Docker];
