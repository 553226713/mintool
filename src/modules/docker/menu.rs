use super::model::{DockerContainer, DockerImage, DockerView};
use crate::app::MintoolApp;
use crate::modules::docker::dialogs::DockerDialog;
use crate::ui::dialogs::Dialog;
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;

impl MintoolApp {
    pub(crate) fn docker_container_menu(
        &mut self,
        container: DockerContainer,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let logs_id = container.id.clone();
        let logs = self.menu_action("docker-logs", "查看日志", cx, move |this, _, cx| {
            this.open_docker_session(logs_id.clone(), DockerView::Logs, cx)
        });
        let exec_id = container.id.clone();
        let exec = self.menu_action("docker-exec", "进入终端", cx, move |this, _, cx| {
            this.open_docker_session(exec_id.clone(), DockerView::Exec, cx)
        });
        let details_id = container.id.clone();
        let details = self.menu_action(
            "docker-details",
            "查看详情",
            cx,
            move |this, window, cx| this.show_container_details(details_id.clone(), window, cx),
        );
        let mut items = vec![logs, exec, details, MenuItem::separator()];
        if container.paused() {
            let id = container.id.clone();
            items.push(self.menu_action(
                "docker-unpause",
                "恢复运行",
                cx,
                move |this, _, cx| this.docker_action(id.clone(), "unpause", cx),
            ));
        } else if container.running() {
            let id = container.id.clone();
            items.push(
                self.menu_action("docker-stop", "停止", cx, move |this, _, cx| {
                    this.docker_action(id.clone(), "stop", cx)
                }),
            );
            let id = container.id.clone();
            items.push(
                self.menu_action("docker-restart", "重启", cx, move |this, _, cx| {
                    this.docker_action(id.clone(), "restart", cx)
                }),
            );
            let id = container.id.clone();
            items.push(
                self.menu_action("docker-pause", "暂停", cx, move |this, _, cx| {
                    this.docker_action(id.clone(), "pause", cx)
                }),
            );
        } else {
            let id = container.id.clone();
            items.push(
                self.menu_action("docker-start", "启动", cx, move |this, _, cx| {
                    this.docker_action(id.clone(), "start", cx)
                }),
            );
        }
        items.push(MenuItem::separator());
        let copy_id = container.id.clone();
        items.push(
            self.menu_action("docker-copy-id", "复制容器 ID", cx, move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_id.clone()));
            }),
        );
        let copy_name = container.name.clone();
        items.push(self.menu_action(
            "docker-copy-name",
            "复制容器名称",
            cx,
            move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_name.clone()));
            },
        ));
        items.push(MenuItem::separator());
        items.push(self.menu_action(
            "docker-remove",
            "删除容器…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Docker(DockerDialog::Container(
                        container.id.clone(),
                        container.name.clone(),
                    )),
                    window,
                    cx,
                )
            },
        ));
        self.menu = Some((position, items));
        cx.notify();
    }

    pub(crate) fn docker_image_menu(
        &mut self,
        image: DockerImage,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let run_reference = image.reference();
        let run = self.menu_action(
            "docker-image-run",
            "运行容器…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Docker(DockerDialog::RunImage {
                        reference: run_reference.clone(),
                    }),
                    window,
                    cx,
                )
            },
        );
        let tag_id = image.id.clone();
        let tag_reference = image.reference();
        let tag = self.menu_action(
            "docker-image-tag",
            "标记镜像…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Docker(DockerDialog::TagImage {
                        id: tag_id.clone(),
                        reference: tag_reference.clone(),
                    }),
                    window,
                    cx,
                )
            },
        );
        let pull = self.menu_action(
            "docker-image-pull",
            "拉取镜像…",
            cx,
            |this, window, cx| {
                this.show_dialog(Dialog::Docker(DockerDialog::PullImage), window, cx)
            },
        );
        let reference = image.reference();
        let copy_name = self.menu_action(
            "docker-image-copy",
            "复制镜像名称",
            cx,
            move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(reference.clone()));
            },
        );
        let id = image.id.clone();
        let copy_id = self.menu_action(
            "docker-image-copy-id",
            "复制镜像 ID",
            cx,
            move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(id.clone()));
            },
        );
        let remove = self.menu_action(
            "docker-image-remove",
            "删除镜像…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Docker(DockerDialog::Image(image.id.clone(), image.reference())),
                    window,
                    cx,
                )
            },
        );
        self.menu = Some((
            position,
            vec![
                run,
                tag,
                MenuItem::separator(),
                pull,
                MenuItem::separator(),
                copy_name,
                copy_id,
                MenuItem::separator(),
                remove,
            ],
        ));
        cx.notify();
    }
}
