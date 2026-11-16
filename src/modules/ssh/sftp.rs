use super::model::SshConnection;
use super::path::{basename, parent_of};
use anyhow::{bail, Context, Result};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

/// Metadata operations should fail fast; transfers may legitimately run for a
/// long time and must not be killed after 20 seconds.
const OP_TIMEOUT_SECONDS: &str = "20";
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(3600);

#[derive(Clone, Debug)]
pub struct RemoteEntry {
    /// Path relative to the login home directory.
    pub path: String,
    pub name: String,
    pub directory: bool,
    pub size: u64,
    pub permissions: String,
    pub owner: String,
    pub group: String,
    pub modified: String,
}

fn quote(path: &str) -> String {
    format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Shared byte counters for the transfer progress bar.
pub struct TransferProgress {
    total: AtomicU64,
    transferred: AtomicU64,
    initial: AtomicU64,
    process_group: AtomicU32,
    track: AtomicBool,
}

impl Default for TransferProgress {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TransferProgress {
    pub fn new(total: u64) -> Self {
        Self {
            total: AtomicU64::new(total),
            transferred: AtomicU64::new(0),
            initial: AtomicU64::new(0),
            process_group: AtomicU32::new(0),
            track: AtomicBool::new(true),
        }
    }
    pub fn untracked() -> Self {
        let progress = Self::new(0);
        progress.track.store(false, Ordering::Relaxed);
        progress
    }
    pub fn tracked(&self) -> bool {
        self.track.load(Ordering::Relaxed)
    }
    pub fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }
    pub fn set_transferred(&self, bytes: u64) {
        self.transferred.store(bytes, Ordering::Relaxed);
    }
    pub fn finish(&self, total: u64) {
        self.set_total(total);
        self.set_transferred(total);
    }
    pub fn transferred(&self) -> u64 {
        self.transferred.load(Ordering::Relaxed)
    }
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }
    pub fn set_initial(&self, bytes: u64) {
        self.initial.store(bytes, Ordering::Relaxed);
        self.set_transferred(bytes);
    }
    pub fn transferred_since_start(&self) -> u64 {
        self.transferred()
            .saturating_sub(self.initial.load(Ordering::Relaxed))
    }
    pub fn terminate_process(&self) {
        let pid = self.process_group.swap(0, Ordering::SeqCst);
        if pid > 0 {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
    pub fn percent(&self) -> Option<u8> {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return None;
        }
        let transferred = self.transferred.load(Ordering::Relaxed).min(total);
        Some(((transferred.saturating_mul(100) / total).min(100)) as u8)
    }
}

fn spawn_progress(
    progress: Arc<TransferProgress>,
    done: Arc<AtomicBool>,
    poll: impl Fn() -> u64 + Send + 'static,
) {
    std::thread::spawn(move || {
        while !done.load(Ordering::Relaxed) {
            progress.set_transferred(poll());
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

fn run(connection: &SshConnection, commands: &[String]) -> Result<String> {
    run_with_timeout(connection, commands, OP_TIMEOUT_SECONDS, true)
}

fn sftp_args(connection: &SshConnection) -> Result<Vec<String>> {
    super::transport::sftp_arguments(connection)
}

fn configure_askpass(command: &mut Command, connection: &SshConnection) {
    command.envs(super::transport::authentication_environment(connection));
}

fn finish(output: Output) -> Result<String> {
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

fn spawn_sftp(connection: &SshConnection, program: &str) -> Result<Command> {
    let mut command = Command::new(program);
    command.args(["-b", "-"]);
    if connection.password_value().is_some() {
        command.arg("-oBatchMode=no");
    }
    command.args(sftp_args(connection)?);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_askpass(&mut command, connection);
    Ok(command)
}

fn write_commands(child: &mut std::process::Child, commands: &[String]) -> Result<()> {
    let mut stdin = child.stdin.take().context("无法写入 sftp")?;
    for command in commands {
        writeln!(stdin, "{command}")?;
    }
    writeln!(stdin, "quit")?;
    Ok(())
}

fn run_with_timeout(
    connection: &SshConnection,
    commands: &[String],
    timeout_seconds: &str,
    batch: bool,
) -> Result<String> {
    let timeout = if Path::new("/usr/bin/timeout").exists() {
        "/usr/bin/timeout"
    } else {
        "timeout"
    };
    let mut command = Command::new(timeout);
    command.arg(timeout_seconds).arg("sftp");
    if batch {
        command.args(["-b", "-"]);
        if connection.password_value().is_some() {
            command.arg("-oBatchMode=no");
        }
    }
    command.args(sftp_args(connection)?);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_askpass(&mut command, connection);
    let mut child = command.spawn().context("无法启动 sftp")?;
    write_commands(&mut child, commands)?;
    finish(child.wait_with_output()?)
}

fn run_transfer(
    connection: &SshConnection,
    commands: &[String],
    cancel: &AtomicBool,
    progress: &TransferProgress,
) -> Result<String> {
    use std::os::unix::process::CommandExt;
    if cancel.load(Ordering::Relaxed) {
        bail!("传输已取消");
    }
    let mut command = spawn_sftp(connection, "sftp")?;
    command.process_group(0);
    let child = command.spawn().context("无法启动 sftp")?;
    progress.process_group.store(child.id(), Ordering::SeqCst);
    let result = wait_transfer(child, commands, cancel);
    progress.process_group.store(0, Ordering::SeqCst);
    result
}

fn kill_transfer(child: &mut std::process::Child) {
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.wait();
}
fn wait_transfer(
    mut child: std::process::Child,
    commands: &[String],
    cancel: &AtomicBool,
) -> Result<String> {
    let mut stdout = child.stdout.take().context("无法读取 sftp 输出")?;
    let mut stderr = child.stderr.take().context("无法读取 sftp 错误")?;
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    if let Err(error) = write_commands(&mut child, commands) {
        kill_transfer(&mut child);
        let _ = out.join();
        let _ = err.join();
        return Err(error);
    }
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            kill_transfer(&mut child);
            let _ = out.join();
            let _ = err.join();
            bail!("传输已取消");
        }
        if start.elapsed() >= TRANSFER_TIMEOUT {
            kill_transfer(&mut child);
            let _ = out.join();
            let _ = err.join();
            bail!("传输超时");
        }
        if child.try_wait()?.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let status = child.wait()?;
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("读取 sftp 输出失败"))??;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("读取 sftp 错误失败"))??;
    finish(Output {
        status,
        stdout,
        stderr,
    })
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

pub fn read_directory(
    connection: &SshConnection,
    parent: &str,
) -> Result<super::directory::RemoteDirectory> {
    let path = if parent.is_empty() { "." } else { parent };
    let output = run_with_timeout(
        connection,
        &[
            "pwd".into(),
            format!("cd {}", quote(path)),
            "pwd".into(),
            "ls -la .".into(),
        ],
        OP_TIMEOUT_SECONDS,
        true,
    )?;
    super::directory::RemoteDirectory::parse(&output)
}

pub fn stat_file(connection: &SshConnection, remote: &str) -> Result<RemoteEntry> {
    let output = run(connection, &[format!("ls -l {}", quote(remote))])?;
    parse_listing(&output, parent_of(remote))
        .into_iter()
        .find(|entry| entry.path == remote || entry.name == basename(remote))
        .with_context(|| format!("远程文件不存在：{remote}"))
}

pub fn upload_to(connection: &SshConnection, local: &Path, remote: &str) -> Result<()> {
    upload_to_with_progress(
        connection,
        local,
        remote,
        Arc::new(AtomicBool::new(false)),
        Arc::new(TransferProgress::untracked()),
    )
}

pub fn upload_to_with_progress(
    connection: &SshConnection,
    local: &Path,
    remote: &str,
    cancel: Arc<AtomicBool>,
    progress: Arc<TransferProgress>,
) -> Result<()> {
    upload_impl(connection, local, remote, cancel, progress, false)
}

pub fn resume_upload(
    connection: &SshConnection,
    local: &Path,
    remote: &str,
    cancel: Arc<AtomicBool>,
    progress: Arc<TransferProgress>,
) -> Result<()> {
    upload_impl(connection, local, remote, cancel, progress, true)
}

fn upload_impl(
    connection: &SshConnection,
    local: &Path,
    remote: &str,
    cancel: Arc<AtomicBool>,
    progress: Arc<TransferProgress>,
    resume: bool,
) -> Result<()> {
    let tracked = progress.tracked();
    let total = std::fs::metadata(local)
        .ok()
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    progress.set_total(total);
    if resume {
        progress.set_initial(stat_file(connection, remote)?.size.min(total));
    }
    let done = Arc::new(AtomicBool::new(false));
    if tracked && total > 0 {
        let poll_connection = connection.clone();
        let poll_remote = remote.to_string();
        let poll_progress = progress.clone();
        let poll_done = done.clone();
        spawn_progress(poll_progress, poll_done, move || {
            stat_file(&poll_connection, &poll_remote)
                .map(|entry| entry.size)
                .unwrap_or(0)
        });
    }
    let recursive = if local.is_dir() { "-R " } else { "" };
    let result = run_transfer(
        connection,
        &[format!(
            "{} {recursive}{} {}",
            if resume { "reput" } else { "put" },
            quote(&local.display().to_string()),
            quote(remote)
        )],
        &cancel,
        &progress,
    );
    done.store(true, Ordering::Relaxed);
    let output = result?;
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
    progress.finish(total);
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

pub fn remove_directory(connection: &SshConnection, remote: &str) -> Result<()> {
    let output = run(connection, &[format!("rmdir {}", quote(remote))])?;
    let lower = output.to_lowercase();
    if lower.contains("can't remove") || lower.contains("not found") || lower.contains("not empty")
    {
        bail!("无法删除远程目录（目录必须为空）");
    }
    Ok(())
}

pub fn mkdir(connection: &SshConnection, remote: &str) -> Result<()> {
    let output = run(connection, &[format!("mkdir {}", quote(remote))])?;
    let lower = output.to_lowercase();
    if lower.contains("can't mkdir")
        || lower.contains("failure")
        || lower.contains("permission denied")
    {
        bail!("无法新建远程目录");
    }
    Ok(())
}

pub fn rename(connection: &SshConnection, from: &str, to: &str) -> Result<()> {
    let output = run(
        connection,
        &[format!("rename {} {}", quote(from), quote(to))],
    )?;
    let lower = output.to_lowercase();
    if lower.contains("couldn't")
        || lower.contains("failure")
        || lower.contains("not found")
        || lower.contains("permission denied")
    {
        bail!("无法重命名远程文件");
    }
    Ok(())
}

pub fn chmod(connection: &SshConnection, remote: &str, mode: &str) -> Result<()> {
    anyhow::ensure!(
        (3..=4).contains(&mode.len()) && mode.chars().all(|c| ('0'..='7').contains(&c)),
        "权限应为 3 或 4 位八进制数字，例如 644"
    );
    let output = run(connection, &[format!("chmod {mode} {}", quote(remote))])?;
    let lower = output.to_lowercase();
    if lower.contains("failure")
        || lower.contains("not found")
        || lower.contains("permission denied")
    {
        bail!("无法修改远程文件权限");
    }
    Ok(())
}

pub fn download_to(connection: &SshConnection, remote: &str, local: &Path) -> Result<()> {
    download_to_with_progress(
        connection,
        remote,
        local,
        Arc::new(AtomicBool::new(false)),
        Arc::new(TransferProgress::untracked()),
    )
}

pub fn download_to_with_progress(
    connection: &SshConnection,
    remote: &str,
    local: &Path,
    cancel: Arc<AtomicBool>,
    progress: Arc<TransferProgress>,
) -> Result<()> {
    download_impl(connection, remote, local, cancel, progress, false)
}

pub fn resume_download(
    connection: &SshConnection,
    remote: &str,
    local: &Path,
    cancel: Arc<AtomicBool>,
    progress: Arc<TransferProgress>,
) -> Result<()> {
    download_impl(connection, remote, local, cancel, progress, true)
}

fn download_impl(
    connection: &SshConnection,
    remote: &str,
    local: &Path,
    cancel: Arc<AtomicBool>,
    progress: Arc<TransferProgress>,
    resume: bool,
) -> Result<()> {
    let tracked = progress.tracked();
    let total = if tracked {
        stat_file(connection, remote)
            .ok()
            .filter(|entry| !entry.directory)
            .map(|entry| entry.size)
            .unwrap_or(0)
    } else {
        0
    };
    progress.set_total(total);
    if resume {
        progress.set_initial(
            std::fs::metadata(local)
                .map(|metadata| metadata.len())
                .unwrap_or(0)
                .min(total),
        );
    }
    let done = Arc::new(AtomicBool::new(false));
    if tracked && total > 0 {
        let poll_local = local.to_path_buf();
        let poll_progress = progress.clone();
        let poll_done = done.clone();
        spawn_progress(poll_progress, poll_done, move || {
            std::fs::metadata(&poll_local)
                .map(|metadata| metadata.len())
                .unwrap_or(0)
        });
    }
    let result = run_transfer(
        connection,
        &[format!(
            "{} -R {} {}",
            if resume { "reget" } else { "get" },
            quote(remote),
            quote(&local.display().to_string())
        )],
        &cancel,
        &progress,
    );
    done.store(true, Ordering::Relaxed);
    let output = result?;
    let lower = output.to_lowercase();
    if !local.exists()
        || lower.contains("can't get")
        || lower.contains("not found")
        || lower.contains("no such file")
    {
        bail!("无法下载远程文件");
    }
    progress.finish(total);
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
            permissions: permissions.into(),
            owner: fields[2].into(),
            group: fields[3].into(),
            modified: fields[5..8].join(" "),
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

#[cfg(test)]
mod process_tests {
    use super::wait_transfer;
    use std::{
        os::unix::process::CommandExt,
        process::{Command, Stdio},
        sync::atomic::AtomicBool,
    };
    #[test]
    fn transfer_reads_output_while_child_is_running() {
        let child = Command::new("python3")
            .args([
                "-c",
                "import sys; sys.stdout.write('x'*262144); sys.stderr.write('y'*262144)",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let result = wait_transfer(child, &[], &AtomicBool::new(false)).unwrap();
        assert_eq!(result.len(), 262144);
    }
    #[test]
    fn cancelling_transfer_reaps_its_process_group() {
        let child = Command::new("/bin/sh")
            .args(["-c", "sleep 60"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        assert!(wait_transfer(child, &[], &AtomicBool::new(true)).is_err());
        assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
    }
}
