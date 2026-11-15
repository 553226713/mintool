//! Run saved project tasks in the focused built-in terminal.

use crate::app::MintoolApp;
use crate::model::session::SessionKey;
use crate::services::mutex::lock_unpoisoned;
use gpui::*;

impl MintoolApp {
    pub(crate) fn run_project_task(&mut self, id: &str, task: &str, cx: &mut Context<Self>) {
        let key = SessionKey::project(id);
        let Some(pane) = self
            .workspaces
            .get(&key)
            .map(|workspace| workspace.state.focused)
        else {
            self.error = Some("请先打开项目".into());
            cx.notify();
            return;
        };
        let showing_file = self
            .workspaces
            .get(&key)
            .and_then(|workspace| workspace.state.layout.find(pane))
            .is_some_and(|pane| pane.showing_file || pane.remote.is_some());
        if showing_file {
            self.error = Some("请先切换到终端面板再运行任务".into());
            cx.notify();
            return;
        }
        let Some(terminal) = self.terminals.get(&(key.clone(), pane)) else {
            self.error = Some("当前没有可用的内置终端，请新建终端".into());
            cx.notify();
            return;
        };
        let result = {
            let view = terminal.read(cx);
            let mut session = lock_unpoisoned(&view.session);
            session.send(format!("{task}\n").as_bytes())
        };
        match result {
            Ok(()) => {
                self.focus_pending = true;
                cx.notify();
            }
            Err(error) => {
                self.error = Some(format!("运行任务失败：{error:#}"));
                cx.notify();
            }
        }
    }
}
