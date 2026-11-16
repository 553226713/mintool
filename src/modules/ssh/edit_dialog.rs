use crate::{app::MintoolApp, ui::dialogs::DialogContent};
use gpui::*;

#[derive(Clone)]
pub struct RemoteEditDialog {
    pub quit: bool,
}
impl DialogContent for RemoteEditDialog {
    fn title(&self) -> String {
        "远程文件尚未保存".into()
    }
    fn accepts_empty_input(&self) -> bool {
        true
    }
    fn confirm_label(&self) -> &'static str {
        if self.quit {
            "保存全部并退出"
        } else {
            "保存并关闭"
        }
    }
    fn discard_label(&self) -> Option<&'static str> {
        Some(if self.quit {
            "丢弃全部并退出"
        } else {
            "丢弃修改"
        })
    }
    fn note(&self, _: &MintoolApp) -> Option<String> {
        Some(
            if self.quit {
                "退出会丢失本地和远程文件的未保存修改。"
            } else {
                "关闭会丢失当前远程文件的修改；取消可继续编辑。"
            }
            .into(),
        )
    }
    fn focus(&self, app: &MintoolApp, window: &mut Window, _: &mut Context<MintoolApp>) {
        app.dialog_focus.focus(window);
    }
    fn discard(&self, app: &mut MintoolApp, cx: &mut Context<MintoolApp>) {
        app.remote_edit = None;
        if self.quit {
            app.quit_confirmed = true;
            cx.quit();
        }
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        _: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        if self.quit {
            app.save_all_previews(cx)?;
        }
        app.save_remote_edit(self.quit, cx);
        Ok(())
    }
}
