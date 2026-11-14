use mintool::{
    model::{
        layout::{Axis, Layout, WorkspaceState},
        project::ProjectRegistry,
        settings::Settings,
    },
    services::terminal_input::{key, paste, KeyModifiers},
    services::{git::GitSnapshot, storage::Storage, terminal::TerminalSession},
};
use std::{
    fs,
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn project_order_grouping_and_removal_preserve_directories() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("a")).unwrap();
    fs::create_dir(dir.path().join("b")).unwrap();
    let mut registry = ProjectRegistry::default();
    let a = registry.add(dir.path().join("a")).unwrap();
    let b = registry.add(dir.path().join("b")).unwrap();
    assert_eq!(registry.add(dir.path().join("a")).unwrap(), a);
    assert_eq!(registry.projects.len(), 2);
    registry.add_group("Work").unwrap();
    let group = registry.groups[0].id.clone();
    registry.move_project(&b, Some(group.clone()), Some(&a));
    assert_eq!(registry.projects[0].id, b);
    assert_eq!(registry.projects[0].group.as_deref(), Some(group.as_str()));
    registry.remove_group(&group);
    assert!(registry.projects.iter().all(|p| p.group.is_none()));
    registry.remove(&a);
    assert!(dir.path().join("a").is_dir());
    assert_eq!(registry.selected, Some(b));
}

#[test]
fn nested_splits_close_by_merging_sibling_and_roundtrip() {
    let mut layout = Layout::pane(1);
    assert!(layout.split(1, 2, Axis::Horizontal));
    assert!(layout.split(2, 3, Axis::Vertical));
    layout.find_mut(3).unwrap().file = Some("/tmp/readme".into());
    assert!(layout.close(2));
    let saved = serde_json::to_string(&layout).unwrap();
    let mut restored: Layout = serde_json::from_str(&saved).unwrap();
    assert_eq!(
        restored.find_mut(3).unwrap().file.as_deref(),
        Some(Path::new("/tmp/readme"))
    );
    assert!(restored.close(1));
    assert_eq!(restored.first_id(), 3);
    assert!(!restored.close(3));
}

#[test]
fn storage_roundtrip_backup_recovery_and_single_instance_lock() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().into()).unwrap();
    assert!(Storage::new(dir.path().into()).is_err());
    let mut registry = ProjectRegistry::default();
    let id = registry.add(dir.path().into()).unwrap();
    storage.save(&Settings::default(), &registry).unwrap();
    let path = storage.workspace_path(&id).unwrap();
    let mut state = WorkspaceState::default();
    state.layout.split(1, 2, Axis::Vertical);
    storage.write(&path, &state).unwrap();
    storage.write(&path, &state).unwrap();
    fs::write(&path, "broken json").unwrap();
    assert!(matches!(
        storage.workspace(&id).unwrap().layout,
        Layout::Split { .. }
    ));
    storage.write(&path, &state).unwrap();
    assert!(storage.workspace_path("../../outside").is_err());
    assert_eq!(storage.projects().unwrap().selected, Some(id));
    drop(storage);
    assert!(Storage::new(dir.path().into()).is_ok());
}

#[test]
fn invalid_config_is_reported_and_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().into()).unwrap();
    fs::write(dir.path().join("projects.json"), "broken").unwrap();
    assert!(storage.projects().is_err());
    assert_eq!(
        fs::read_to_string(dir.path().join("projects.json")).unwrap(),
        "broken"
    );
}

