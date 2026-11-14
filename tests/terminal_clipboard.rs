use mintool::services::terminal_clipboard::TerminalClipboard;
use std::{fs, path::Path, process::Command};

#[test]
fn pasted_images_keep_original_bytes_and_live_until_terminal_closes() {
    let mut clipboard = TerminalClipboard::default();
    let png = include_bytes!("../assets/desktop/mintool.png");
    let first = clipboard.save_image("png", png).unwrap();
    let second = clipboard.save_image("png", png).unwrap();
    assert_ne!(first, second);
    assert_eq!(fs::read(&first).unwrap(), png);
    assert_eq!(fs::read(&second).unwrap(), png);
    assert_eq!(first.extension().unwrap(), "png");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&first).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(first.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    let directory = first.parent().unwrap().to_path_buf();
    drop(clipboard);
    assert!(!directory.exists());
}

#[test]
fn image_path_is_one_literal_shell_argument() {
    let path = Path::new("/tmp/图 片'\"$HOME`whoami`$(whoami).png");
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(format!(
            "printf '%s' {}",
            TerminalClipboard::quote_path(path)
        ))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        path.to_str().unwrap()
    );
}

#[test]
fn empty_images_and_invalid_extensions_are_rejected() {
    let mut clipboard = TerminalClipboard::default();
    assert!(clipboard.save_image("png", &[]).is_err());
    assert!(clipboard.save_image("../../outside", b"data").is_err());
}
