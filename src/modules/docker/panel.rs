//! Right-hand auxiliary panel for Docker logs/exec sessions.

use super::model::{parse_session_id, DockerView};
use crate::app::MintoolApp;
use crate::model::session::SessionView;
use crate::ui::theme;
use adabraka_ui::components::button::{Button, ButtonSize, ButtonVariant};
use adabraka_ui::components::icon::Icon;
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MintoolApp {
    /// Render the container's logs/terminal in a side panel while the resource
    /// list stays visible in the main area.
    pub(crate) fn render_docker_session_panel(
        &mut self,
        session: &SessionView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(workspace) = self.workspaces.get(&session.key) else {
            return div().into_any_element();
        };
        let layout = workspace.state.layout.clone();
        let Some((container_id, view)) = parse_session_id(&session.key.id) else {
            return div().into_any_element();
        };
        let compose_name = container_id.strip_prefix("compose-").map(str::to_string);
        let name = self
            .modules
            .docker
            .container(&container_id)
            .map(|container| container.name.clone())
            .unwrap_or_else(|| compose_name.clone().unwrap_or_else(|| container_id.clone()));
        let title = match &compose_name {
            Some(project) => format!("Compose · {project}"),
            None => format!("{} · {}", name, view.label()),
        };
        let logs_id = container_id.clone();
        let exec_id = container_id.clone();
        let details_id = container_id.clone();
        let tail_id = container_id.clone();
        let timestamp_id = container_id.clone();
        let tail_label = format!("{} 行", self.modules.docker.log_tail);
        let timestamps = self.modules.docker.log_timestamps;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::background())
            .child(
                div()
                    .h(px(36.))
                    .px_3()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(theme::panel())
                    .border_b_1()
                    .border_color(theme::border())
                    .child(Icon::new("docker").size(px(14.)).color(theme::accent()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        Button::new("docker-panel-logs", "日志")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .selected(view == DockerView::Logs)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_docker_session(logs_id.clone(), DockerView::Logs, cx)
                            })),
                    )
                    .when(compose_name.is_none(), |header| {
                        header.child(
                            Button::new("docker-panel-exec", "终端")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .selected(view == DockerView::Exec)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_docker_session(exec_id.clone(), DockerView::Exec, cx)
                                })),
                        )
                    })
                    .when(compose_name.is_none(), |header| {
                        header.child(
                            Button::new("docker-panel-details", "详情")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.show_container_details(details_id.clone(), window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("docker-panel-close", "")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .icon("x")
                            .tooltip("关闭面板")
                            .on_click(cx.listener(|this, _, _, cx| this.close_docker_session(cx))),
                    ),
            )
            .when(view == DockerView::Logs, |panel| {
                panel.child(
                    div()
                        .h(px(30.))
                        .px_3()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_color(theme::border())
                        .child(
                            Button::new("docker-log-tail", tail_label)
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .tooltip("切换日志行数")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.modules.docker.log_tail =
                                        match this.modules.docker.log_tail {
                                            100 => 500,
                                            500 => 2000,
                                            _ => 100,
                                        };
                                    this.restart_docker_log(tail_id.clone(), cx);
                                })),
                        )
                        .child(
                            Button::new("docker-log-timestamps", "时间戳")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .selected(timestamps)
                                .tooltip("显示日志时间戳")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.modules.docker.log_timestamps =
                                        !this.modules.docker.log_timestamps;
                                    this.restart_docker_log(timestamp_id.clone(), cx);
                                })),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(theme::muted())
                                .child("切换后重新加载日志"),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_layout(session, &layout, window, cx)),
            )
            .into_any_element()
    }
}