fn git(path: &Path, args: &[&str]) {
    let result = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
fn git_detects_branch_staged_modified_deleted_untracked_and_nested_path() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "test-branch"]);
    git(
        dir.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    git(dir.path(), &["config", "user.name", "Test"]);
    fs::create_dir(dir.path().join("nested")).unwrap();
    fs::write(dir.path().join("nested/modified"), "old").unwrap();
    fs::write(dir.path().join("deleted"), "old").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "initial"]);
    fs::write(dir.path().join("nested/modified"), "new").unwrap();
    fs::remove_file(dir.path().join("deleted")).unwrap();
    fs::write(dir.path().join("untracked 中文"), "new").unwrap();
    fs::write(dir.path().join("staged"), "new").unwrap();
    git(dir.path(), &["add", "staged"]);
    let snapshot = GitSnapshot::scan(&dir.path().join("nested"));
    assert_eq!(snapshot.branch, "test-branch");
    assert_eq!(snapshot.changes.len(), 4);
    assert_eq!(
        snapshot.status(&dir.path().join("nested/modified")),
        Some(" M")
    );
    assert_eq!(snapshot.status(&dir.path().join("deleted")), Some(" D"));
    assert_eq!(
        snapshot.status(&dir.path().join("untracked 中文")),
        Some("??")
    );
    assert_eq!(snapshot.status(&dir.path().join("staged")), Some("A "));
    assert!(snapshot.status(&dir.path().join("nested")).is_some());
}
#[test]
fn git_rename_zero_terminated_paths() {
    let changes =
        GitSnapshot::parse_status(Path::new("/repo"), b"R  new name\0old name\0?? odd\nname\0");
    assert_eq!(changes.len(), 2);
    assert_eq!(changes.get(Path::new("/repo/new name")).unwrap(), "R ");
    assert!(changes.contains_key(Path::new("/repo/odd\nname")));
}

fn wait_for(session: &TerminalSession, text: &str) {
    let start = Instant::now();
    loop {
        let screen = session.parser.lock().unwrap().screen().contents();
        if screen.contains(text) {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "missing {text:?} in {screen:?}"
        );
        thread::sleep(Duration::from_millis(30));
    }
}
#[test]
fn real_pty_isolates_working_directories_keeps_processes_and_resizes() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut first = TerminalSession::spawn(a.path(), "/bin/bash").unwrap();
    let mut second = TerminalSession::spawn(b.path(), "/bin/bash").unwrap();
    first.send(b"printf 'ROOT=%s\\n' \"$PWD\"\r").unwrap();
    second.send(b"printf 'ROOT=%s\\n' \"$PWD\"\r").unwrap();
    wait_for(&first, &format!("ROOT={}", a.path().display()));
    wait_for(&second, &format!("ROOT={}", b.path().display()));
    first.resize(30, 100).unwrap();
    first.send(b"stty size\r").unwrap();
    wait_for(&first, "30 100");
    first
        .send(b"sleep 0.2; printf 'BACK%s\\n' GROUND_OK\r")
        .unwrap();
    second.send(b"printf 'OTHER_%s\\n' PROJECT_OK\r").unwrap();
    wait_for(&second, "OTHER_PROJECT_OK");
    wait_for(&first, "BACKGROUND_OK");
    first.send(b"sleep 60\r").unwrap();
    thread::sleep(Duration::from_millis(100));
    first.send(&[3]).unwrap();
    first.send(b"printf 'INTERRUPTED_%s\\n' OK\r").unwrap();
    wait_for(&first, "INTERRUPTED_OK");
}
#[test]
fn terminal_paste_and_cursor_modes_are_encoded() {
    let mut parser =
        vt100::Parser::new(vt100::Parser::DEFAULT_ROWS, vt100::Parser::DEFAULT_COLS, 0);
    parser.process(b"\x1b[?2004h\x1b[?1h");
    assert_eq!(
        paste("hello\n世界", parser.screen().bracketed_paste()),
        "\x1b[200~hello\n世界\x1b[201~".as_bytes()
    );
    assert_eq!(
        key(
            "up",
            KeyModifiers::default(),
            parser.screen().application_cursor()
        )
        .unwrap(),
        b"\x1bOA"
    );
    assert_eq!(
        key(
            "c",
            KeyModifiers {
                control: true,
                ..Default::default()
            },
            false
        )
        .unwrap(),
        vec![3]
    );
}

#[test]
fn terminal_resize_truncating_wide_character_does_not_panic() {
    let mut parser =
        vt100::Parser::new(vt100::Parser::DEFAULT_ROWS, vt100::Parser::DEFAULT_COLS, 0);
    parser.process("你".as_bytes());
    parser.set_size(
        std::num::NonZeroU16::new(2).unwrap(),
        std::num::NonZeroU16::new(1).unwrap(),
    );
    parser.process(b"\x1b[K");
}

