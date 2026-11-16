use super::sftp::{parse_listing, RemoteEntry};
use anyhow::{Context, Result};

/// Canonical paths reported by SFTP, independent of the login directory.
pub struct RemoteDirectory {
    pub home: String,
    pub path: String,
    pub entries: Vec<RemoteEntry>,
}

impl RemoteDirectory {
    pub fn parse(output: &str) -> Result<Self> {
        let mut paths = output.lines().filter_map(|line| {
            line.trim_end_matches('\r')
                .strip_prefix("Remote working directory: ")
        });
        let home = paths.next().context("SFTP 未返回主目录")?.to_string();
        let path = paths.next().context("SFTP 未返回当前目录")?.to_string();
        let entries = parse_listing(output, &path);
        Ok(Self {
            home,
            path,
            entries,
        })
    }
}
