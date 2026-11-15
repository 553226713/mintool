use crate::app::MintoolApp;
use crate::modules::projects::dialogs::ProjectDialog;
use crate::modules::ssh::dialogs::SshDialog;
use crate::modules::ModuleId;
use crate::ui::{dialogs::Dialog, theme};
use adabraka_ui::components::button::{Button, ButtonVariant};
use adabraka_ui::components::icon::Icon;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MintoolApp {
    pub(crate) fn empty_state(&mut self, cx: &mut Context<Self>) -> AnyElement {
        match self.modules.active {
            ModuleId::Projects => div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    Icon::new("square-terminal")
                        .size(px(46.))
                        .text_color(theme::accent()),
                )
                .child(
                    div()
                        .text_size(px(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("每个项目，都有自己的工作现场"),
                )
                .child(
                    div()
                        .text_color(theme::muted())
                        .child("独立终端 · 自由分屏 · Git 文件树"),
                )
                .child(
                    Button::new("first-project", "添加项目目录")
                        .variant(ButtonVariant::Default)
                        .icon("plus")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_dialog(
                                Dialog::Project(ProjectDialog::AddProject(None)),
                                window,
                                cx,
                            )
                        })),
                )
                .into_any_element(),
            ModuleId::Docker => {
                let error = self.modules.docker.error.clone();
                let total = self.modules.docker.containers.len();
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(
                        Icon::new("docker")
                            .size(px(46.))
                            .text_color(theme::accent()),
                    )
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if error.is_some() {
                                "无法连接本地 Docker"
                            } else if total == 0 {
                                "本地没有容器"
                            } else {
                                "选择容器查看日志或进入终端"
                            }),
                    )
                    .when_some(error, |state, error| {
                        state.child(
                            div()
                                .max_w(px(560.))
                                .text_color(theme::danger())
                                .child(error),
                        )
                    })
                    .child(div().text_color(theme::muted()).child(if total == 0 {
                        "确认 Docker 服务已启动；也可以右键容器进入终端或启停容器"
                    } else {
                        "日志和终端在右侧面板打开，关闭面板后可继续浏览资源"
                    }))
                    .child(
                        Button::new("refresh-docker-empty", "刷新")
                            .variant(ButtonVariant::Outline)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_docker(cx))),
                    )
                    .into_any_element()
            }
            ModuleId::Ssh => {
                let has_connections = !self.modules.ssh.registry.connections.is_empty();
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(
                        Icon::new("square-terminal")
                            .size(px(46.))
                            .text_color(theme::accent()),
                    )
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if has_connections {
                                "双击左侧连接开始会话"
                            } else {
                                "连接远程主机"
                            }),
                    )
                    .child(div().text_color(theme::muted()).child(if has_connections {
                        "单击只选中连接，双击或右键“连接”打开终端"
                    } else {
                        "保存连接配置 · 同一连接多会话 · 支持密码与密钥"
                    }))
                    .when(!has_connections, |state| {
                        state.child(
                            Button::new("first-ssh", "新建 SSH 连接")
                                .variant(ButtonVariant::Default)
                                .icon("plus")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_dialog(
                                        Dialog::Ssh(SshDialog::Connection {
                                            edit: None,
                                            group: None,
                                        }),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                    })
                    .into_any_element()
            }
        }
    }
}
