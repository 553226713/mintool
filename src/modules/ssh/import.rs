use super::model::{SshConnection, SshRegistry};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

struct Pending {
    alias: String,
    user: Option<String>,
    port: Option<u16>,
    identity: Option<PathBuf>,
}

/// Import concrete `Host` entries from an OpenSSH config file.
///
/// Wildcard-only blocks such as `Host *` are skipped. Existing connections
/// with the same host and port are left untouched.
pub fn import_config(registry: &mut SshRegistry, path: &Path) -> Result<usize> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let mut pending: Vec<Pending> = Vec::new();
    let mut current: Option<Pending> = None;
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let Some(key) = parts.next() else {
            continue;
        };
        let value = parts.next().unwrap_or_default().trim();
        if key.eq_ignore_ascii_case("Host") {
            if let Some(block) = current.take() {
                pending.push(block);
            }
            let alias = value
                .split_whitespace()
                .find(|pattern| !pattern.contains('*') && !pattern.contains('?'));
            if let Some(alias) = alias {
                current = Some(Pending {
                    alias: alias.to_string(),
                    user: None,
                    port: None,
                    identity: None,
                });
            }
            continue;
        }
        let Some(block) = current.as_mut() else {
            continue;
        };
        match key.to_ascii_lowercase().as_str() {
            "user" => block.user = Some(value.to_string()),
            "port" => block.port = value.parse().ok(),
            "identityfile" => block.identity = Some(expand_home(value)),
            _ => {}
        }
    }
    if let Some(block) = current {
        pending.push(block);
    }

    let mut imported = 0;
    for block in pending {
        // Keep the alias as the connection target so `ssh` applies the whole
        // config block (ProxyJump, ProxyCommand, ...), not just these fields.
        let host = block.alias.clone();
        let port = block.port.unwrap_or(22);
        if registry
            .connections
            .iter()
            .any(|connection| connection.host == host && connection.port == port)
        {
            continue;
        }
        let mut connection = SshConnection::new(&block.alias, &host, None);
        connection.user = block.user.unwrap_or_default();
        connection.port = port;
        connection.identity = block.identity;
        registry.add(connection);
        imported += 1;
    }
    Ok(imported)
}

fn expand_home(value: &str) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(value)
}
