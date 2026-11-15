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
    args.push(connection.target());
    SshLaunch {
        program: "ssh".to_string(),
        args,
        password: connection.password.clone(),
        envs: Vec::new(),
    }
}
