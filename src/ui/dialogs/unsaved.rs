//! Confirmation shown when closing a dirty buffer or quitting.

use super::DialogContent;
use crate::app::MintoolApp;
use crate::model::session::SessionKey;
use gpui::*;

#[derive(Clone)]
pub enum UnsavedTarget {
    Close { session: SessionKey, pane: u64 },
    Quit,
}

#[derive(Clone)]
pub struct UnsavedDialog {
    target: UnsavedTarget,
}

impl UnsavedDialog {
    pub fn close(session: SessionKey, pane: u64) -> Self {
        Self {
            target: UnsavedTarget::Close { session, pane },
        }
    }
    pub fn quit() -> Self {
        Self {
            target: UnsavedTarget::Quit,
        }
    }
}

impl DialogContent for UnsavedDialog {
    fn title(&self) -> String {
        "未保存的修改".into()
    }
    fn accepts_empty_input(&self) -> bool {
        true
    }
    fn confirm_label(&self) -> &'static str {
        match &self.target {
            UnsavedTarget::Close { .. } => "保存并关闭",
            UnsavedTarget::Quit => "保存并退出",
        }
    }
    fn focus(&self, app: &MintoolApp, window: &mut Window, _cx: &mut Context<MintoolApp>) {
        app.dialog_focus.focus(window);
    }
    fn discard_label(&self) -> Option<&'static str> {
        Some("不保存")
    }
    fn live_note(&self, app: &MintoolApp, cx: &Context<MintoolApp>) -> Option<String> {
        Some(match &self.target {
            UnsavedTarget::Close { session, pane } => {
                let name = app
                    .previews
                    .get(&(session.clone(), *pane))
                    .and_then(|(path, _, _)| path.file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("「{name}」尚未保存，关闭标签页会丢失这些修改。")
            }
            UnsavedTarget::Quit => {
                let count = app
                    .previews
                    .values()
                    .filter(|(_, _, state)| state.read(cx).has_unsaved_changes())
                    .count();
                format!("有 {count} 个文件尚未保存，退出后会丢失这些修改。")
            }
        })
    }
    fn discard(&self, app: &mut MintoolApp, cx: &mut Context<MintoolApp>) {
        match &self.target {
            UnsavedTarget::Close { session, pane } => app.close_pane(session, *pane, cx),
            UnsavedTarget::Quit => {
                app.quit_confirmed = true;
                cx.quit();
            }
        }
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        _value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        match &self.target {
            UnsavedTarget::Close { session, pane } => {
                app.save_preview(session, *pane, cx)?;
                app.close_pane(session, *pane, cx);
                Ok(())
            }
            UnsavedTarget::Quit => {
                app.save_all_previews(cx)?;
                app.quit_confirmed = true;
                cx.quit();
                Ok(())
            }
        }
    }
}
