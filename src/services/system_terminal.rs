use anyhow::{bail, Result};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug)]
pub struct SystemTerminal {
    pub name: &'static str,
    pub executable: PathBuf,
}

impl SystemTerminal {
    pub fn detect() -> Vec<Self> {
        Self::detect_in(&std::env::var_os("PATH").unwrap_or_default())
    }
    pub fn detect_in(paths: &OsStr) -> Vec<Self> {
        use std::os::unix::fs::PermissionsExt;
        ["alacritty", "xterm", "urxvt", "st"]
            .into_iter()
            .filter_map(|name| {
                std::env::split_paths(paths)
                    .map(|path| path.join(name))
                    .find(|path| {
                        path.metadata().is_ok_and(|meta| {
                            meta.is_file() && meta.permissions().mode() & 0o111 != 0
                        })
                    })
                    .map(|executable| Self { name, executable })
            })
            .collect()
    }
    pub fn command(&self, host: u32, directory: &Path, shell: &str) -> Result<Command> {
        use std::os::unix::process::CommandExt;
        anyhow::ensure!(
            directory.is_dir(),
            "项目目录不存在：{}",
            directory.display()
        );
        let mut command = Command::new(&self.executable);
        command.process_group(0);
        command.current_dir(directory);
        let host = host.to_string();
        match self.name {
            "alacritty" => {
                command
                    .args(["--embed", &host, "--working-directory"])
                    .arg(directory)
                    .args(["-e", shell]);
            }
            "xterm" => {
                command.args([
                    "-fa",
                    "monospace:lang=zh-cn",
                    "-fs",
                    "12",
                    "-into",
                    &host,
                    "-e",
                    shell,
                ]);
            }
            "urxvt" => {
                command
                    .args(["-embed", &host, "-cd"])
                    .arg(directory)
                    .args(["-e", shell]);
            }
            "st" => {
                command.args(["-w", &host, "-e", shell]);
            }
            _ => bail!("该终端不支持嵌入"),
        }
        // Embedding targets are X11 child windows, even in a mixed desktop session.
        command
            .env_remove("WAYLAND_DISPLAY")
            .env("WINIT_UNIX_BACKEND", "x11");
        Ok(command)
    }
}
