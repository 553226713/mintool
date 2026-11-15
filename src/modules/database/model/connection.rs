use crate::model::session::{
    ModuleId, SessionCapabilities, SessionKey, SessionSurface, SessionView as NeutralSessionView,
    TerminalLauncher,
};
use crate::modules::common::grouped::GroupedItem;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseEngine {
    #[default]
    Sqlite,
    Postgres,
    Mysql,
    Mariadb,
}

impl DatabaseEngine {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sqlite => "SQLite",
            Self::Postgres => "PostgreSQL",
            Self::Mysql => "MySQL",
            Self::Mariadb => "MariaDB",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Sqlite => "file-database",
            Self::Postgres | Self::Mysql | Self::Mariadb => "file-database",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Self::Sqlite => 0,
            Self::Postgres => 5432,
            Self::Mysql | Self::Mariadb => 3306,
        }
    }

    pub fn is_file(self) -> bool {
        self == Self::Sqlite
    }

    pub const ALL: [Self; 4] = [Self::Sqlite, Self::Postgres, Self::Mysql, Self::Mariadb];
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DatabaseConnection {
    pub id: String,
    pub name: String,
    pub engine: DatabaseEngine,
    pub path: PathBuf,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: Option<String>,
}

impl DatabaseConnection {
    pub fn subtitle(&self) -> String {
        match self.engine {
            DatabaseEngine::Sqlite => self.path.display().to_string(),
            DatabaseEngine::Postgres | DatabaseEngine::Mysql | DatabaseEngine::Mariadb => {
                let user = if self.user.is_empty() {
                    String::new()
                } else {
                    format!("{}@", self.user)
                };
                let database = if self.database.is_empty() {
                    String::new()
                } else {
                    format!("/{}", self.database)
                };
                format!("{user}{}{database}", self.host)
            }
        }
    }

    pub fn query_database(&self) -> String {
        if self.database.is_empty() {
            match self.engine {
                DatabaseEngine::Postgres => "postgres".into(),
                DatabaseEngine::Mysql | DatabaseEngine::Mariadb => "information_schema".into(),
                DatabaseEngine::Sqlite => String::new(),
            }
        } else {
            self.database.clone()
        }
    }
}

impl GroupedItem for DatabaseConnection {
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

impl NeutralSessionView {
    /// Query workspace owned by the database module.
    pub fn database(connection: &DatabaseConnection) -> Self {
        Self {
            key: SessionKey::new(ModuleId::Database, connection.id.clone()),
            title: connection.name.clone(),
            subtitle: format!("{} · {}", connection.engine.label(), connection.subtitle()),
            cwd: if connection.engine.is_file() {
                connection
                    .path
                    .parent()
                    .unwrap_or_else(|| Path::new("/"))
                    .to_path_buf()
            } else {
                crate::services::desktop::home_dir()
            },
            launcher: TerminalLauncher::LocalShell,
            surface: SessionSurface::Query,
            capabilities: SessionCapabilities::DATABASE,
            run_command: None,
        }
    }
}

pub fn expand_path(input: &str) -> PathBuf {
    let trimmed = input.trim();
    if trimmed == "~" {
        return crate::services::desktop::home_dir();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return crate::services::desktop::home_dir().join(rest);
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        path
    } else {
        crate::services::desktop::home_dir().join(path)
    }
}
