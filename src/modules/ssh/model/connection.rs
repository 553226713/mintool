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
        }
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
