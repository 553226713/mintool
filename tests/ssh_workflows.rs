use mintool::{
    model::session::SessionKey,
    modules::{
        common::selection::select,
        ssh::{
            model::SshConnection,
            remote_save::{save_with_store, RemoteStore},
            transfer_task::{ConflictPolicy, TransferKind, TransferState, TransferTask},
            transport,
        },
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{atomic::Ordering, Mutex},
};

#[derive(Default)]
struct Store {
    files: Mutex<BTreeMap<String, String>>,
    modes: Mutex<BTreeMap<String, String>>,
    conflict_during_upload: bool,
    fail_rename: bool,
}
impl Store {
    fn original() -> Self {
        Self {
            files: Mutex::new(BTreeMap::from([("/etc/config".into(), "original".into())])),
            ..Default::default()
        }
    }
}
impl RemoteStore for Store {
    fn read(&self, path: &str) -> anyhow::Result<String> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing"))
    }
    fn permissions(&self, _: &str) -> anyhow::Result<String> {
        Ok("-rw-r-----".into())
    }
    fn write(&self, path: &str, text: &str) -> anyhow::Result<()> {
        let mut files = self.files.lock().unwrap();
        files.insert(path.into(), text.into());
        if self.conflict_during_upload {
            files.insert("/etc/config".into(), "external edit".into());
        }
        Ok(())
    }
    fn chmod(&self, path: &str, mode: &str) -> anyhow::Result<()> {
        self.modes.lock().unwrap().insert(path.into(), mode.into());
        Ok(())
    }
    fn rename(&self, from: &str, to: &str) -> anyhow::Result<()> {
        anyhow::ensure!(!self.fail_rename, "rename rejected");
        assert!(from.starts_with("/etc/.config.mintool-"));
        let mut files = self.files.lock().unwrap();
        let text = files.remove(from).unwrap();
        files.insert(to.into(), text);
        Ok(())
    }
    fn remove(&self, path: &str) -> anyhow::Result<()> {
        self.files.lock().unwrap().remove(path);
        Ok(())
    }
}
#[test]
fn remote_save_replaces_only_after_upload_and_preserves_mode() {
    let store = Store::original();
    save_with_store(&store, "/etc/config", "original", "edited").unwrap();
    assert_eq!(store.read("/etc/config").unwrap(), "edited");
    assert_eq!(store.files.lock().unwrap().len(), 1);
    assert!(!store.modes.lock().unwrap().is_empty());
    assert!(store
        .modes
        .lock()
        .unwrap()
        .values()
        .all(|mode| mode == "640"));
}
#[test]
fn remote_save_detects_both_preexisting_and_inflight_conflicts() {
    let store = Store::original();
    assert!(save_with_store(&store, "/etc/config", "outdated", "edited").is_err());
    assert_eq!(store.read("/etc/config").unwrap(), "original");
    assert_eq!(store.files.lock().unwrap().len(), 1);
    let store = Store {
        conflict_during_upload: true,
        ..Store::original()
    };
    assert!(save_with_store(&store, "/etc/config", "original", "edited").is_err());
    assert_eq!(store.read("/etc/config").unwrap(), "external edit");
    assert_eq!(store.files.lock().unwrap().len(), 1);
}
#[test]
fn failed_remote_replace_keeps_original_and_removes_temporary_upload() {
    let store = Store {
        fail_rename: true,
        ..Store::original()
    };
    assert!(save_with_store(&store, "/etc/config", "original", "edited").is_err());
    assert_eq!(store.read("/etc/config").unwrap(), "original");
    assert_eq!(store.files.lock().unwrap().len(), 1);
}
#[test]
fn multiselect_supports_toggle_range_and_additive_range() {
    let order = vec!["a", "b", "c", "d", "e"];
    let mut selection = BTreeSet::new();
    select(&order, &mut selection, None, "b", false, false);
    select(&order, &mut selection, Some(&"b"), "d", true, false);
    assert_eq!(selection, BTreeSet::from(["b", "d"]));
    select(&order, &mut selection, Some(&"b"), "e", false, true);
    assert_eq!(selection, BTreeSet::from(["b", "c", "d", "e"]));
    select(&order, &mut selection, Some(&"d"), "a", true, true);
    assert_eq!(selection, BTreeSet::from(["a", "b", "c", "d", "e"]));
    select(&order, &mut selection, Some(&"a"), "c", true, false);
    assert!(!selection.contains("c"));
}
#[test]
fn retries_have_new_cancellation_and_progress_handles_and_keep_owner() {
    let owner = (SessionKey::ssh("server-a"), 3);
    let mut task = TransferTask::new(
        owner.clone(),
        SshConnection::new("a", "a", None),
        TransferKind::Download {
            remote: "/x".into(),
            local: PathBuf::from("/tmp/x"),
        },
        false,
        ConflictPolicy::Ask,
    );
    let old_cancel = task.cancel.clone();
    let old_progress = task.progress.clone();
    task.state = TransferState::Failed;
    task.error = Some("network".into());
    task.retry();
    old_cancel.store(true, Ordering::Relaxed);
    old_progress.set_transferred(123);
    assert_eq!(task.owner, owner);
    assert_eq!(task.state, TransferState::Ready);
    assert!(!task.cancel.load(Ordering::Relaxed));
    assert_eq!(task.progress.transferred(), 0);
    assert_eq!(task.attempt, 1);
    assert!(task.error.is_none());
}
#[test]
fn ssh_and_sftp_share_jump_proxy_identity_and_resolved_password() {
    let mut connection = SshConnection::new("prod", "prod-alias", None);
    connection.port = 2222;
    connection.user = "deploy".into();
    connection.identity = Some("/tmp/key with spaces".into());
    connection.resolved_password = Some("test-only".into());
    connection.extra_args = vec![
        "-J".into(),
        "bastion".into(),
        "-o".into(),
        "ProxyCommand=ssh -W %h:%p relay".into(),
        "-L".into(),
        "9999:localhost:80".into(),
    ];
    let ssh = transport::connection_arguments(&connection);
    let sftp = transport::sftp_arguments(&connection).unwrap();
    assert!(ssh.windows(2).any(|p| p == ["-J", "bastion"]));
    assert!(sftp.windows(2).any(|p| p == ["-J", "bastion"]));
    assert!(sftp.windows(2).any(|p| p == ["-P", "2222"]));
    assert!(sftp.iter().any(|p| p == "/tmp/key with spaces"));
    assert!(sftp.iter().any(|p| p == "ProxyCommand=ssh -W %h:%p relay"));
    assert!(!sftp
        .iter()
        .any(|p| p == "BatchMode=yes" || p == "-L" || p == "9999:localhost:80"));
    assert_eq!(sftp.last().unwrap(), "deploy@prod-alias");
    assert!(transport::authentication_environment(&connection)
        .iter()
        .any(|(name, value)| name == "MINTOOL_ASKPASS_PASSWORD" && value == "test-only"));
}
#[test]
fn browser_history_and_saved_preferences_are_isolated() {
    use mintool::{
        model::session::ModuleId,
        modules::ssh::{
            browser::RemoteBrowser,
            local::LocalBrowser,
            preferences::{self, BrowserPreferences},
        },
        services::storage::Storage,
    };
    let mut first = RemoteBrowser::new("one".into());
    let second = RemoteBrowser::new("two".into());
    first.current = "/var/log".into();
    first.selected_paths.insert("/var/log/a".into());
    assert!(second.current.is_empty());
    assert!(second.selected_paths.is_empty());
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("sub")).unwrap();
    let mut local = LocalBrowser::new(root.path().into());
    local.navigate(root.path().join("sub"));
    local.back();
    local.forward();
    assert!(local.current.ends_with("sub"));
    let storage = Storage::new(root.path().join("config")).unwrap();
    let saved: BTreeMap<String, BrowserPreferences> = BTreeMap::from([
        (
            "one/1".into(),
            BrowserPreferences {
                remote: "/var/log".into(),
                bookmarks: vec!["/etc".into()],
                ..Default::default()
            },
        ),
        (
            "two/1".into(),
            BrowserPreferences {
                remote: "/srv".into(),
                ..Default::default()
            },
        ),
    ]);
    storage
        .write(
            &storage.module_dir(ModuleId::Ssh).join("browsers.json"),
            &saved,
        )
        .unwrap();
    let restored = preferences::load(&storage).unwrap();
    assert_eq!(restored["one/1"].remote, "/var/log");
    assert_eq!(restored["two/1"].remote, "/srv");
    assert_eq!(restored["one/1"].bookmarks, vec!["/etc"]);
}

