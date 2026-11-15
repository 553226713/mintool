use super::{SessionCapabilities, SessionKey, TerminalLauncher};
use std::path::PathBuf;

/// A module-neutral description of an open workspace.
#[derive(Clone, Debug)]
pub struct SessionView {
    pub key: SessionKey,
    pub title: String,
    pub subtitle: String,
    pub cwd: PathBuf,
    pub launcher: TerminalLauncher,
    pub capabilities: SessionCapabilities,
    /// Command run before the interactive shell for local project terminals.
    pub run_command: Option<String>,
}
