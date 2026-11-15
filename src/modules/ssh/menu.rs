use crate::app::MintoolApp;
use crate::modules::ssh::dialogs::SshDialog;
use crate::modules::ssh::model::SshConnection;
use crate::modules::SessionKey;
use crate::ui::dialogs::Dialog;
use crate::workspace::layout::Axis;
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;

impl MintoolApp {
    pub(crate) fn ssh_connection_menu(
        &mut self,
        connection: SshConnection,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let id = connection.id.clone();
        let connect = self.menu_action("ssh-connect", "连接", cx, move |this, _, cx| {
            this.open_ssh_connection(id.clone(), cx)
        });
        let id = connection.id.clone();
        let terminal = self.menu_action(
            "ssh-new-terminal",
            "新建终端",
            cx,
            move |this, _, cx| {
                this.open_ssh_connection(id.clone(), cx);
                let key = SessionKey::ssh(id.clone());
                if let Some(workspace) = this.workspaces.get(&key) {
                    let focused = workspace.state.focused;
                    this.split(focused, Axis::Horizontal, cx);
                }
            },
        );
        let edit_id = connection.id.clone();
        let edit = self.menu_action(
            "ssh-edit",
            "编辑连接…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::Connection {
                        edit: Some(edit_id.clone()),
                        group: None,
                    }),
                    window,
                    cx,
                )
            },
        );
        let duplicate_connection = connection.clone();
        let duplicate =
            self.menu_action("ssh-duplicate", "复制连接", cx, move |this, _, cx| {
                let mut copy = duplicate_connection.clone();
                copy.id = uuid::Uuid::new_v4().to_string();
                copy.name = format!("{} 副本", duplicate_connection.name);
                this.modules.ssh.registry.add(copy);
                this.changed(cx);
            });
        let move_id = connection.id.clone();
        let move_name = connection.name.clone();
        let moving = self.menu_action(
            "ssh-move",
            "移动到分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::Move(move_id.clone(), move_name.clone())),
                    window,
                    cx,
                )
            },
        );
        let peers: Vec<_> = self
            .modules
            .ssh
            .registry
            .connections
            .iter()
            .filter(|candidate| candidate.group == connection.group)
            .collect();
        let index = peers
            .iter()
            .position(|candidate| candidate.id == connection.id)
            .unwrap_or(0);
        let up_id = connection.id.clone();
        let up = self
            .menu_action("ssh-up", "上移", cx, move |this, _, cx| {
                this.modules.ssh.registry.shift_connection(&up_id, -1);
                this.changed(cx);
            })
            .disabled(index == 0);
        let down_id = connection.id.clone();
        let down = self
            .menu_action("ssh-down", "下移", cx, move |this, _, cx| {
                this.modules.ssh.registry.shift_connection(&down_id, 1);
                this.changed(cx);
            })
            .disabled(index + 1 >= peers.len());
        let remove = self.menu_action(
            "ssh-remove",
            "删除连接…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::Remove(
                        connection.id.clone(),
                        connection.name.clone(),
                    )),
                    window,
                    cx,
                )
            },
        );
        self.menu = Some((
            position,
            vec![
                connect,
                terminal,
                MenuItem::separator(),
                edit,
                duplicate,
                moving,
                MenuItem::separator(),
                up,
                down,
                MenuItem::separator(),
                remove,
            ],
        ));
        cx.notify();
    }

    pub(crate) fn ssh_group_menu(
        &mut self,
        id: String,
        name: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let add_group = id.clone();
        let add = self.menu_action(
            "ssh-group-add",
            "新建连接到此分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::Connection {
                        edit: None,
                        group: Some(add_group.clone()),
                    }),
                    window,
                    cx,
                )
            },
        );
        let collapsed = self
            .modules
            .ssh
            .registry
            .groups
            .iter()
            .any(|group| group.id == id && group.collapsed);
        let toggle_id = id.clone();
        let toggle = self.menu_action(
            "ssh-group-toggle",
            if collapsed {
                "展开分组"
            } else {
                "折叠分组"
            },
            cx,
            move |this, _, cx| {
                if let Some(group) = this
                    .modules
                    .ssh
                    .registry
                    .groups
                    .iter_mut()
                    .find(|group| group.id == toggle_id)
                {
                    group.collapsed = !group.collapsed;
                }
                this.changed(cx);
            },
        );
        let rename_id = id.clone();
        let rename_name = name.clone();
        let rename = self.menu_action(
            "ssh-group-rename",
            "重命名分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::GroupRename(
                        rename_id.clone(),
                        rename_name.clone(),
                    )),
                    window,
                    cx,
                )
            },
        );
        let index = self
            .modules
            .ssh
            .registry
            .groups
            .iter()
            .position(|group| group.id == id)
            .unwrap_or(0);
        let up_id = id.clone();
        let up = self
            .menu_action("ssh-group-up", "上移", cx, move |this, _, cx| {
                this.modules.ssh.registry.shift_group(&up_id, -1);
                this.changed(cx);
            })
            .disabled(index == 0);
        let down_id = id.clone();
        let down = self
            .menu_action("ssh-group-down", "下移", cx, move |this, _, cx| {
                this.modules.ssh.registry.shift_group(&down_id, 1);
                this.changed(cx);
            })
            .disabled(index + 1 >= self.modules.ssh.registry.groups.len());
        let remove = self.menu_action(
            "ssh-group-remove",
            "删除分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::GroupRemove(id.clone(), name.clone())),
                    window,
                    cx,
                )
            },
        );
        self.menu = Some((
            position,
            vec![add, toggle, rename, up, down, MenuItem::separator(), remove],
        ));
        cx.notify();
    }

    pub(crate) fn import_ssh_config(&mut self, cx: &mut Context<Self>) {
        let path = super::session::home_dir().join(".ssh").join("config");
        match super::import::import_config(&mut self.modules.ssh.registry, &path) {
            Ok(count) => {
                self.error = Some(if count == 0 {
                    "没有新的 SSH 连接可导入".to_string()
                } else {
                    format!("已从 {} 导入 {count} 个连接", path.display())
                });
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
        self.changed(cx);
    }

    pub(crate) fn ssh_navigation_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let import = self.menu_action(
            "ssh-import",
            "从 ~/.ssh/config 导入",
            cx,
            |this, _, cx| this.import_ssh_config(cx),
        );
        let connection = self.menu_action(
            "ssh-navigation-new",
            "新建 SSH 连接…",
            cx,
            |this, window, cx| {
                this.show_dialog(
                    Dialog::Ssh(SshDialog::Connection {
                        edit: None,
                        group: None,
                    }),
                    window,
                    cx,
                )
            },
        );
        let group = self.menu_action(
            "ssh-navigation-group",
            "新建分组…",
            cx,
            |this, window, cx| this.show_dialog(Dialog::Ssh(SshDialog::GroupAdd), window, cx),
        );
        self.menu = Some((
            position,
            vec![connection, group, MenuItem::separator(), import],
        ));
        cx.notify();
    }
}
