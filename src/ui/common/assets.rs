use gpui::*;
use std::borrow::Cow;

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes = match path {
            "icons/arrow-up.svg" => include_bytes!("../../../assets/icons/arrow-up.svg").as_slice(),
            "icons/arrow-down.svg" => {
                include_bytes!("../../../assets/icons/arrow-down.svg").as_slice()
            }
            "icons/check.svg" => include_bytes!("../../../assets/icons/check.svg").as_slice(),
            "icons/chevron-down.svg" => {
                include_bytes!("../../../assets/icons/chevron-down.svg").as_slice()
            }
            "icons/chevron-right.svg" => {
                include_bytes!("../../../assets/icons/chevron-right.svg").as_slice()
            }
            "icons/columns-2.svg" => {
                include_bytes!("../../../assets/icons/columns-2.svg").as_slice()
            }
            "icons/docker.svg" => include_bytes!("../../../assets/icons/docker.svg").as_slice(),
            "icons/file-text.svg" => {
                include_bytes!("../../../assets/icons/file-text.svg").as_slice()
            }
            "icons/external-link.svg" => {
                include_bytes!("../../../assets/icons/external-link.svg").as_slice()
            }
            "icons/files.svg" => include_bytes!("../../../assets/icons/files.svg").as_slice(),
            "icons/folder-closed.svg" => {
                include_bytes!("../../../assets/icons/folder-closed.svg").as_slice()
            }
            "icons/folder-open.svg" => {
                include_bytes!("../../../assets/icons/folder-open.svg").as_slice()
            }
            "icons/folder-plus.svg" => {
                include_bytes!("../../../assets/icons/folder-plus.svg").as_slice()
            }
            "icons/folder-x.svg" => include_bytes!("../../../assets/icons/folder-x.svg").as_slice(),
            "icons/git-branch.svg" => {
                include_bytes!("../../../assets/icons/git-branch.svg").as_slice()
            }
            "icons/plus.svg" => include_bytes!("../../../assets/icons/plus.svg").as_slice(),
            "icons/rows-2.svg" => include_bytes!("../../../assets/icons/rows-2.svg").as_slice(),
            "icons/search.svg" => include_bytes!("../../../assets/icons/search.svg").as_slice(),
            "icons/bell.svg" => include_bytes!("../../../assets/icons/bell.svg").as_slice(),
            "icons/settings-2.svg" => {
                include_bytes!("../../../assets/icons/settings-2.svg").as_slice()
            }
            "icons/square-terminal.svg" => {
                include_bytes!("../../../assets/icons/square-terminal.svg").as_slice()
            }
            "icons/x.svg" => include_bytes!("../../../assets/icons/x.svg").as_slice(),
            "icons/file-code.svg" => {
                include_bytes!("../../../assets/icons/file-code.svg").as_slice()
            }
            "icons/file-config.svg" => {
                include_bytes!("../../../assets/icons/file-config.svg").as_slice()
            }
            "icons/file-markdown.svg" => {
                include_bytes!("../../../assets/icons/file-markdown.svg").as_slice()
            }
            "icons/file-image.svg" => {
                include_bytes!("../../../assets/icons/file-image.svg").as_slice()
            }
            "icons/file-archive.svg" => {
                include_bytes!("../../../assets/icons/file-archive.svg").as_slice()
            }
            "icons/file-database.svg" => {
                include_bytes!("../../../assets/icons/file-database.svg").as_slice()
            }
            "icons/file-media.svg" => {
                include_bytes!("../../../assets/icons/file-media.svg").as_slice()
            }
            "icons/sun.svg" => include_bytes!("../../../assets/icons/sun.svg").as_slice(),
            "icons/moon.svg" => include_bytes!("../../../assets/icons/moon.svg").as_slice(),
            _ => return gpui_component_assets::Assets.load(path),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|name| name.starts_with(path))
            .map(|name| (*name).into())
            .collect())
    }
}
const ICONS: &[&str] = &[
    "icons/moon.svg",
    "icons/sun.svg",
    "icons/file-media.svg",
    "icons/file-database.svg",
    "icons/file-archive.svg",
    "icons/file-image.svg",
    "icons/file-markdown.svg",
    "icons/file-config.svg",
    "icons/file-code.svg",
    "icons/arrow-up.svg",
    "icons/arrow-down.svg",
    "icons/check.svg",
    "icons/chevron-down.svg",
    "icons/chevron-right.svg",
    "icons/columns-2.svg",
    "icons/file-text.svg",
    "icons/external-link.svg",
    "icons/files.svg",
    "icons/folder-closed.svg",
    "icons/folder-open.svg",
    "icons/folder-plus.svg",
    "icons/folder-x.svg",
    "icons/git-branch.svg",
    "icons/plus.svg",
    "icons/rows-2.svg",
    "icons/search.svg",
    "icons/bell.svg",
    "icons/settings-2.svg",
    "icons/square-terminal.svg",
    "icons/x.svg",
];
