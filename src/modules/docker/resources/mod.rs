use super::model::DockerResource;
use super::resource_summary::{
    docker_resource_total, docker_table_header, filtered_docker_resource_count,
};
use crate::app::MintoolApp;
use crate::modules::docker::dialogs::DockerDialog;
use crate::ui::{dialogs::Dialog, theme};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
    input::{Input, InputSize},
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use rows::{docker_compose_service_row, docker_container_matches};

mod rows;

impl MintoolApp {
    pub(crate) fn render_docker_resources(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let resource = self.modules.docker.resource;
        if resource == DockerResource::Compose {
            self.ensure_compose_loaded(cx);
        }
        let filter = self.modules.docker.filter.read(cx).content().to_lowercase();
        let loading = self.modules.docker.loading;
        let count = filtered_docker_resource_count(&self.modules.docker, resource, &filter);
        let total = docker_resource_total(&self.modules.docker, resource);
        let error = self.modules.docker.error.clone();
        let mut rows = Vec::new();
        match resource {
            DockerResource::Containers => {
                rows.push(docker_table_header(&["容器", "状态", "端口", "操作"]));
                let containers: Vec<_> = self
                    .modules
                    .docker
                    .containers
                    .iter()
                    .filter(|container| docker_container_matches(container, &filter))
                    .cloned()
                    .collect();
                for container in &containers {
                    rows.push(self.docker_container_resource_row(container, cx));
                }
            }
            DockerResource::Images => {
                rows.push(docker_table_header(&["镜像", "镜像 ID", "大小", "操作"]));
                let images: Vec<_> = self
                    .modules
                    .docker
                    .images
                    .iter()
                    .filter(|image| {
                        filter.is_empty()
                            || image.reference().to_lowercase().contains(&filter)
                            || image.id.to_lowercase().contains(&filter)
                    })
                    .cloned()
                    .collect();
                for image in &images {
                    rows.push(self.docker_image_resource_row(image, cx));
                }
            }
            DockerResource::Volumes => {
                rows.push(docker_table_header(&["卷", "驱动", "挂载点", "操作"]));
                let volumes: Vec<_> = self
                    .modules
                    .docker
                    .volumes
                    .iter()
                    .filter(|volume| {
                        filter.is_empty()
                            || volume.name.to_lowercase().contains(&filter)
                            || volume.driver.to_lowercase().contains(&filter)
                            || volume.mountpoint.to_lowercase().contains(&filter)
                    })
                    .cloned()
                    .collect();
                for volume in &volumes {
                    rows.push(self.docker_volume_resource_row(volume, cx));
                }
            }
            DockerResource::Compose => {
                if self.modules.docker.compose_loading {
                    rows.push(
                        div()
                            .p_4()
                            .text_color(theme::muted())
                            .child("正在读取 Compose 项目…")
                            .into_any_element(),
                    );
                }
                let projects: Vec<_> = self
                    .modules
                    .docker
                    .compose
                    .iter()
                    .filter(|project| {
                        filter.is_empty()
                            || project.name.to_lowercase().contains(&filter)
                            || project.services.iter().any(|service| {
                                service.service.to_lowercase().contains(&filter)
                                    || service.name.to_lowercase().contains(&filter)
                            })
                    })
                    .cloned()
                    .collect();
                for project in &projects {
                    rows.push(self.docker_compose_project_row(project, cx));
                    for service in &project.services {
                        rows.push(docker_compose_service_row(service));
                    }
                }
            }
            DockerResource::Networks => {
                rows.push(docker_table_header(&["网络", "驱动", "范围", "操作"]));
                let networks: Vec<_> = self
                    .modules
                    .docker
                    .networks
                    .iter()
                    .filter(|network| {
                        filter.is_empty()
                            || network.name.to_lowercase().contains(&filter)
                            || network.driver.to_lowercase().contains(&filter)
                            || network.id.to_lowercase().contains(&filter)
                    })
                    .cloned()
                    .collect();
                for network in &networks {
                    rows.push(self.docker_network_resource_row(network, cx));
                }
            }
        }
        if count == 0 && !loading {
            rows.push(
                div()
                    .flex_1()
                    .min_h(px(180.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .text_color(theme::muted())
                    .child(if filter.is_empty() {
                        format!("没有{}资源", resource.label())
                    } else {
                        "没有匹配的资源".to_string()
                    })
                    .into_any_element(),
            );
        }
        div()
            .id("docker-resource-content")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::background())
            .child(
                div()
                    .flex_none()
                    .px_5()
                    .py_4()
                    .border_b_1()
                    .border_color(theme::border())
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Icon::new(resource.icon())
                                            .size(px(19.))
                                            .color(theme::accent()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(17.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(resource.label()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(1.))
                                            .rounded_full()
                                            .bg(theme::elevated())
                                            .text_size(px(10.))
                                            .text_color(theme::muted())
                                            .child(format!("{count} / {total}")),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme::muted())
                                    .child(resource.description()),
                            ),
                    )
                    .child(
                        Input::new(&self.modules.docker.filter)
                            .map(|mut input| {
                                input.style().align_items = Some(AlignItems::Stretch);
                                input
                            })
                            .w(px(220.))
                            .size(InputSize::Sm)
                            .placeholder("筛选当前分类…")
                            .prefix(Icon::new("search").size(px(13.)).color(theme::muted())),
                    )
                    .when(resource == DockerResource::Images, |header| {
                        header.child(
                            Button::new("pull-docker-image", "拉取…")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_dialog(
                                        Dialog::Docker(DockerDialog::PullImage),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                    })
                    .when(resource == DockerResource::Containers, |header| {
                        header.child(
                            Button::new("docker-stats", "统计")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_docker_stats(window, cx)
                                })),
                        )
                    })
                    .when(resource == DockerResource::Volumes, |header| {
                        header.child(
                            Button::new("create-docker-volume", "新建…")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_dialog(
                                        Dialog::Docker(DockerDialog::CreateVolume),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                    })
                    .when(resource == DockerResource::Networks, |header| {
                        header.child(
                            Button::new("create-docker-network", "新建…")
                                .variant(ButtonVariant::Outline)
                                .size(ButtonSize::Sm)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.show_dialog(
                                        Dialog::Docker(DockerDialog::CreateNetwork),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                    })
                    .when(resource != DockerResource::Compose, |header| {
                        header.child(
                            Button::new("prune-docker-resources", "清理")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Sm)
                                .tooltip("清理未使用的资源")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.show_dialog(
                                        Dialog::Docker(DockerDialog::Prune(resource)),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                    })
                    .child(
                        Button::new("refresh-docker-resources", "刷新")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .disabled(loading)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if resource == DockerResource::Compose {
                                    this.modules.docker.compose_loaded = false;
                                    this.load_compose(cx);
                                } else {
                                    this.refresh_docker(cx);
                                }
                            })),
                    ),
            )
            .when_some(error, |page, error| {
                page.child(
                    div()
                        .mx_5()
                        .mt_4()
                        .p_3()
                        .rounded_md()
                        .bg(theme::error_background())
                        .text_size(px(11.))
                        .text_color(theme::danger())
                        .child(error),
                )
            })
            .child(
                div()
                    .id("docker-resource-table")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_4()
                    .pb_4()
                    .children(rows),
            )
            .into_any_element()
    }
}
