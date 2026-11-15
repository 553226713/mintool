use adabraka_ui::components::icon::Icon;
use gpui::*;
use std::path::Path;

pub fn file_icon(path: &Path, directory: bool, expanded: bool) -> Icon {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let (icon, color) = if directory {
        (
            if expanded {
                "folder-open"
            } else {
                "folder-closed"
            },
            0xe4bd73,
        )
    } else if name == "dockerfile" || name.starts_with(".env") || name.starts_with(".git") {
        ("file-config", 0x91a8c8)
    } else {
        match ext.as_str() {
            "rs" => ("file-code", 0xdea584),
            "js" | "jsx" => ("file-code", 0xe8d36e),
            "ts" | "tsx" => ("file-code", 0x7bb4ef),
            "py" => ("file-code", 0x8cbfd9),
            "go" => ("file-code", 0x73cedd),
            "c" | "h" | "cpp" | "hpp" | "java" | "kt" | "swift" | "rb" | "php" | "vue"
            | "svelte" | "html" | "css" | "scss" => ("file-code", 0xbb9ee8),
            "json" | "toml" | "yaml" | "yml" | "xml" | "ini" | "conf" | "lock" => {
                ("file-config", 0xc6b487)
            }
            "md" | "mdx" | "rst" => ("file-markdown", 0x85bade),
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "ico" => ("file-image", 0xb8a1dd),
            "zip" | "gz" | "tar" | "xz" | "7z" | "rar" => ("file-archive", 0xd8ae76),
            "db" | "sqlite" | "sql" => ("file-database", 0x86c7af),
            "sh" | "bash" | "zsh" | "fish" => ("square-terminal", 0x8ac9a5),
            "mp3" | "wav" | "ogg" | "flac" | "mp4" | "webm" | "mov" => ("file-media", 0xdb95b7),
            _ => ("file-text", 0x9aa9bd),
        }
    };
    let mut color: Hsla = rgb(color).into();
    if crate::ui::theme::is_light() {
        color.l = color.l.min(0.40);
    }
    Icon::new(icon).size(px(14.)).text_color(color)
}
