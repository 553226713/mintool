use mintool::{model::settings::Settings, services::system_terminal::SystemTerminal};
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn detects_only_executable_embeddable_terminals_and_keeps_path_priority() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    for path in [
        first.join("xterm"),
        second.join("xterm"),
        first.join("alacritty"),
        first.join("konsole"),
    ] {
        fs::write(&path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(first.join("st"), "not executable").unwrap();
    let paths = std::env::join_paths([&first, &second]).unwrap();
    let detected = SystemTerminal::detect_in(&paths);
    assert_eq!(
        detected
            .iter()
            .map(|terminal| terminal.name)
            .collect::<Vec<_>>(),
        ["alacritty", "xterm"]
    );
    assert_eq!(detected[1].executable, first.join("xterm"));
    let command = detected[0].command(123, root.path(), "/bin/bash").unwrap();
    assert_eq!(
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        [
            "--embed",
            "123",
            "--working-directory",
            root.path().to_str().unwrap(),
            "-e",
            "/bin/bash"
        ]
    );
    assert_eq!(command.get_current_dir(), Some(root.path()));
}

#[test]
fn existing_settings_default_to_automatic_and_preserve_backend_on_reload() {
    let mut settings: Settings = toml::from_str("font_size = 15.0\nshell = '/bin/bash'\n").unwrap();
    assert_eq!(settings.terminal_backend, "auto");
    settings.terminal_backend = "xterm".into();
    let restored: Settings = toml::from_str(&toml::to_string(&settings).unwrap()).unwrap();
    assert_eq!(restored.terminal_backend, "xterm");
}

#[test]
#[ignore = "requires an isolated X11 display and xterm in PATH"]
fn embeds_resizes_hides_and_cleans_up_a_real_system_terminal() {
    use mintool::services::embedded_terminal::EmbeddedTerminal;
    use std::time::{Duration, Instant};
    use x11rb::{
        connection::Connection,
        protocol::xproto::{ConnectionExt, CreateWindowAux, MapState, WindowClass},
        COPY_DEPTH_FROM_PARENT,
    };
    let (connection, screen) = x11rb::connect(None).unwrap();
    let parent = connection.generate_id().unwrap();
    connection
        .create_window(
            COPY_DEPTH_FROM_PARENT,
            parent,
            connection.setup().roots[screen].root,
            0,
            0,
            800,
            600,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap()
        .check()
        .unwrap();
    connection.map_window(parent).unwrap().check().unwrap();
    let terminal = SystemTerminal::detect()
        .into_iter()
        .find(|terminal| terminal.name == "xterm")
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut embedded =
        EmbeddedTerminal::spawn(parent, &terminal, directory.path(), "/bin/sh").unwrap();
    embedded.position(10, 20, 700, 400).unwrap();
    let host = connection
        .query_tree(parent)
        .unwrap()
        .reply()
        .unwrap()
        .children[0];
    let start = Instant::now();
    let child = loop {
        embedded.refresh_children().unwrap();
        if let Some(child) = connection
            .query_tree(host)
            .unwrap()
            .reply()
            .unwrap()
            .children
            .first()
        {
            break *child;
        }
        assert!(!embedded.exited(), "xterm exited before embedding");
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    };
    embedded.position(30, 40, 640, 360).unwrap();
    let geometry = connection.get_geometry(child).unwrap().reply().unwrap();
    assert_eq!((geometry.width, geometry.height), (640, 360));
    assert!(!embedded.exited());
    embedded.show(false).unwrap();
    assert_eq!(
        connection
            .get_window_attributes(host)
            .unwrap()
            .reply()
            .unwrap()
            .map_state,
        MapState::UNMAPPED
    );
    embedded.show(true).unwrap();
    drop(embedded);
    assert!(connection
        .query_tree(parent)
        .unwrap()
        .reply()
        .unwrap()
        .children
        .is_empty());
    connection.destroy_window(parent).unwrap().check().unwrap();
}
