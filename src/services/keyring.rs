//! Optional credential storage through the system keyring (`secret-tool`).
//!
//! When the tool is unavailable the caller falls back to the existing
//! plaintext configuration behaviour.

use anyhow::{bail, Context, Result};
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::OnceLock,
};

const SERVICE: &str = "mintool-ssh";
static AVAILABLE: OnceLock<bool> = OnceLock::new();

pub fn available() -> bool {
    *AVAILABLE.get_or_init(|| {
        Command::new("secret-tool")
            .args(["lookup", "service", "__mintool_probe__"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    })
}

pub fn store(account: &str, secret: &str) -> Result<()> {
    let mut child = Command::new("secret-tool")
        .args([
            "store",
            "--label",
            "Mintool SSH",
            "service",
            SERVICE,
            "account",
            account,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("无法启动 secret-tool")?;
    child
        .stdin
        .take()
        .context("无法写入 secret-tool")?
        .write_all(secret.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        );
    }
    Ok(())
}

pub fn lookup(account: &str) -> Result<Option<String>> {
    let output = Command::new("secret-tool")
        .args(["lookup", "service", SERVICE, "account", account])
        .output()
        .context("无法启动 secret-tool")?;
    if !output.status.success() {
        return Ok(None);
    }
    let secret = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches(['\r', '\n'])
        .to_string();
    Ok((!secret.is_empty()).then_some(secret))
}

pub fn clear(account: &str) -> Result<()> {
    let output = Command::new("secret-tool")
        .args(["clear", "service", SERVICE, "account", account])
        .output()
        .context("无法启动 secret-tool")?;
    if !output.status.success() {
        bail!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        );
    }
    Ok(())
}
