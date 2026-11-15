use super::model::{
    DockerContainer, DockerImage, DockerNetwork, DockerResource, DockerView, DockerVolume,
};
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

impl MintoolApp {
    pub(crate) fn render_docker_resources(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let resource = self.modules.docker.resource;
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
                    .child(
                        Button::new("refresh-docker-resources", "刷新")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .disabled(loading)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_docker(cx))),
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

    fn docker_container_resource_row(
        &self,
        container: &DockerContainer,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.modules.docker.selected.as_deref() == Some(container.id.as_str());
        let logs_id = container.id.clone();
        let exec_id = container.id.clone();
        let action_id = container.id.clone();
        let remove_id = container.id.clone();
        let remove_name = container.name.clone();
        let menu_container = container.clone();
        let menu_id = container.id.clone();
        let (action, action_label) = if container.paused() {
            ("unpause", "恢复")
        } else if container.running() {
            ("stop", "停止")
        } else {
            ("start", "启动")
        };
        let status_color = if container.running() {
            theme::success()
        } else if container.paused() {
            theme::warning()
        } else if container.state == "dead" {
            theme::danger()
        } else {
            theme::muted()
        };
        resource_row(
            SharedString::from(format!("docker-resource-container-{}", container.id)),
            selected,
            52.,
        )
        .hover(move |style| {
            style.bg(if selected {
                theme::selected_hover()
            } else {
                theme::elevated()
            })
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.select_docker_container(menu_id.clone(), cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                this.select_docker_container(menu_container.id.clone(), cx);
                this.docker_container_menu(menu_container.clone(), event.position, cx);
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(div().size(px(7.)).rounded_full().bg(status_color))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(container.name.clone()),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(10.))
                                .text_color(theme::muted())
                                .child(container.image.clone()),
                        ),
                ),
        )
        .child(
            div()
                .w(px(120.))
                .text_size(px(10.))
                .text_color(status_color)
                .child(container.state_label().to_string()),
        )
        .child(
            div()
                .w(px(150.))
                .truncate()
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(if container.ports.is_empty() {
                    "—".to_string()
                } else {
                    container.ports.clone()
                }),
        )
        .child(
            div()
                .w(px(230.))
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .child(
                    Button::new(
                        SharedString::from(format!("docker-logs-{}", container.id)),
                        "日志",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_docker_session(logs_id.clone(), DockerView::Logs, cx)
                    })),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("docker-exec-{}", container.id)),
                        "终端",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .disabled(!container.running())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_docker_session(exec_id.clone(), DockerView::Exec, cx)
                    })),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("docker-toggle-{}", container.id)),
                        action_label,
                    )
                    .variant(ButtonVariant::Outline)
                    .size(ButtonSize::Sm)
                    .disabled(container.state == "dead")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.docker_action(action_id.clone(), action, cx)
                    })),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("docker-remove-{}", container.id)),
                        "",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .icon("x")
                    .tooltip("删除容器")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_dialog(
                            Dialog::Docker(DockerDialog::Container(
                                remove_id.clone(),
                                remove_name.clone(),
                            )),
                            window,
                            cx,
                        )
                    })),
                ),
        )
        .into_any_element()
    }

    fn docker_image_resource_row(&self, image: &DockerImage, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.modules.docker.selected.as_deref() == Some(image.id.as_str());
        let copy_reference = image.reference();
        let copy_id = image.id.clone();
        let remove_id = image.id.clone();
        let remove_name = image.reference();
        let menu_image = image.clone();
        let menu_id = image.id.clone();
        resource_row(
            SharedString::from(format!("docker-resource-image-{}", image.id)),
            selected,
            48.,
        )
        .hover(move |style| {
            style.bg(if selected {
                theme::selected_hover()
            } else {
                theme::elevated()
            })
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.modules.docker.selected = Some(menu_id.clone());
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                this.modules.docker.selected = Some(menu_image.id.clone());
                this.docker_image_menu(menu_image.clone(), event.position, cx);
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.))
                .child(copy_reference.clone()),
        )
        .child(
            div()
                .w(px(120.))
                .truncate()
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(short_id(&image.id)),
        )
        .child(
            div()
                .w(px(150.))
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(image.size.clone()),
        )
        .child(
            div()
                .w(px(230.))
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .child(
                    Button::new(
                        SharedString::from(format!("docker-copy-image-{}", image.id)),
                        "复制",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_id.clone()));
                    })),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("docker-remove-image-{}", image.id)),
                        "",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .icon("x")
                    .tooltip("删除镜像")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_dialog(
                            Dialog::Docker(DockerDialog::Image(
                                remove_id.clone(),
                                remove_name.clone(),
                            )),
                            window,
                            cx,
                        )
                    })),
                ),
        )
        .into_any_element()
    }

    fn docker_volume_resource_row(
        &self,
        volume: &DockerVolume,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.modules.docker.selected.as_deref() == Some(volume.name.as_str());
        let select_name = volume.name.clone();
        let copy_path = volume.mountpoint.clone();
        let remove_name = volume.name.clone();
        let dialog_name = volume.name.clone();
        resource_row(
            SharedString::from(format!("docker-resource-volume-{}", volume.name)),
            selected,
            48.,
        )
        .hover(move |style| {
            style.bg(if selected {
                theme::selected_hover()
            } else {
                theme::elevated()
            })
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.modules.docker.selected = Some(select_name.clone());
            cx.notify();
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.))
                .child(volume.name.clone()),
        )
        .child(
            div()
                .w(px(120.))
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(volume.driver.clone()),
        )
        .child(
            div()
                .w(px(150.))
                .truncate()
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(if volume.mountpoint.is_empty() {
                    "—".to_string()
                } else {
                    volume.mountpoint.clone()
                }),
        )
        .child(
            div()
                .w(px(230.))
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .child(
                    Button::new(
                        SharedString::from(format!("docker-copy-volume-{}", volume.name)),
                        "复制路径",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .disabled(copy_path.is_empty())
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()));
                    })),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("docker-remove-volume-{}", volume.name)),
                        "",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .icon("x")
                    .tooltip("删除卷")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_dialog(
                            Dialog::Docker(DockerDialog::Volume(
                                remove_name.clone(),
                                dialog_name.clone(),
                            )),
                            window,
                            cx,
                        )
                    })),
                ),
        )
        .into_any_element()
    }

    fn docker_network_resource_row(
        &self,
        network: &DockerNetwork,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.modules.docker.selected.as_deref() == Some(network.id.as_str());
        let select_id = network.id.clone();
        let copy_id_value = network.id.clone();
        let remove_id = network.id.clone();
        let remove_name = network.name.clone();
        resource_row(
            SharedString::from(format!("docker-resource-network-{}", network.id)),
            selected,
            48.,
        )
        .hover(move |style| {
            style.bg(if selected {
                theme::selected_hover()
            } else {
                theme::elevated()
            })
        })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.modules.docker.selected = Some(select_id.clone());
            cx.notify();
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .truncate()
                        .text_size(px(12.))
                        .child(network.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(9.))
                        .text_color(theme::muted())
                        .child(short_id(&network.id)),
                ),
        )
        .child(
            div()
                .w(px(120.))
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(network.driver.clone()),
        )
        .child(
            div()
                .w(px(150.))
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(network.scope.clone()),
        )
        .child(
            div()
                .w(px(230.))
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .child(
                    Button::new(
                        SharedString::from(format!("docker-copy-network-{}", network.id)),
                        "复制 ID",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_id_value.clone()));
                    })),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("docker-remove-network-{}", network.id)),
                        "",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .icon("x")
                    .tooltip(if network.builtin() {
                        "Docker 内置网络不可删除"
                    } else {
                        "删除网络"
                    })
                    .disabled(network.builtin())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_dialog(
                            Dialog::Docker(DockerDialog::Network(
                                remove_id.clone(),
                                remove_name.clone(),
                            )),
                            window,
                            cx,
                        )
                    })),
                ),
        )
        .into_any_element()
    }
}

fn resource_row(id: SharedString, selected: bool, height: f32) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(height))
        .px_3()
        .flex()
        .items_center()
        .gap_3()
        .rounded_lg()
        .border_1()
        .border_color(if selected {
            theme::selected_border()
        } else {
            theme::background()
        })
        .bg(if selected {
            theme::selected()
        } else {
            theme::background()
        })
}

fn docker_container_matches(container: &DockerContainer, filter: &str) -> bool {
    filter.is_empty()
        || container.name.to_lowercase().contains(filter)
        || container.image.to_lowercase().contains(filter)
        || container.id.to_lowercase().contains(filter)
}

fn short_id(id: &str) -> String {
    let value = id.strip_prefix("sha256:").unwrap_or(id);
    value.chars().take(12).collect()
}
