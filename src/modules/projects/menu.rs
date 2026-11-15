use crate::{
    app::MintoolApp,
    modules::projects::{dialogs::ProjectDialog, model::Project},
    modules::SessionKey,
    services::desktop::Desktop,
    ui::dialogs::Dialog,
    workspace::layout::Axis,
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
                    let id = terminal_project.id.clone();
                    this.select_project(id.clone(), cx);
                    let key = SessionKey::project(&id);
                    this.ensure_session(&key);
                    if let Some(workspace) = this.workspaces.get(&key) {
                        let focused = workspace.state.focused;
                        this.split(focused, Axis::Horizontal, cx);
                    }
                },
            )
            .disabled(!project.path.is_dir());
        let (command_id, command_name) = (project.id.clone(), project.name.clone());
        let command = self.menu_action(
            "project-command",
            "设置启动命令…",
            cx,
            move |this, window, cx| {
                let command = this
                    .modules
                    .projects
                    .registry
                    .projects
                    .iter()
                    .find(|project| project.id == command_id)
                    .and_then(|project| project.run_command.clone());
                this.show_dialog(
                    Dialog::Project(ProjectDialog::SetCommand {
                        id: command_id.clone(),
                        name: command_name.clone(),
                        command,
                    }),
                    window,
                    cx,
                )
            },
        );
        let pin_id = project.id.clone();
        let pinned = project.pinned;
        let pin = self.menu_action(
            "project-pin",
            if pinned { "取消置顶" } else { "置顶" },
            cx,
            move |this, _, cx| {
                if let Some(project) = this
                    .modules
                    .projects
                    .registry
                    .projects
                    .iter_mut()
                    .find(|project| project.id == pin_id)
                {
                    project.pinned = !pinned;
                }
                this.changed(cx);
            },
        );
        let (color_id, color_name) = (project.id.clone(), project.name.clone());
        let color = self.menu_action(
            "project-color",
            "设置颜色…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Project(ProjectDialog::SetColor {
                        id: color_id.clone(),
                        name: color_name.clone(),
                    }),
                    window,
                    cx,
                )
            },
        );
        let archive_id = project.id.clone();
        let archived = project.archived;
        let archive = self.menu_action(
            "project-archive",
            if archived { "取消归档" } else { "归档" },
            cx,
            move |this, _, cx| {
                if let Some(project) = this
                    .modules
                    .projects
                    .registry
                    .projects
                    .iter_mut()
                    .find(|project| project.id == archive_id)
                {
                    project.archived = !archived;
                }
                this.changed(cx);
            },
        );
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
                this.show_dialog(
                    Dialog::Project(ProjectDialog::RenameProject(id.clone(), name.clone())),
                    window,
                    cx,
                )
            },
        );
        let (task_id, task_name) = (project.id.clone(), project.name.clone());
        let add_task = self.menu_action(
            "project-add-task",
            "添加任务…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Project(ProjectDialog::AddTask {
                        id: task_id.clone(),
                        name: task_name.clone(),
                    }),
                    window,
                    cx,
                )
            },
        );
        let mut task_items = Vec::new();
        for (index, task) in project.tasks.iter().take(8).enumerate() {
            let id = project.id.clone();
            let command = task.clone();
            let label = format!("运行任务：{task}");
            task_items.push(self.menu_action(
                format!("project-task-{index}"),
                label,
                cx,
                move |this, _, cx| this.run_project_task(&id, &command, cx),
            ));
        }
        let (id, path) = (project.id.clone(), project.path.display().to_string());
        let locate = self.menu_action(
            "locate",
            "重新定位目录…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Project(ProjectDialog::Locate(id.clone(), path.clone())),
                    window,
                    cx,
                )
            },
        );
        let (id, name) = (project.id.clone(), project.name.clone());
        let move_group = self.menu_action(
            "move-group",
            "移动到分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Project(ProjectDialog::MoveProject(id.clone(), name.clone())),
                    window,
                    cx,
                )
            },
        );
        let peers: Vec<_> = self
            .modules
            .projects
            .registry
            .projects
            .iter()
            .filter(|p| p.group == project.group)
            .collect();
        let index = peers.iter().position(|p| p.id == project.id).unwrap_or(0);
        let id = project.id.clone();
        let up = self
            .menu_action("project-up", "上移", cx, move |this, _, cx| {
                this.modules.projects.registry.shift_project(&id, -1);
                this.changed(cx);
            })
            .disabled(index == 0);
        let id = project.id.clone();
        let down = self
            .menu_action("project-down", "下移", cx, move |this, _, cx| {
                this.modules.projects.registry.shift_project(&id, 1);
                this.changed(cx);
            })
            .disabled(index + 1 >= peers.len());
        let remove = self.menu_action(
            "remove",
            "移除项目记录…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Project(ProjectDialog::Remove(
                        project.id.clone(),
                        project.name.clone(),
                    )),
                    window,
                    cx,
                )
            },
        );
        let mut items = vec![
            open,
            terminal,
            command,
            add_task,
            pin,
            color,
            archive,
            browse,
            copy,
            MenuItem::separator(),
        ];
        items.extend(task_items);
        items.push(MenuItem::separator());
        items.extend([
            rename,
            locate,
            move_group,
            up,
            down,
            MenuItem::separator(),
            remove,
        ]);
        self.menu = Some((position, items));
        cx.notify();
    }
}
