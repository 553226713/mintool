use std::{path::Path, process::Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineChange {
    pub start: usize,
    pub count: usize,
    pub kind: ChangeKind,
}

pub struct FileChanges;
impl FileChanges {
    /// Compare HEAD with the working copy, including staged changes.
    pub fn scan(root: &Path, path: &Path, added: bool, has_head: bool) -> Vec<LineChange> {
        if added {
            return vec![LineChange {
                start: 0,
                count: usize::MAX,
                kind: ChangeKind::Added,
            }];
        }
        let Ok(relative) = path.strip_prefix(root) else {
            return Vec::new();
        };
        let mut command = Command::new("git");
        command.arg("-C").arg(root).args([
            "--literal-pathspecs",
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--unified=0",
            "--no-renames",
        ]);
        if has_head {
            command.arg("HEAD");
        }
        command.arg("--").arg(relative);
        command
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| Self::parse(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    }
    pub fn parse(diff: &str) -> Vec<LineChange> {
        diff.lines()
            .filter_map(|line| {
                if !line.starts_with("@@ -") {
                    return None;
                }
                let mut fields = line.split_whitespace();
                fields.next()?;
                let old = fields.next()?.strip_prefix('-')?;
                let new = fields.next()?.strip_prefix('+')?;
                let (old_start, old_count) = Self::range(old)?;
                let (start, count) = Self::range(new)?;
                let _ = old_start;
                Some(LineChange {
                    start: start.saturating_sub(1),
                    count: count.max(1),
                    kind: if count == 0 {
                        ChangeKind::Deleted
                    } else if old_count == 0 {
                        ChangeKind::Added
                    } else {
                        ChangeKind::Modified
                    },
                })
            })
            .collect()
    }
    fn range(value: &str) -> Option<(usize, usize)> {
        let mut parts = value.split(',');
        Some((
            parts.next()?.parse().ok()?,
            parts.next().unwrap_or("1").parse().ok()?,
        ))
    }
}
