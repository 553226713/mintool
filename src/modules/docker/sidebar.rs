use super::model::DockerResource;
use crate::app::MintoolApp;
use crate::ui::theme;
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
};
use gpui::*;

impl MintoolApp {
    pub(crate) fn render_docker_sidebar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = self.modules.docker.resource;
        let loading = self.modules.docker.loading;
        let mut rows = Vec::new();
        for resource in DockerResource::ALL {
            let selected = active == resource;
            let count = self.docker_resource_count(resource);
            rows.push(
                div()
                    .id(SharedString::from(format!(
                        "docker-resource-{}",
                        resource.label()
                    )))
                    .mx_3()
                    .my(px(3.))
                    .h(px(40.))
                    .px_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(if selected {
                        theme::selected_border()
                    } else {
                        theme::panel()
                    })
                    .bg(if selected {
                        theme::selected()
                    } else {
                        theme::panel()
                    })
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .hover(move |style| {
                        style.bg(if selected {
                            theme::selected_hover()
                        } else {
                            theme::elevated()
                        })
                    })
                    .child(Icon::new(resource.icon()).size(px(15.)).color(if selected {
                        theme::accent()
                    } else {
                        theme::muted()
                    }))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(if selected {
                                theme::accent()
                            } else {
                                theme::text()
                            })
                            .child(resource.label()),
                    )
                    .child(
                        div()
                            .min_w(px(22.))
                            .px_1()
                            .py(px(1.))
                            .rounded_full()
                            .bg(theme::background())
                            .text_size(px(9.))
                            .text_color(theme::muted())
                            .text_center()
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_docker_resource(resource, cx);
                    }))
                    .into_any_element(),
            );
        }
        div()
            .id("docker-navigation")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::panel())
            .child(
                div()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Docker"),
                            )
                            .child(div().text_size(px(10.)).text_color(theme::muted()).child(
                                if loading {
                                    "正在读取资源…"
                                } else {
                                    "本地资源"
                                },
                            )),
                    )
                    .child(
                        Button::new("refresh-docker", "刷新")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .disabled(loading)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_docker(cx))),
                    ),
            )
            .child(
                div()
                    .px_5()
                    .pt_3()
                    .pb_1()
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::muted())
                    .child("资源类型"),
            )
            .child(
                div()
                    .id("docker-resource-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(theme::border())
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_size(px(10.))
                    .text_color(theme::muted())
                    .child("每 5 秒自动刷新，操作后立即更新")
                    .child("删除容器会同时停止容器进程"),
            )
            .into_any_element()
    }

    fn docker_resource_count(&self, resource: DockerResource) -> usize {
        match resource {
            DockerResource::Containers => self.modules.docker.containers.len(),
            DockerResource::Images => self.modules.docker.images.len(),
            DockerResource::Volumes => self.modules.docker.volumes.len(),
            DockerResource::Networks => self.modules.docker.networks.len(),
            DockerResource::Compose => self.modules.docker.compose.len(),
        }
    }
}
