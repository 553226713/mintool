use super::SshLaunch;

#[derive(Clone, Debug)]
pub enum TerminalLauncher {
    LocalShell,
    Ssh(SshLaunch),
    Docker { program: String, args: Vec<String> },
}
