//! Docker confirmations, container details and the run-from-image form.

use crate::app::MintoolApp;
use crate::ui::dialogs::DialogContent;
use crate::ui::theme;
use adabraka_ui::components::input::Input;
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone)]
pub enum DockerDialog {
    Container(String, String),
    Image(String, String),
    Volume(String, String),
    Network(String, String),
    Details { name: String, text: String },
    RunImage { reference: String },
    Prune(super::model::DockerResource),
    PullImage,
    TagImage { id: String, reference: String },
    CreateVolume,
    CreateNetwork,
    Stats { text: String },
}

impl DialogContent for DockerDialog {
    fn title(&self) -> String {
        match self {
            Self::Container(..) => "删除容器".into(),
            Self::Image(..) => "删除镜像".into(),
            Self::Volume(..) => "删除数据卷".into(),
            Self::Network(..) => "删除网络".into(),
            Self::Details { name, .. } => format!("容器详情 · {name}"),
            Self::RunImage { .. } => "从镜像运行容器".into(),
            Self::Prune(resource) => format!("清理未使用的{}", resource.label()),
            Self::PullImage => "拉取镜像".into(),
            Self::TagImage { .. } => "标记镜像".into(),
            Self::CreateVolume => "新建数据卷".into(),
            Self::CreateNetwork => "新建网络".into(),
            Self::Stats { .. } => "容器资源统计".into(),
        }
    }
    fn input_value(&self, _app: &MintoolApp) -> String {
        match self {
            Self::TagImage { reference, .. } => reference.clone(),
            _ => String::new(),
        }
    }
    fn input_hint(&self) -> Option<&'static str> {
        match self {
            Self::PullImage => Some("镜像名称，例如 nginx:latest"),
            Self::TagImage { .. } => Some("新标签，例如 myrepo/nginx:latest"),
            Self::CreateVolume => Some("数据卷名称"),
            Self::CreateNetwork => Some("网络名称"),
            _ => None,
        }
    }
    fn accepts_empty_input(&self) -> bool {
        !matches!(
            self,
            Self::PullImage | Self::TagImage { .. } | Self::CreateVolume | Self::CreateNetwork
        )
    }
    fn has_actions(&self) -> bool {
        !matches!(self, Self::Details { .. } | Self::Stats { .. })
    }
    fn subject(&self) -> Option<String> {
        match self {
            Self::Container(_, name)
            | Self::Image(_, name)
            | Self::Volume(_, name)
            | Self::Network(_, name) => Some(name.clone()),
            _ => None,
        }
    }
    fn note(&self, _app: &MintoolApp) -> Option<String> {
        match self {
            Self::Container(..) => Some("强制删除此容器，容器内未保存的数据会丢失。".into()),
            Self::Image(..) => Some("删除此本地镜像。正在使用该镜像的容器不会被删除。".into()),
            Self::Volume(..) => Some("永久删除此数据卷及其中的数据，无法恢复。".into()),
            Self::Network(..) => {
                Some("删除此 Docker 网络。正在连接该网络的容器会导致删除失败。".into())
            }
            Self::RunImage { reference } => Some(format!("镜像：{reference}")),
            Self::Prune(resource) => Some(format!(
                "清理未使用的{}。运行中的容器和仍被使用的资源不会删除。",
                resource.label()
            )),
            Self::Details { .. }
            | Self::PullImage
            | Self::TagImage { .. }
            | Self::CreateVolume
            | Self::CreateNetwork
            | Self::Stats { .. } => None,
        }
    }
    fn confirm_label(&self) -> &'static str {
        match self {
            Self::RunImage { .. } => "运行",
            Self::PullImage => "拉取",
            Self::TagImage { .. } => "标记",
            Self::CreateVolume | Self::CreateNetwork => "创建",
            Self::Prune(_) => "清理",
            _ => "删除",
        }
    }
    fn render_extra(&self, app: &MintoolApp, _cx: &mut Context<MintoolApp>) -> Option<AnyElement> {
        match self {
            Self::Details { text, .. } | Self::Stats { text } => Some(
                div()
                    .id("docker-details")
                    .w_full()
                    .max_h(px(480.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .font_family("DejaVu Sans Mono")
                    .text_size(px(11.))
                    .text_color(theme::text())
                    .children(text.lines().map(|line| div().child(line.to_string())))
                    .into_any_element(),
            ),
            Self::RunImage { .. } => {
                let form = &app.modules.docker.run_form;
                let field = |label: &'static str, input: Input| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme::muted())
                                .child(label),
                        )
                        .child(
                            input
                                .map(|mut input| {
                                    input.style().align_items = Some(AlignItems::Stretch);
                                    input
                                })
                                .w_full(),
                        )
                };
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(field("容器名称（可选）", Input::new(&form.name)))
                        .child(field("端口映射（逗号或空格分隔）", Input::new(&form.ports)))
                        .child(field(
                            "环境变量（KEY=VALUE，空格分隔）",
                            Input::new(&form.env),
                        ))
                        .child(field("挂载（源:目标，逗号分隔）", Input::new(&form.mounts)))
                        .child(field("启动命令（可选）", Input::new(&form.command)))
                        .into_any_element(),
                )
            }
            _ => None,
        }
    }
    fn prepare(&self, app: &mut MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
        if !matches!(self, Self::RunImage { .. }) {
            return;
        }
        let form = &app.modules.docker.run_form;
        for input in [
            &form.name,
            &form.ports,
            &form.env,
            &form.mounts,
            &form.command,
        ] {
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
    }
    fn focus(&self, app: &MintoolApp, window: &mut Window, cx: &mut Context<MintoolApp>) {
        match self {
            Self::RunImage { .. } => app
                .modules
                .docker
                .run_form
                .name
                .read(cx)
                .focus_handle(cx)
                .focus(window),
            Self::Details { .. } => app.dialog_focus.focus(window),
            _ => app.dialog_input.read(cx).focus_handle(cx).focus(window),
        }
    }
    fn submit(
        &self,
        app: &mut MintoolApp,
        value: String,
        cx: &mut Context<MintoolApp>,
    ) -> anyhow::Result<()> {
        match self {
            Self::Container(id, _) => app.remove_docker_container(id.clone(), cx),
            Self::Image(id, _) => app.remove_docker_image(id.clone(), cx),
            Self::Volume(name, _) => app.remove_docker_volume(name.clone(), cx),
            Self::Network(id, _) => app.remove_docker_network(id.clone(), cx),
            Self::Details { .. } => {}
            Self::Prune(resource) => app.prune_docker_resource(*resource, cx),
            Self::PullImage => {
                app.pull_docker_image(value, cx);
            }
            Self::TagImage { id, .. } => {
                app.tag_docker_image(id.clone(), value, cx);
            }
            Self::CreateVolume => app.create_docker_volume(value, cx),
            Self::CreateNetwork => app.create_docker_network(value, cx),
            Self::Stats { .. } => {}
            Self::RunImage { reference } => {
                let form = &app.modules.docker.run_form;
                app.run_docker_image(
                    reference.clone(),
                    form.name.read(cx).content().trim().to_string(),
                    form.ports.read(cx).content().trim().to_string(),
                    form.env.read(cx).content().trim().to_string(),
                    form.mounts.read(cx).content().trim().to_string(),
                    form.command.read(cx).content().trim().to_string(),
                    cx,
                );
            }
        }
        Ok(())
    }
}