#[test]
fn quoted_proxy_arguments_survive_a_form_roundtrip() {
    let original = vec![
        "-J".to_string(),
        "user@jump:2222".into(),
        "-o".into(),
        "ProxyCommand=ssh -W %h:%p relay".into(),
        "-i".into(),
        "/tmp/key with spaces".into(),
    ];
    assert_eq!(
        transport::parse_arguments(&shell_words::join(&original)).unwrap(),
        original
    );
    assert!(transport::parse_arguments("-o 'broken").is_err());
}

#[test]
fn recent_directories_are_deduplicated_bounded_and_keep_the_most_recent() {
    use mintool::modules::ssh::preferences::BrowserPreferences;
    let mut preferences = BrowserPreferences::default();
    for index in 0..30 {
        assert!(preferences.remember_remote(&format!("/dir/{index}")));
    }
    assert_eq!(preferences.recent_remote.len(), 20);
    assert_eq!(preferences.recent_remote[0], "/dir/29");
    assert!(!preferences.remember_remote("/dir/29"));
    assert!(preferences.remember_remote("/dir/20"));
    assert_eq!(preferences.recent_remote[0], "/dir/20");
    assert_eq!(
        preferences
            .recent_remote
            .iter()
            .filter(|path| path.as_str() == "/dir/20")
            .count(),
        1
    );
}
#[test]
fn resumed_transfer_progress_counts_only_new_bytes_for_speed() {
    let progress = mintool::modules::ssh::sftp::TransferProgress::new(1000);
    progress.set_initial(900);
    progress.set_transferred(950);
    assert_eq!(progress.percent(), Some(95));
    assert_eq!(progress.transferred_since_start(), 50);
    progress.finish(1000);
    assert_eq!(progress.transferred_since_start(), 100);
    assert_eq!(progress.percent(), Some(100));
}
