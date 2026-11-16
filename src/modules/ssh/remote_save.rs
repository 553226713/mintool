use super::{
    model::SshConnection,
    path::{basename, join_remote, parent_of},
    sftp,
};
use anyhow::{ensure, Context, Result};

/// Operations needed for a checked, same-directory replacement.
pub trait RemoteStore {
    fn read(&self, path: &str) -> Result<String>;
    fn permissions(&self, path: &str) -> Result<String>;
    fn write(&self, path: &str, text: &str) -> Result<()>;
    fn chmod(&self, path: &str, mode: &str) -> Result<()>;
    fn rename(&self, from: &str, to: &str) -> Result<()>;
    fn remove(&self, path: &str) -> Result<()>;
}
impl RemoteStore for SshConnection {
    fn read(&self, path: &str) -> Result<String> {
        read_text(self, path)
    }
    fn permissions(&self, path: &str) -> Result<String> {
        Ok(sftp::stat_file(self, path)?.permissions)
    }
    fn write(&self, path: &str, text: &str) -> Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        std::fs::write(file.path(), text)?;
        sftp::upload_to(self, file.path(), path)
    }
    fn chmod(&self, path: &str, mode: &str) -> Result<()> {
        sftp::chmod(self, path, mode)
    }
    fn rename(&self, from: &str, to: &str) -> Result<()> {
        sftp::rename(self, from, to)
    }
    fn remove(&self, path: &str) -> Result<()> {
        sftp::remove_remote(self, path)
    }
}

pub fn read_text(connection: &SshConnection, remote: &str) -> Result<String> {
    let local = sftp::download_to_temp(connection, remote)?;
    let result = std::fs::read_to_string(&local).context("远程文件不是 UTF-8 文本");
    let _ = std::fs::remove_file(local);
    result
}

pub fn save(
    connection: &SshConnection,
    remote: &str,
    original: &str,
    replacement: &str,
) -> Result<()> {
    save_with_store(connection, remote, original, replacement)
}

pub fn save_with_store(
    store: &impl RemoteStore,
    remote: &str,
    original: &str,
    replacement: &str,
) -> Result<()> {
    ensure!(
        store.read(remote)? == original,
        "远端文件已被其他操作修改，请先重新打开并合并修改"
    );
    let permissions = store.permissions(remote)?;
    let temporary = join_remote(
        parent_of(remote),
        &format!(".{}.mintool-{}.tmp", basename(remote), uuid::Uuid::new_v4()),
    );
    let result = (|| {
        store.write(&temporary, replacement)?;
        if let Some(mode) = permission_mode(&permissions) {
            store.chmod(&temporary, &mode)?;
        }
        ensure!(
            store.read(remote)? == original,
            "上传期间远端文件发生变化，已停止覆盖"
        );
        store.rename(&temporary, remote)
    })();
    if result.is_err() {
        let _ = store.remove(&temporary);
    }
    result
}

pub fn permission_mode(permissions: &str) -> Option<String> {
    let bytes = permissions.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    let mut mode = 0u16;
    for (i, byte) in bytes[1..10].iter().enumerate() {
        if matches!(*byte, b'r' | b'w' | b'x' | b's' | b't') {
            mode |= 1 << (8 - i);
        }
    }
    if matches!(bytes[3], b's' | b'S') {
        mode |= 0o4000;
    }
    if matches!(bytes[6], b's' | b'S') {
        mode |= 0o2000;
    }
    if matches!(bytes[9], b't' | b'T') {
        mode |= 0o1000;
    }
    Some(format!("{mode:o}"))
}