#[test]
fn terminal_queries_survive_chunk_boundaries() {
    let mut parser = vte::Parser::new();
    let mut queries = mintool::services::terminal_query::TerminalQueries {
        cursor: (4, 9),
        replies: Vec::new(),
    };
    for chunk in [b"\x1b[".as_slice(), b"6n\x1b[", b"c\x1b]11;?\x07"] {
        for byte in chunk {
            parser.advance(&mut queries, *byte);
        }
    }
    assert_eq!(
        queries.replies,
        b"\x1b[5;10R\x1b[?1;2c\x1b]11;rgb:1e1e/2121/2828\x1b\\"
    );
}

#[test]
fn relocating_workspace_preserves_layout_and_rebases_project_files() {
    let mut state = WorkspaceState::default();
    state.layout.split(1, 2, Axis::Horizontal);
    state.layout.find_mut(2).unwrap().file = Some("/old/project/src/main.rs".into());
    state.expanded.insert("/old/project/src".into());
    let mut workspace = mintool::services::workspace::Workspace::new(state);
    workspace.relocate(Path::new("/old/project"), Path::new("/new/project"));
    assert!(matches!(workspace.state.layout, Layout::Split { .. }));
    assert_eq!(
        workspace.state.layout.find_mut(2).unwrap().file.as_deref(),
        Some(Path::new("/new/project/src/main.rs"))
    );
    assert!(workspace
        .state
        .expanded
        .contains(Path::new("/new/project/src")));
}

#[test]
fn file_preview_reloads_changes_and_rejects_binary_and_large_files() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("readme.txt");
    let mut cache = mintool::services::files::FileCache::default();
    fs::write(&file, "hello 世界").unwrap();
    assert_eq!(cache.document(&file).unwrap(), "hello 世界");
    fs::write(&file, "updated").unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(std::time::SystemTime::now() + Duration::from_secs(1))
        .unwrap();
    assert_eq!(cache.document(&file).unwrap(), "updated");
    let binary = dir.path().join("binary");
    fs::write(&binary, b"data\0binary").unwrap();
    assert!(cache.document(&binary).is_err());
    let large = dir.path().join("large");
    fs::File::create(&large)
        .unwrap()
        .set_len(2 * 1024 * 1024 + 1)
        .unwrap();
    assert!(cache.document(&large).is_err());
    fs::remove_file(&file).unwrap();
    assert!(cache.document(&file).is_err());
}

#[test]
fn resizing_nested_split_updates_only_its_own_ratio() {
    let mut layout = Layout::pane(1);
    layout.split(1, 2, Axis::Horizontal);
    layout.split(1, 3, Axis::Vertical);
    assert!(layout.set_ratio(1, 3, 0.7));
    if let Layout::Split { ratio, first, .. } = layout {
        assert_eq!(ratio, 0.5);
        if let Layout::Split { ratio, .. } = *first {
            assert_eq!(ratio, 0.7);
        } else {
            panic!("nested split missing");
        }
    } else {
        panic!("root split missing");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_gpu_transform_matches_wgsl_storage_alignment() {
    assert_eq!(std::mem::align_of::<gpui::TransformationMatrix>(), 8);
    assert_eq!(std::mem::size_of::<gpui::TransformationMatrix>(), 24);
}

#[test]
fn theme_preference_defaults_for_existing_config_and_survives_restart() {
    use mintool::model::appearance::Appearance;
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().into()).unwrap();
    fs::write(
        dir.path().join("config.toml"),
        "font_size = 16.0\nshell = '/bin/sh'\n",
    )
    .unwrap();
    let mut settings = storage.settings().unwrap();
    assert_eq!(settings.appearance, Appearance::Dark);
    settings.appearance = Appearance::Light;
    storage
        .save(&settings, &ProjectRegistry::default())
        .unwrap();
    drop(storage);
    let storage = Storage::new(dir.path().into()).unwrap();
    let restored = storage.settings().unwrap();
    assert_eq!(restored.appearance, Appearance::Light);
    assert_eq!(restored.font_size, 16.0);
    assert_eq!(restored.shell, "/bin/sh");
}
