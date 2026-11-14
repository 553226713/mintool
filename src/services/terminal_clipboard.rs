use anyhow::{ensure, Context, Result};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

/// Keeps pasted images available for the lifetime of their terminal view.
#[derive(Default)]
pub struct TerminalClipboard {
    directory: Option<TempDir>,
}

impl TerminalClipboard {
    pub fn save_image(&mut self, extension: &str, bytes: &[u8]) -> Result<PathBuf> {
        ensure!(
            matches!(
                extension,
                "png" | "jpg" | "webp" | "gif" | "svg" | "bmp" | "tiff"
            ),
            "不支持的剪贴板图片格式"
        );
        ensure!(!bytes.is_empty(), "剪贴板图片为空");
        if self.directory.is_none() {
            self.directory = Some(
                tempfile::Builder::new()
                    .prefix("mintool-clipboard-")
                    .permissions({
                        use std::os::unix::fs::PermissionsExt;
                        fs::Permissions::from_mode(0o700)
                    })
                    .tempdir()
                    .context("无法创建剪贴板图片目录")?,
            );
        }
        let file = tempfile::Builder::new()
            .prefix("image-")
            .suffix(&format!(".{extension}"))
            .tempfile_in(self.directory.as_ref().unwrap().path())
            .context("无法创建剪贴板图片文件")?;
        fs::write(file.path(), bytes).context("无法保存剪贴板图片")?;
        let (_, path) = file.keep().context("无法保留剪贴板图片")?;
        Ok(path)
    }

    pub fn quote_path(path: &std::path::Path) -> String {
        format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
    }
}
