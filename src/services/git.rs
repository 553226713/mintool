use super::file_changes::{FileChanges, LineChange};
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Default, Debug)]
pub struct GitSnapshot {
    pub root: Option<PathBuf>,
    pub branch: String,
    pub lines: BTreeMap<PathBuf, Vec<LineChange>>,
    pub changes: BTreeMap<PathBuf, String>,
    pub error: Option<String>,
}
impl GitSnapshot {
    pub fn scan(path: &Path) -> Self {
        let output = match Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "--show-toplevel"])
            .output()
        {
            Ok(o) if o.status.success() => o,
            Ok(_) => return Self::default(),
            Err(e) => {
                return Self {
                    error: Some(e.to_string()),
                    ..Self::default()
                }
            }
        };
        let root = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim_end());
        let branch = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().into())
            .unwrap_or_else(|| {
                Command::new("git")
                    .arg("-C")
                    .arg(&root)
                    .args(["rev-parse", "--short", "HEAD"])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| format!("detached {}", String::from_utf8_lossy(&o.stdout).trim()))
                    .unwrap_or_else(|| "HEAD".into())
            });
        let mut snapshot = Self {
            root: Some(root.clone()),
            branch,
            ..Self::default()
        };
        match Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
        {
            Ok(o) if o.status.success() => snapshot.changes = Self::parse_status(&root, &o.stdout),
            Ok(o) => snapshot.error = Some(String::from_utf8_lossy(&o.stderr).trim().into()),
            Err(e) => snapshot.error = Some(e.to_string()),
        }
        let has_head = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["rev-parse", "--verify", "HEAD"])
            .output()
            .is_ok_and(|o| o.status.success());
        for (path, status) in &snapshot.changes {
            snapshot.lines.insert(
                path.clone(),
                FileChanges::scan(
                    &root,
                    path,
                    status == "??" || status.starts_with('A') || !has_head,
                    has_head,
                ),
            );
        }
        snapshot
    }
    pub fn parse_status(root: &Path, bytes: &[u8]) -> BTreeMap<PathBuf, String> {
        let mut changes = BTreeMap::new();
        let mut entries = bytes.split(|b| *b == 0);
        while let Some(entry) = entries.next() {
            if entry.len() < 4 {
                continue;
            }
            let status = String::from_utf8_lossy(&entry[..2]).into_owned();
            #[cfg(unix)]
            let path = {
                use std::os::unix::ffi::OsStrExt;
                PathBuf::from(std::ffi::OsStr::from_bytes(&entry[3..]))
            };
            #[cfg(not(unix))]
            let path = PathBuf::from(String::from_utf8_lossy(&entry[3..]).as_ref());
            changes.insert(root.join(path), status.clone());
            if status.contains('R') || status.contains('C') {
                entries.next();
            }
        }
        changes
    }
    pub fn status(&self, path: &Path) -> Option<&str> {
        self.changes.get(path).map(String::as_str).or_else(|| {
            self.changes
                .iter()
                .find(|(p, _)| p.starts_with(path))
                .map(|(_, s)| s.as_str())
        })
    }
    /// Staged changes are the first status column (untracked `??` is not staged).
    pub fn is_staged(&self, path: &Path) -> bool {
        self.status(path)
            .is_some_and(|status| !status.starts_with(' ') && !status.starts_with('?'))
    }
    /// Worktree changes are the second column; untracked files count as such.
    pub fn is_unstaged(&self, path: &Path) -> bool {
        self.status(path).is_some_and(|status| {
            status == "??" || status.chars().nth(1).is_some_and(|state| state != ' ')
        })
    }
}

fn run_git(root: &Path, args: &[&std::ffi::OsStr]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .context("无法启动 git")?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "{}",
            message.lines().take(3).collect::<Vec<_>>().join(" / ")
        );
    }
    Ok(())
}

fn relative<'a>(root: &Path, path: &'a Path) -> &'a std::ffi::OsStr {
    path.strip_prefix(root).unwrap_or(path).as_os_str()
}

pub fn stage(root: &Path, paths: &[PathBuf]) -> Result<()> {
    let mut args = vec![
        std::ffi::OsString::from("add"),
        std::ffi::OsString::from("--"),
    ];
    args.extend(paths.iter().map(|path| relative(root, path).to_owned()));
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|arg| arg.as_os_str()).collect();
    run_git(root, &refs)
}

pub fn unstage(root: &Path, paths: &[PathBuf]) -> Result<()> {
    let mut args = vec![
        std::ffi::OsString::from("restore"),
        std::ffi::OsString::from("--staged"),
        std::ffi::OsString::from("--"),
    ];
    args.extend(paths.iter().map(|path| relative(root, path).to_owned()));
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|arg| arg.as_os_str()).collect();
    if run_git(root, &refs).is_ok() {
        return Ok(());
    }
    // Initial commit: no HEAD yet, so remove staged files from the index.
    let mut args = vec![
        std::ffi::OsString::from("rm"),
        std::ffi::OsString::from("--cached"),
        std::ffi::OsString::from("--quiet"),
        std::ffi::OsString::from("--"),
    ];
    args.extend(paths.iter().map(|path| relative(root, path).to_owned()));
    let refs: Vec<&std::ffi::OsStr> = args.iter().map(|arg| arg.as_os_str()).collect();
    run_git(root, &refs)
}

pub fn diff(root: &Path, path: &Path, staged: bool) -> Result<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(["diff", "--no-color"]);
    if staged {
        command.arg("--cached");
    }
    command.arg("--").arg(relative(root, path));
    let output = command.output().context("无法启动 git diff")?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn commit(root: &Path, message: &str) -> Result<()> {
    run_git(
        root,
        &[
            std::ffi::OsStr::new("commit"),
            std::ffi::OsStr::new("-m"),
            std::ffi::OsStr::new(message),
        ],
    )
}

pub struct GitMonitor {
    requests: mpsc::Sender<PathBuf>,
    results: mpsc::Receiver<(PathBuf, GitSnapshot)>,
    last: Instant,
    requested: Option<PathBuf>,
}
impl Default for GitMonitor {
    fn default() -> Self {
        let (requests, incoming) = mpsc::channel::<PathBuf>();
        let (outgoing, results) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(mut path) = incoming.recv() {
                while let Ok(newer) = incoming.try_recv() {
                    path = newer;
                }
                let snapshot = GitSnapshot::scan(&path);
                if outgoing.send((path, snapshot)).is_err() {
                    break;
                }
            }
        });
        Self {
            requests,
            results,
            last: Instant::now() - Duration::from_secs(10),
            requested: None,
        }
    }
}
impl GitMonitor {
    /// Request a scan on the next poll instead of waiting for the interval.
    pub fn force(&mut self) {
        self.last = Instant::now() - Duration::from_secs(10);
    }
    pub fn poll(&mut self, path: &Path) -> Vec<(PathBuf, GitSnapshot)> {
        if self.requested.as_deref() != Some(path) || self.last.elapsed() >= Duration::from_secs(2)
        {
            let _ = self.requests.send(path.into());
            self.last = Instant::now();
            self.requested = Some(path.into());
        }
        self.results.try_iter().collect()
    }
}
