//! Prompt shown when a dirty buffer's file changed on disk.

use super::DialogContent;
use crate::app::MintoolApp;
use crate::model::session::SessionKey;
use gpui::*;
use std::path::PathBuf;

#[derive(Clone)]
pub struct ConflictDialog {
    session: SessionKey,
    pane: u64,
    path: PathBuf,
    disk_text: String,
}

impl ConflictDialog {
    pub fn new(session: SessionKey, pane: u64, path: PathBuf, disk_text: String) -> Self {
        Self {
            session,
            pane,
            path,
            disk_text,
        }
    }
}

impl DialogContent for ConflictDialog {
    fn title(&self) -> String {
        "文件已被外部修改".into()
    }
    fn accepts_empty_input(&self) -> bool {
        true
    }
    fn note(&self, _app: &MintoolApp) -> Option<String> {
        let name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string());
        Some(format!(
            "「{name}」在磁盘上已被修改，当前编辑器还有未保存内容。"
        ))
    }
    fn confirm_label(&self) -> &'static str {
        "加载磁盘版本"
    }
    fn discard_label(&self) -> Option<&'static str> {
        Some("保留我的修改")
    }
    fn focus(&self, app: &MintoolApp, window: &mut Window, _cx: &mut Context<MintoolApp>) {
        app.dialog_focus.focus(window);
    }
    fn discard(&self, app: &mut MintoolApp, cx: &mut Context<MintoolApp>) {
        app.keep_external_conflict(&self.session, self.pane, &self.disk_text, cx);
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        _value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        app.reload_external_conflict(&self.session, self.pane, &self.disk_text, cx);
        Ok(())
    }
}
