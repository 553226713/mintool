use crate::modules::common::grouped::GroupedItem;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

fn default_port() -> u16 {
    22
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SshConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub identity: Option<PathBuf>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub extra_args: Vec<String>,
    #[serde(default)]
    pub color: Option<String>,
    /// Saved forwards: `8080:localhost:80`, `R:9000:localhost:9000`, `D:1080`.
    #[serde(default)]
    pub forwards: Vec<String>,
    /// Password is stored in the system keyring instead of this file.
    #[serde(default)]
    pub use_keyring: bool,
    /// Runtime lookup result, never serialized.
    #[serde(skip)]
    pub resolved_password: Option<String>,
}
impl SshConnection {
    pub fn new(name: &str, host: &str, group: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            host: host.into(),
            port: default_port(),
            user: String::new(),
            password: None,
            identity: None,
            group,
            extra_args: Vec::new(),
            color: None,
            forwards: Vec::new(),
            use_keyring: false,
            resolved_password: None,
        }
    }
    pub fn password_value(&self) -> Option<&str> {
        self.resolved_password
            .as_deref()
            .or(self.password.as_deref())
    }

    pub fn target(&self) -> String {
        if self.user.is_empty() {
            self.host.clone()
        } else {
            format!("{}@{}", self.user, self.host)
        }
    }
    pub fn display_address(&self) -> String {
        if self.port == 22 {
            self.target()
        } else {
            format!("{}:{}", self.target(), self.port)
        }
    }
}
impl GroupedItem for SshConnection {
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
