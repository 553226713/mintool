use mintool::modules::ssh::{
    model::{SshConnection, SshRegistry},
    session::launch,
};
use std::path::PathBuf;

#[test]
fn connections_can_be_grouped_reordered_and_removed() {
    let mut registry = SshRegistry::default();
    let work = registry.add_group("生产").unwrap();
    let mut first = SshConnection::new("web", "web.example.com", Some(work.clone()));
    first.port = 2222;
    let first = registry.add(first);
    let second = registry.add(SshConnection::new(
        "db",
        "db.example.com",
        Some(work.clone()),
    ));
    assert_eq!(registry.connections.len(), 2);
    assert_eq!(registry.selected.as_deref(), Some(second.as_str()));

    registry.shift_connection(&second, -1);
    assert_eq!(registry.connections[0].id, second);
    registry.move_connection(&second, None, None);
    assert!(registry.connections[1].group.is_none());

    registry.remove_group(&work);
    assert!(registry
        .connections
        .iter()
        .all(|connection| connection.group.is_none()));
    registry.remove(&first);
    assert_eq!(registry.connections.len(), 1);
    assert_eq!(registry.selected.as_deref(), Some(second.as_str()));
}

#[cfg(unix)]
#[test]
fn saved_connections_are_written_with_owner_only_permissions() {
    use mintool::modules::ModuleId;
    use mintool::services::storage::Storage;
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().into()).unwrap();
    let mut registry = SshRegistry::default();
    let mut connection = SshConnection::new("prod", "prod.example.com", None);
    connection.password = Some("secret".into());
    registry.add(connection);
    registry.save(&storage).unwrap();

    let path = storage.registry_path(ModuleId::Ssh);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(
        SshRegistry::load(&storage).unwrap().connections[0]
            .password
            .as_deref(),
        Some("secret")
    );
}

#[test]
fn ssh_config_import_skips_wildcards_and_existing_hosts() {
    use mintool::modules::ssh::import::import_config;

    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    std::fs::write(
        &config,
        "Host prod\n  HostName 10.0.0.1\n  User root\n  Port 2222\n  IdentityFile ~/.ssh/id_prod\n\nHost *\n  ServerAliveInterval 30\n\nHost db\n  HostName db.internal\n",
    )
    .unwrap();
    let mut registry = SshRegistry::default();
    assert_eq!(import_config(&mut registry, &config).unwrap(), 2);
    let prod = registry
        .connections
        .iter()
        .find(|connection| connection.name == "prod")
        .unwrap();
    assert_eq!(prod.host, "prod");
    assert_eq!(prod.port, 2222);
    assert_eq!(prod.user, "root");
    assert!(prod
        .identity
        .as_ref()
        .is_some_and(|path| path.ends_with(".ssh/id_prod")));
    let db = registry
        .connections
        .iter()
        .find(|connection| connection.name == "db")
        .unwrap();
    assert_eq!(db.host, "db");
    assert_eq!(db.port, 22);
    assert_eq!(import_config(&mut registry, &config).unwrap(), 0);
    assert_eq!(registry.connections.len(), 2);
}

#[test]
fn ssh_launch_keeps_aliases_and_only_overrides_explicit_values() {
    let alias = SshConnection::new("prod", "prod-alias", None);
    let launched = launch(&alias);
    assert_eq!(launched.program, "ssh");
    assert_eq!(launched.args.last().unwrap(), "prod-alias");
    assert!(!launched.args.iter().any(|argument| argument == "-p"));

    let mut explicit = SshConnection::new("box", "10.0.0.8", None);
    explicit.user = "root".into();
    explicit.port = 2200;
    explicit.identity = Some(PathBuf::from("/home/me/.ssh/id_ed25519"));
    explicit.password = Some("secret".into());
    let launched = launch(&explicit);
    assert_eq!(launched.args.last().unwrap(), "root@10.0.0.8");
    assert!(launched
        .args
        .windows(2)
        .any(|pair| pair == ["-p".to_string(), "2200".to_string()]));
    assert!(launched
        .args
        .windows(2)
        .any(|pair| pair == ["-i".to_string(), "/home/me/.ssh/id_ed25519".to_string()]));
    assert_eq!(launched.password.as_deref(), Some("secret"));
}

#[test]
#[ignore = "requires the local mintool-ssh-test container on 127.0.0.1:2222"]
fn live_sftp_lists_and_downloads() {
    use mintool::modules::ssh::sftp::{
        download_to_temp, list_directory, remove_remote, stat_file, upload_to,
    };

    std::env::set_var("MINTOOL_ASKPASS_EXECUTABLE", env!("CARGO_BIN_EXE_mintool"));
    let mut connection = SshConnection::new("local", "127.0.0.1", None);
    connection.port = 2222;
    connection.user = "tester".into();
    connection.password = Some("testpass".into());
    let entries = list_directory(&connection, "").unwrap();
    let file = entries
        .iter()
        .find(|entry| entry.name == "a file.txt")
        .expect("test file missing");
    let stat = stat_file(&connection, &file.path).unwrap();
    assert!(!stat.directory);
    assert_eq!(stat.size, file.size);
    let local = download_to_temp(&connection, &file.path).unwrap();
    let text = std::fs::read_to_string(&local).unwrap();
    assert!(text.contains("hello remote"));
    std::fs::remove_file(local).unwrap();

    let nested = list_directory(&connection, "sub").unwrap();
    let inner = nested
        .iter()
        .find(|entry| entry.name == "inner.txt")
        .expect("nested test file missing");
    assert_eq!(inner.path, "sub/inner.txt");
    let inner_local = download_to_temp(&connection, &inner.path).unwrap();
    assert_eq!(
        std::fs::read_to_string(&inner_local).unwrap(),
        "nested remote\n"
    );
    std::fs::remove_file(inner_local).unwrap();

    let upload_dir = tempfile::tempdir().unwrap();
    let local = upload_dir.path().join("upload test.txt");
    std::fs::write(&local, "uploaded content\n").unwrap();
    upload_to(&connection, &local, "upload test.txt").unwrap();
    let uploaded = stat_file(&connection, "upload test.txt").unwrap();
    assert_eq!(uploaded.size, 17);
    let downloaded = download_to_temp(&connection, "upload test.txt").unwrap();
    assert_eq!(
        std::fs::read_to_string(&downloaded).unwrap(),
        "uploaded content\n"
    );
    std::fs::remove_file(downloaded).unwrap();
    remove_remote(&connection, "upload test.txt").unwrap();
    std::fs::remove_file(local).unwrap();
}
