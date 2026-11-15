use crate::{
    model::session::{ModuleId, SessionKey, SessionView, TerminalLauncher},
    services::{
        files::FileCache, git::GitSnapshot, storage::Storage, terminal::TerminalSession,
        terminal::TerminalSpec,
    },
    workspace::layout::WorkspaceState,
};
use anyhow::Result;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

impl SessionView {
    /// Build the PTY command for a new terminal pane.
    pub fn terminal_spec(&self, shell: &str) -> TerminalSpec {
        match &self.launcher {
            TerminalLauncher::LocalShell => match &self.run_command {
                Some(command) => TerminalSpec {
                    program: shell.into(),
                    args: vec!["-c".to_string(), format!("{command}; exec {shell}")],
                    cwd: self.cwd.clone(),
                    envs: Vec::new(),
                },
                None => TerminalSpec::local_shell(self.cwd.clone(), shell),
            },
            TerminalLauncher::Docker { program, args } => TerminalSpec {
                program: program.clone(),
                args: args.clone(),
                cwd: self.cwd.clone(),
                envs: Vec::new(),
            },
            TerminalLauncher::Ssh(launch) => TerminalSpec {
                program: launch.program.clone(),
                args: launch.args.clone(),
                cwd: self.cwd.clone(),
                envs: launch.envs.clone(),
            },
        }
    }
}

pub struct Workspace {
    pub state: WorkspaceState,
    pub terminals: HashMap<u64, Arc<Mutex<TerminalSession>>>,
    pub errors: HashMap<u64, String>,
    pub git: GitSnapshot,
    pub cache: FileCache,
}
impl Workspace {
    pub fn path(storage: &Storage, key: &SessionKey) -> Result<PathBuf> {
        storage.session_path(key)
    }
    /// Load persisted pane state, including the one-time legacy migration.
    pub fn load(storage: &Storage, key: &SessionKey) -> Result<WorkspaceState> {
        let path = storage.session_path(key)?;
        let mut state: WorkspaceState = if path.exists() {
            storage.read_json(&path)?
        } else if key.module == ModuleId::Projects {
            let legacy = storage
                .root
                .join("workspaces")
                .join(format!("{}.json", key.id));
            if legacy.exists() {
                let state: WorkspaceState = storage.read_json(&legacy)?;
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::copy(&legacy, &path);
                state
            } else {
                WorkspaceState::default()
            }
        } else {
            WorkspaceState::default()
        };
        if key.module == ModuleId::Projects {
            state.migrate_nested_files();
        }
        Ok(state)
    }
    pub fn save(storage: &Storage, key: &SessionKey, state: &WorkspaceState) -> Result<()> {
        storage.write(&storage.session_path(key)?, state)
    }
    pub fn new(state: WorkspaceState) -> Self {
        Self {
            state,
            terminals: HashMap::new(),
            errors: HashMap::new(),
            git: GitSnapshot::default(),
            cache: FileCache::default(),
        }
    }
    pub fn relocate(&mut self, old: &Path, new: &Path) {
        self.terminals.clear();
        self.errors.clear();
        self.git = GitSnapshot::default();
        self.cache = FileCache::default();
        let mut ids = Vec::new();
        self.state.layout.ids(&mut ids);
        for id in ids {
            if let Some(pane) = self.state.layout.find_mut(id) {
                if let Some(file) = &pane.file {
                    if let Ok(relative) = file.strip_prefix(old) {
                        pane.file = Some(new.join(relative));
                    }
                }
            }
        }
        self.state.expanded = self
            .state
            .expanded
            .iter()
            .filter_map(|path| path.strip_prefix(old).ok().map(|path| new.join(path)))
            .collect();
    }
}
