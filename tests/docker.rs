#[test]
#[ignore = "requires a running local Docker engine"]
fn live_snapshot_lists_the_test_container() {
    let (containers, images, _, _) = mintool::modules::docker::cli::snapshot().unwrap();
    assert!(containers
        .iter()
        .any(|container| container.name == "mintool-ssh-test"));
    assert!(images
        .iter()
        .any(|image| image.repository == "mintool-ssh-test"));
}

#[test]
fn docker_sessions_build_logs_and_exec_commands() {
    use mintool::model::session::SessionView;
    use mintool::modules::docker::model::{parse_session_id, DockerContainer, DockerView};

    let container = DockerContainer {
        id: "abc123".into(),
        name: "web".into(),
        image: "nginx:latest".into(),
        state: "running".into(),
        status: "Up".into(),
        ports: "8080->80/tcp".into(),
    };
    let logs = SessionView::docker(&container, DockerView::Logs);
    assert_eq!(logs.key.id, "abc123-logs");
    let spec = logs.terminal_spec("/bin/bash");
    assert_eq!(spec.program, "docker");
    assert_eq!(spec.args, ["logs", "-f", "--tail", "500", "abc123"]);
    let exec = SessionView::docker(&container, DockerView::Exec);
    assert_eq!(exec.key.id, "abc123-exec");
    let spec = exec.terminal_spec("/bin/bash");
    assert_eq!(spec.args, ["exec", "-it", "abc123", "/bin/sh"]);
    assert_eq!(
        parse_session_id("abc123-exec"),
        Some(("abc123".to_string(), DockerView::Exec))
    );
}

#[test]
fn docker_session_files_are_storage_safe() {
    use mintool::modules::{ModuleId, SessionKey};
    use mintool::services::storage::Storage;

    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::new(dir.path().into()).unwrap();
    assert!(storage
        .session_path(&SessionKey::new(ModuleId::Docker, "abc123-logs"))
        .is_ok());
    assert!(storage
        .session_path(&SessionKey::new(ModuleId::Docker, "../../outside"))
        .is_err());
}
