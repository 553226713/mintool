use mintool::services::{
    file_actions::FileAction,
    file_changes::{ChangeKind, FileChanges},
    files::FileCache,
    git::GitSnapshot,
};
use std::{fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn changes_cover_staged_and_unstaged_lines_and_deletion_anchors() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.email", "test@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    let path = root.join("[test] 文件.rs");
    fs::write(&path, "one\ntwo\nthree\nfour\nfive\nsix\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "initial"]);
    fs::write(&path, "one\nTWO\nthree\nfour\nfive\nsix\n").unwrap();
    git(root, &["add", "."]);
    fs::write(&path, "one\nTWO\nthree\nfour\nfive\nSIX\nnew\n").unwrap();
    let snapshot = GitSnapshot::scan(root);
    let changes = &snapshot.lines[&path];
    assert_eq!(changes[0].start, 1);
    assert_eq!(changes[0].kind, ChangeKind::Modified);
    assert_eq!(changes[1].start, 5);
    assert_eq!(changes[1].count, 2);
    fs::write(&path, "two\nthree\nfour\nfive\nsix\n").unwrap();
    let changes = FileChanges::scan(root, &path, false, true);
    assert_eq!(changes[0].start, 0);
    assert_eq!(changes[0].kind, ChangeKind::Deleted);
    fs::write(root.join("new.txt"), "new\n").unwrap();
    let snapshot = GitSnapshot::scan(root);
    assert_eq!(
        snapshot.lines[&root.join("new.txt")][0].kind,
        ChangeKind::Added
    );
}

#[test]
fn new_repository_marks_staged_new_files_as_added() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    let path = dir.path().join("new.rs");
    fs::write(&path, "fn main() {}\n").unwrap();
    git(dir.path(), &["add", "."]);
    assert_eq!(
        GitSnapshot::scan(dir.path()).lines[&path][0].kind,
        ChangeKind::Added
    );
}

#[test]
fn file_actions_preserve_existing_files_and_refresh_cached_directories() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let mut cache = FileCache::default();
    assert!(cache.entries(&root).unwrap().is_empty());
    let file = FileAction::NewFile(root.clone())
        .execute("a.rs")
        .unwrap()
        .unwrap();
    fs::write(&file, "original").unwrap();
    cache.invalidate();
    assert_eq!(cache.entries(&root).unwrap().len(), 1);
    assert!(FileAction::NewFile(root.clone()).execute("a.rs").is_err());
    assert!(FileAction::NewDirectory(root.clone())
        .execute("../outside")
        .is_err());
    assert!(FileAction::Rename(file.clone()).execute("..").is_err());
    let copy = FileAction::duplicate(&file).unwrap();
    assert_eq!(fs::read_to_string(&copy).unwrap(), "original");
    assert!(FileAction::Rename(file.clone())
        .execute(copy.file_name().unwrap().to_str().unwrap())
        .is_err());
    let renamed = FileAction::Rename(file.clone())
        .execute("b.rs")
        .unwrap()
        .unwrap();
    assert!(!file.exists());
    assert_eq!(fs::read_to_string(&renamed).unwrap(), "original");
    fs::remove_file(copy).unwrap();
    cache.invalidate();
    let entries = cache.entries(&root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "b.rs");
}

fn wait_for_event(
    monitor: &mut mintool::services::file_monitor::FileMonitor,
    path: &Path,
) -> std::collections::BTreeSet<std::path::PathBuf> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
    let mut paths = std::collections::BTreeSet::new();
    loop {
        if let Some(events) = monitor.poll() {
            assert!(events.error.is_none(), "{:?}", events.error);
            paths.extend(events.paths);
            if paths.contains(path) || events.rescan {
                return paths;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "missing native event for {}",
            path.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn native_events_refresh_only_affected_cache_and_follow_atomic_save() {
    use mintool::services::file_monitor::FileMonitor;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let unrelated = root.join("unchanged");
    fs::create_dir(&unrelated).unwrap();
    let mut monitor = FileMonitor::default();
    monitor.synchronize([root.clone()].into());
    let mut cache = FileCache::default();
    assert_eq!(cache.entries(&root).unwrap().len(), 1);
    let path = root.join("new.txt");
    fs::write(&path, "first").unwrap();
    let paths = wait_for_event(&mut monitor, &path);
    cache.invalidate_paths(&paths);
    assert_eq!(cache.entries(&root).unwrap().len(), 2);
    assert_eq!(cache.document(&path).unwrap(), "first");
    let temporary = root.join(".save.tmp");
    fs::write(&temporary, "second").unwrap();
    fs::rename(&temporary, &path).unwrap();
    let paths = wait_for_event(&mut monitor, &path);
    cache.invalidate_paths(&paths);
    assert_eq!(cache.document(&path).unwrap(), "second");
    fs::remove_file(&path).unwrap();
    let paths = wait_for_event(&mut monitor, &path);
    cache.invalidate_paths(&paths);
    assert_eq!(cache.entries(&root).unwrap().len(), 1);
    assert!(cache.document(&path).is_err());
    // Reads do not feed back into new refresh events.
    std::thread::sleep(std::time::Duration::from_millis(120));
    assert!(monitor.poll().is_none());
}

#[test]
fn native_watches_recover_recreated_directories_and_reexpanded_nodes() {
    use mintool::services::file_monitor::FileMonitor;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let child = root.join("child");
    fs::create_dir(&child).unwrap();
    let desired = [root.clone(), child.clone()].into();
    let mut monitor = FileMonitor::default();
    monitor.synchronize(desired);
    fs::remove_dir(&child).unwrap();
    fs::create_dir(&child).unwrap();
    wait_for_event(&mut monitor, &child);
    // It has the same path but a new inode, so a fresh native watch is required.
    assert!(monitor
        .synchronize([root.clone(), child.clone()].into())
        .contains(&child));
    let file = child.join("live.txt");
    fs::write(&file, "live").unwrap();
    wait_for_event(&mut monitor, &file);
    let mut cache = FileCache::default();
    assert_eq!(cache.entries(&child).unwrap().len(), 1);
    monitor.synchronize([root.clone()].into());
    fs::write(child.join("while-collapsed.txt"), "new").unwrap();
    let added = monitor.synchronize([root, child.clone()].into());
    assert!(added.contains(&child));
    cache.invalidate_paths(&added);
    assert_eq!(cache.entries(&child).unwrap().len(), 2);
}
