use super::model::SshConnection;
use super::path::{basename, parent_of};
use anyhow::{bail, Context, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const TIMEOUT_SECONDS: &str = "20";

#[derive(Clone, Debug)]
pub struct RemoteEntry {
    /// Path relative to the login home directory.
    pub path: String,
    pub name: String,
    pub directory: bool,
    pub size: u64,
}

fn quote(path: &str) -> String {
    format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}

fn run(connection: &SshConnection, commands: &[String]) -> Result<String> {
    let timeout = if Path::new("/usr/bin/timeout").exists() {
        "/usr/bin/timeout"
    } else {
        "timeout"
    };
    let mut command = Command::new(timeout);
    command.arg(TIMEOUT_SECONDS).arg("sftp").arg("-q");
    if connection.port != 22 {
        command.arg("-P").arg(connection.port.to_string());
    }
    if let Some(identity) = &connection.identity {
        command.arg("-i").arg(identity);
    }
    command.args([
        "-o",
        "ConnectTimeout=8",
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        "NumberOfPasswordPrompts=1",
        "-o",
        "LogLevel=ERROR",
    ]);
    if connection.password.is_none() {
        command.args(["-o", "BatchMode=yes"]);
    }
    command.arg(connection.target());
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(password) = &connection.password {
        if let Some(executable) = super::session::askpass_executable() {
            command
                .env("SSH_ASKPASS", executable)
                .env("SSH_ASKPASS_REQUIRE", "force")
                .env("MINTOOL_ASKPASS", "1")
                .env("MINTOOL_ASKPASS_PASSWORD", password);
        }
    }
    let mut child = command.spawn().context("无法启动 sftp")?;
    {
        let mut stdin = child.stdin.take().context("无法写入 sftp")?;
        for command in commands {
            writeln!(stdin, "{command}")?;
        }
        writeln!(stdin, "quit")?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let message = if !stderr.is_empty() { stderr } else { stdout };
        if output.status.code() == Some(124) {
            bail!("远程文件操作超时");
        }
        bail!(
            "{}",
            message.lines().take(3).collect::<Vec<_>>().join(" / ")
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn list_directory(connection: &SshConnection, parent: &str) -> Result<Vec<RemoteEntry>> {
    let listing_path = if parent.is_empty() { "." } else { parent };
    let output = run(connection, &[format!("ls -l {}", quote(listing_path))])?;
    let entries = parse_listing(&output, parent);
    if entries.is_empty() {
        let lower = output.to_lowercase();
        if lower.contains("can't ls")
            || lower.contains("not found")
            || lower.contains("no such file")
        {
            bail!("无法读取远程目录");
        }
    }
    Ok(entries)
}

pub fn stat_file(connection: &SshConnection, remote: &str) -> Result<RemoteEntry> {
    let output = run(connection, &[format!("ls -l {}", quote(remote))])?;
    parse_listing(&output, parent_of(remote))
        .into_iter()
        .find(|entry| entry.path == remote || entry.name == basename(remote))
        .with_context(|| format!("远程文件不存在：{remote}"))
}

pub fn upload_to(connection: &SshConnection, local: &Path, remote: &str) -> Result<()> {
    let output = run(
        connection,
        &[format!(
            "put {} {}",
            quote(&local.display().to_string()),
            quote(remote)
        )],
    )?;
    let lower = output.to_lowercase();
    if lower.contains("can't put")
        || lower.contains("not found")
        || lower.contains("permission denied")
        || lower.contains("no such file")
    {
        bail!(
            "无法上传文件：{}",
            output.lines().last().unwrap_or("权限或路径错误")
        );
    }
    Ok(())
}

pub fn remove_remote(connection: &SshConnection, remote: &str) -> Result<()> {
    let output = run(connection, &[format!("rm {}", quote(remote))])?;
    let lower = output.to_lowercase();
    if lower.contains("can't remove") || lower.contains("not found") {
        bail!("无法删除远程文件");
    }
    Ok(())
}

pub fn download_to(connection: &SshConnection, remote: &str, local: &Path) -> Result<()> {
    let output = run(
        connection,
        &[format!(
            "get {} {}",
            quote(remote),
            quote(&local.display().to_string())
        )],
    )?;
    let lower = output.to_lowercase();
    if !local.exists()
        || lower.contains("can't get")
        || lower.contains("not found")
        || lower.contains("no such file")
    {
        let _ = std::fs::remove_file(local);
        bail!("无法下载远程文件");
    }
    Ok(())
}

pub fn download_to_temp(connection: &SshConnection, remote: &str) -> Result<PathBuf> {
    let directory = std::env::temp_dir().join("mintool-remote");
    std::fs::create_dir_all(&directory)?;
    let local = directory.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    download_to(connection, remote, &local)?;
    Ok(local)
}

pub fn parse_listing(output: &str, parent: &str) -> Vec<RemoteEntry> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let line = line.trim_end_matches('\r');
        let Some((fields, rest)) = split_fields(line, 8) else {
            continue;
        };
        let permissions = fields[0];
        let Some(kind) = permissions.chars().next() else {
            continue;
        };
        if !matches!(kind, '-' | 'd' | 'l' | 'c' | 'b' | 'p' | 's') {
            continue;
        }
        let name_part = rest.split(" -> ").next().unwrap_or(rest).trim();
        if name_part.is_empty() || name_part == "." || name_part == ".." {
            continue;
        }
        let name = basename(name_part).to_string();
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        let size = fields[4].parse::<u64>().unwrap_or(0);
        let path = if parent.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", parent.trim_end_matches('/'), name)
        };
        entries.push(RemoteEntry {
            path,
            name,
            directory: kind == 'd',
            size,
        });
    }
    entries.sort_by(|left, right| {
        right
            .directory
            .cmp(&left.directory)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    entries
}

fn split_fields(line: &str, count: usize) -> Option<(Vec<&str>, &str)> {
    let bytes = line.as_bytes();
    let mut fields = Vec::with_capacity(count);
    let mut index = 0;
    while fields.len() < count {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        let start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        fields.push(&line[start..index]);
    }
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    Some((fields, &line[index..]))
}

#[cfg(test)]
mod tests {
    use super::parse_listing;

    #[test]
    fn parses_directories_files_spaces_and_symlinks() {
        let output = "\
sftp> ls -l .
-rw-r--r--    ? root     tester         25 Nov 15 00:09 ./a file.txt\r
lrwxrwxrwx    ? root     tester         23 Nov 15 00:09 ./link.txt -> ./a file.txt\r
drwxr-sr-x    ? root     tester       4096 Nov 15 00:09 ./sub\r
-rw-r--r--    ? root     tester         13 Nov 15 00:09 ./中文.txt\r
sftp> quit\r
";
        let entries = parse_listing(output, "");
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].name, "sub");
        assert!(entries[0].directory);
        assert_eq!(entries[1].name, "a file.txt");
        assert_eq!(entries[1].path, "a file.txt");
        assert_eq!(entries[1].size, 25);
        assert!(entries.iter().any(|entry| entry.name == "中文.txt"));
        assert!(entries.iter().any(|entry| entry.name == "link.txt"));
    }

    #[test]
    fn builds_nested_paths() {
        let output = "-rw-r--r-- 1 user group 12 Jan 1 00:00 ./inner.txt\n";
        let entries = parse_listing(output, "sub");
        assert_eq!(entries[0].path, "sub/inner.txt");
        assert_eq!(entries[0].name, "inner.txt");
    }
}
