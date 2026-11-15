use crate::app::MintoolApp;
use crate::modules::docker::dialogs::DockerDialog;
use crate::modules::docker::model::{
    ComposeProject, ComposeService, DockerContainer, DockerImage, DockerNetwork, DockerView,
    DockerVolume,
};
use crate::ui::dialogs::Dialog;
use crate::ui::theme;
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MintoolApp {
    pub(super) fn docker_container_resource_row(
        &self,
        container: &DockerContainer,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.modules.docker.selected.as_deref() == Some(container.id.as_str());
        let logs_id = container.id.clone();
        let exec_id = container.id.clone();
        let details_id = container.id.clone();
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
                .w(px(300.))
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .child(
                    Button::new(
                        SharedString::from(format!("docker-details-{}", container.id)),
                        "详情",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_container_details(details_id.clone(), window, cx)
                    })),
                )
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

    pub(super) fn docker_compose_project_row(
        &self,
        project: &ComposeProject,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let logs_id = project.name.clone();
        let up_id = project.name.clone();
        let down_id = project.name.clone();
        let restart_id = project.name.clone();
        div()
            .id(SharedString::from(format!("compose-{}", project.name)))
            .mt_4()
            .mb_1()
            .h(px(38.))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .child(Icon::new("files").size(px(15.)).color(theme::accent()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(project.name.clone()),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(theme::muted())
                    .child(project.status.clone()),
            )
            .child(
                Button::new(
                    SharedString::from(format!("compose-logs-{}", project.name)),
                    "日志",
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(
                    cx.listener(move |this, _, _, cx| this.open_compose_logs(logs_id.clone(), cx)),
                ),
            )
            .child(
                Button::new(
                    SharedString::from(format!("compose-up-{}", project.name)),
                    "启动",
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(
                    cx.listener(move |this, _, _, cx| this.compose_action(up_id.clone(), "up", cx)),
                ),
            )
            .child(
                Button::new(
                    SharedString::from(format!("compose-down-{}", project.name)),
                    "停止",
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.compose_action(down_id.clone(), "down", cx)
                })),
            )
            .child(
                Button::new(
                    SharedString::from(format!("compose-restart-{}", project.name)),
                    "重启",
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.compose_action(restart_id.clone(), "restart", cx)
                })),
            )
            .into_any_element()
    }

    pub(super) fn docker_image_resource_row(
        &self,
        image: &DockerImage,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.modules.docker.selected.as_deref() == Some(image.id.as_str());
        let copy_reference = image.reference();
        let tag_reference = image.reference();
        let tag_id = image.id.clone();
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
                .w(px(300.))
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .child(
                    Button::new(
                        SharedString::from(format!("docker-run-image-{}", image.id)),
                        "运行",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_dialog(
                            Dialog::Docker(DockerDialog::RunImage {
                                reference: copy_reference.clone(),
                            }),
                            window,
                            cx,
                        )
                    })),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("docker-tag-image-{}", image.id)),
                        "标记",
                    )
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Sm)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_dialog(
                            Dialog::Docker(DockerDialog::TagImage {
                                id: tag_id.clone(),
                                reference: tag_reference.clone(),
                            }),
                            window,
                            cx,
                        )
                    })),
                )
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

    pub(super) fn docker_volume_resource_row(
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

    pub(super) fn docker_network_resource_row(
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

pub(super) fn docker_compose_service_row(service: &ComposeService) -> AnyElement {
    let running = service.state == "running";
    let color = if running {
        theme::success()
    } else if service.state == "exited" {
        theme::muted()
    } else {
        theme::warning()
    };
    div()
        .h(px(28.))
        .pl(px(36.))
        .pr_3()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(11.))
        .child(div().size(px(6.)).rounded_full().bg(color))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(if service.service.is_empty() {
                    service.name.clone()
                } else {
                    service.service.clone()
                }),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(theme::muted())
                .child(service.state.clone()),
        )
        .when(!service.ports.is_empty(), |row| {
            row.child(
                div()
                    .text_size(px(10.))
                    .text_color(theme::muted())
                    .child(service.ports.clone()),
            )
        })
        .into_any_element()
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

pub(super) fn docker_container_matches(container: &DockerContainer, filter: &str) -> bool {
    filter.is_empty()
        || container.name.to_lowercase().contains(filter)
        || container.image.to_lowercase().contains(filter)
        || container.id.to_lowercase().contains(filter)
}

fn short_id(id: &str) -> String {
    let value = id.strip_prefix("sha256:").unwrap_or(id);
    value.chars().take(12).collect()
}
