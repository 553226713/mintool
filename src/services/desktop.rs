use std::{
    path::Path,
    process::{Command, Stdio},
};

pub struct Desktop;
impl Desktop {
    pub fn open_directory(path: &Path) -> anyhow::Result<()> {
        anyhow::ensure!(path.is_dir(), "项目目录不存在：{}", path.display());
        let mut child = Command::new("xdg-open")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}
