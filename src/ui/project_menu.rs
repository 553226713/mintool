use crate::{
    app::MintoolApp,
    model::{layout::Axis, project::Project},
    services::{desktop::Desktop, workspace::Workspace},
    ui::dialogs::Dialog,
};
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;

impl MintoolApp {
    pub(crate) fn project_menu(
        &mut self,
        project: Project,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let id = project.id.clone();
        let open = self.menu_action("open-project", "打开项目", cx, move |this, _, cx| {
            this.select_project(id.clone(), cx)
        });
        let terminal_project = project.clone();
        let terminal = self
            .menu_action(
                "project-terminal",
                "新建终端",
                cx,
                move |this, _, cx| {
                    let id = &terminal_project.id;
                    if !this.workspaces.contains_key(id) {
                        match this.storage.workspace(id) {
                            Ok(state) => {
                                this.workspaces.insert(id.clone(), Workspace::new(state));
                            }
                            Err(error) => {
                                this.error = Some(format!("{error:#}"));
                                return;
                            }
                        }
                    }
                    this.select_project(id.clone(), cx);
                    let focused = this.workspaces[id].state.focused;
                    this.split(focused, Axis::Horizontal, cx);
                },
            )
            .disabled(!project.path.is_dir());
        let path = project.path.clone();
        let browse = self
            .menu_action(
                "open-folder",
                "在文件管理器中打开",
                cx,
                move |this, _, cx| {
                    if let Err(error) = Desktop::open_directory(&path) {
                        this.error = Some(format!("{error:#}"));
                        cx.notify();
                    }
                },
            )
            .disabled(!project.path.is_dir());
        let path = project.path.display().to_string();
        let copy = self.menu_action("copy-path", "复制项目路径", cx, move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(path.clone()))
        });
        let (id, name) = (project.id.clone(), project.name.clone());
        let rename = self.menu_action(
            "rename",
            "重命名项目…",
            cx,
            move |this, window, cx| {
                this.show_dialog(Dialog::RenameProject(id.clone(), name.clone()), window, cx)
            },
        );
        let (id, path) = (project.id.clone(), project.path.display().to_string());
        let locate = self.menu_action(
            "locate",
            "重新定位目录…",
            cx,
            move |this, window, cx| {
                this.show_dialog(Dialog::Locate(id.clone(), path.clone()), window, cx)
            },
        );
        let (id, name) = (project.id.clone(), project.name.clone());
        let move_group = self.menu_action(
            "move-group",
            "移动到分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(Dialog::MoveProject(id.clone(), name.clone()), window, cx)
            },
        );
        let peers: Vec<_> = self
            .projects
            .projects
            .iter()
            .filter(|p| p.group == project.group)
            .collect();
        let index = peers.iter().position(|p| p.id == project.id).unwrap_or(0);
        let id = project.id.clone();
        let up = self
            .menu_action("project-up", "上移", cx, move |this, _, cx| {
                this.projects.shift_project(&id, -1);
                this.changed(cx);
            })
            .disabled(index == 0);
        let id = project.id.clone();
        let down = self
            .menu_action("project-down", "下移", cx, move |this, _, cx| {
                this.projects.shift_project(&id, 1);
                this.changed(cx);
            })
            .disabled(index + 1 >= peers.len());
        let remove = self.menu_action(
            "remove",
            "移除项目记录…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Remove(project.id.clone(), project.name.clone()),
                    window,
                    cx,
                )
            },
        );
        self.menu = Some((
            position,
            vec![
                open,
                terminal,
                browse,
                copy,
                MenuItem::separator(),
                rename,
                locate,
                move_group,
                up,
                down,
                MenuItem::separator(),
                remove,
            ],
        ));
        cx.notify();
    }
}
