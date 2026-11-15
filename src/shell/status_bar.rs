//! Bottom status bar: current location, git state and terminal count.

use crate::app::MintoolApp;
use crate::modules::ModuleId;
use crate::ui::theme;
use adabraka_ui::components::icon::Icon;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MintoolApp {
    pub(crate) fn status_bar(&self, cx: &Context<Self>) -> impl IntoElement {
        let session = self.current_session();
        let key = self.active_session_key();
        let workspace = key.as_ref().and_then(|key| self.workspaces.get(key));
        let location = match self.modules.active {
            ModuleId::Projects => self
                .current()
                .map(|project| project.path.display().to_string())
                .unwrap_or_else(|| "就绪".into()),
            ModuleId::Ssh => session
                .as_ref()
                .map(|session| session.subtitle.clone())
                .unwrap_or_else(|| "未选择 SSH 连接".into()),
            ModuleId::Docker => {
                let running = self
                    .modules
                    .docker
                    .containers
                    .iter()
                    .filter(|container| container.running())
                    .count();
                format!(
                    "Docker · {running} 个运行中 / {} 个容器",
                    self.modules.docker.containers.len()
                )
            }
        };
        let terminals = workspace.map_or(0, |workspace| workspace.terminals.len())
            + key.as_ref().map_or(0, |key| {
                self.system_terminals
                    .keys()
                    .filter(|(session, _)| session == key)
                    .count()
            });
        div()
            .h(px(29.))
            .flex_none()
            .px_4()
            .flex()
            .items_center()
            .gap_3()
            .text_size(px(11.))
            .text_color(theme::muted())
            .border_t_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(div().size(px(6.)).rounded_full().bg(theme::success()))
            .child(div().flex_1().min_w_0().truncate().child(location))
            .when(self.modules.active == ModuleId::Projects, |bar| {
                bar.when_some(
                    workspace.filter(|workspace| workspace.git.root.is_some()),
                    |bar, workspace| {
                        bar.child(Icon::new("git-branch").size(px(13.)))
                            .child(
                                div()
                                    .text_color(theme::accent())
                                    .child(workspace.git.branch.clone()),
                            )
                            .child(format!("{} 项变更", workspace.git.changes.len()))
                    },
                )
            })
            .child(format!("{terminals} 个终端"))
            .child("UTF-8")
            .child("Adabraka UI")
            .map(|bar| {
                let _ = cx;
                bar
            })
    }
}
