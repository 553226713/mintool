use super::model::SshConnection;
use crate::model::session::{
    ModuleId, SessionCapabilities, SessionKey, SessionView, SshLaunch, TerminalLauncher,
};
use std::path::PathBuf;

/// Path used for `SSH_ASKPASS`. Integration tests can override it with
/// `MINTOOL_ASKPASS_EXECUTABLE`, because a test binary is not the GUI binary.
pub fn askpass_executable() -> Option<PathBuf> {
    std::env::var_os("MINTOOL_ASKPASS_EXECUTABLE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
}

pub fn home_dir() -> PathBuf {
    crate::services::desktop::home_dir()
}

/// Shared OpenSSH control socket options. The first connection becomes a
/// master and later ssh/sftp operations reuse authentication for a minute.
pub fn control_options() -> Vec<String> {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".mintool").join("run"));
    let directory = base.join("mintool-ssh");
    if std::fs::create_dir_all(&directory).is_err() {
        return Vec::new();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    }
    vec![
        "-o".to_string(),
        "ControlMaster=auto".to_string(),
        "-o".to_string(),
        format!("ControlPath={}/%C", directory.display()),
        "-o".to_string(),
        "ControlPersist=60".to_string(),
    ]
}

impl SessionView {
    /// Neutral workspace description owned by the SSH module.
    pub fn ssh(connection: &SshConnection) -> Self {
        let mut launched = launch(connection);
        if let Some(password) = &launched.password {
            if let Some(executable) = askpass_executable() {
                launched.envs = vec![
                    ("SSH_ASKPASS".to_string(), executable.display().to_string()),
                    ("SSH_ASKPASS_REQUIRE".to_string(), "force".to_string()),
                    ("MINTOOL_ASKPASS".to_string(), "1".to_string()),
                    ("MINTOOL_ASKPASS_PASSWORD".to_string(), password.clone()),
                ];
            }
        }
        Self {
            key: SessionKey::new(ModuleId::Ssh, connection.id.clone()),
            title: connection.name.clone(),
            subtitle: connection.display_address(),
            cwd: home_dir(),
            launcher: TerminalLauncher::Ssh(launched),
            capabilities: SessionCapabilities::SSH,
            run_command: None,
        }
    }
}

/// Build the system `ssh` invocation. Key/agent authentication is handled by
/// OpenSSH itself; a saved password is fed through `SSH_ASKPASS`.
pub fn launch(connection: &SshConnection) -> SshLaunch {
    let mut args = Vec::new();
    if connection.port != 22 {
        args.push("-p".to_string());
        args.push(connection.port.to_string());
    }
    if let Some(identity) = &connection.identity {
        args.push("-i".to_string());
        args.push(identity.display().to_string());
    }
    args.extend([
        "-o".to_string(),
        "ServerAliveInterval=30".to_string(),
        "-o".to_string(),
        "ServerAliveCountMax=3".to_string(),
    ]);
    args.extend(connection.extra_args.iter().cloned());
    for forward in &connection.forwards {
        let (flag, target) = if let Some(target) = forward.strip_prefix("R:") {
            ("-R", target)
        } else if let Some(target) = forward.strip_prefix("D:") {
            ("-D", target)
        } else {
            ("-L", forward.as_str())
        };
        args.push(flag.to_string());
        args.push(target.to_string());
    }
    args.extend(control_options());
    args.push(connection.target());
    SshLaunch {
        program: "ssh".to_string(),
        args,
        password: connection.password_value().map(str::to_string),
        envs: Vec::new(),
    }
}
