use anyhow::{Context, Result};
use std::{
    fs::{self, OpenOptions},
    path::{Component, Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug)]
pub enum FileAction {
    NewFile(PathBuf),
    NewDirectory(PathBuf),
    Rename(PathBuf),
    Trash(PathBuf),
}
impl FileAction {
    pub fn title(&self) -> &'static str {
        match self {
            Self::NewFile(_) => "新建文件",
            Self::NewDirectory(_) => "新建文件夹",
            Self::Rename(_) => "重命名",
            Self::Trash(_) => "移到回收站",
        }
    }
    pub fn path(&self) -> &Path {
        match self {
            Self::NewFile(p) | Self::NewDirectory(p) | Self::Rename(p) | Self::Trash(p) => p,
        }
    }
    pub fn execute(&self, name: &str) -> Result<Option<PathBuf>> {
        if matches!(self, Self::Trash(_)) {
            let output = Command::new("gio")
                .args(["trash", "--"])
                .arg(self.path())
                .output()
                .context("无法启动系统回收站服务 gio")?;
            anyhow::ensure!(
                output.status.success(),
                "移到回收站失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Ok(None);
        }
        let mut components = Path::new(name).components();
        anyhow::ensure!(
            matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none(),
            "名称必须是单个文件名，不能包含路径或 .."
        );
        let parent = match self {
            Self::Rename(path) => path.parent().context("无法重命名根目录")?,
            _ => self.path(),
        };
        let destination = parent.join(name);
        if matches!(self, Self::Rename(path) if *path == destination) {
            return Ok(Some(destination));
        }
        anyhow::ensure!(destination.symlink_metadata().is_err(), "目标名称已存在");
        match self {
            Self::NewFile(_) => {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&destination)?;
            }
            Self::NewDirectory(_) => fs::create_dir(&destination)?,
            Self::Rename(path) => fs::rename(path, &destination)?,
            Self::Trash(_) => unreachable!(),
        }
        Ok(Some(destination))
    }
    pub fn duplicate(path: &Path) -> Result<PathBuf> {
        let parent = path.parent().context("无法复制根目录")?;
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = path
            .extension()
            .map(|ext| format!(".{}", ext.to_string_lossy()))
            .unwrap_or_default();
        for index in 1..10000 {
            let destination = parent.join(format!("{stem} 副本 {index}{extension}"));
            let mut target = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
            {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            let result = (|| -> Result<()> {
                let mut source = fs::File::open(path)?;
                std::io::copy(&mut source, &mut target)?;
                fs::set_permissions(&destination, source.metadata()?.permissions())?;
                Ok(())
            })();
            if let Err(error) = result {
                let _ = fs::remove_file(&destination);
                return Err(error);
            }
            return Ok(destination);
        }
        anyhow::bail!("副本名称已用完")
    }
}
