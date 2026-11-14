use std::{
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result};

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
    pub fn open_uri(uri: &str) -> Result<()> {
        let uri = uri.trim();
        anyhow::ensure!(is_openable_uri(uri), "选中文本不是可打开的链接");
        let mut child = Command::new("xdg-open")
            .arg(uri)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
    pub fn open_clipboard_link() -> Result<()> {
        let clipboard = x11_clipboard::Clipboard::new()
            .map_err(|error| anyhow::anyhow!("无法连接 X11 剪贴板：{error:?}"))?;
        let atoms = &clipboard.getter.atoms;
        let bytes = clipboard
            .load(
                atoms.clipboard,
                atoms.utf8_string,
                atoms.property,
                Duration::from_secs(1),
            )
            .map_err(|error| anyhow::anyhow!("无法读取选中链接：{error:?}"))?;
        let text = String::from_utf8(bytes).context("选中链接不是 UTF-8 文本")?;
        Self::open_uri(&text)
    }
}

fn is_openable_uri(uri: &str) -> bool {
    ["http://", "https://", "mailto:", "file://"]
        .iter()
        .any(|scheme| uri.starts_with(scheme))
}

#[cfg(test)]
mod tests {
    use super::is_openable_uri;

    #[test]
    fn only_explicit_desktop_uri_schemes_are_opened() {
        assert!(is_openable_uri("https://example.com"));
        assert!(is_openable_uri("file:///tmp/example.html"));
        assert!(!is_openable_uri("example.com"));
        assert!(!is_openable_uri("javascript:alert(1)"));
        assert!(!is_openable_uri("--help"));
    }
}
