use super::dialogs::DatabaseDialog;
use super::model::DatabaseConnection;
use crate::app::MintoolApp;
use crate::ui::dialogs::Dialog;
use crate::workspace::layout::Axis;
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;

impl MintoolApp {
    pub(crate) fn database_connection_menu(
        &mut self,
        connection: DatabaseConnection,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let open_id = connection.id.clone();
        let open = self.menu_action(
            "database-open",
            "打开查询工作区",
            cx,
            move |this, _, cx| this.open_database_connection(open_id.clone(), cx),
        );
        let new_query_id = connection.id.clone();
        let new_query = self.menu_action(
            "database-new-query",
            "新建查询",
            cx,
            move |this, _, cx| {
                this.open_database_connection(new_query_id.clone(), cx);
                let focused = this
                    .workspaces
                    .get(&crate::modules::SessionKey::database(new_query_id.clone()))
                    .map(|workspace| workspace.state.focused);
                if let Some(focused) = focused {
                    this.split(focused, Axis::Horizontal, cx);
                }
            },
        );
        let edit_id = connection.id.clone();
        let edit = self.menu_action(
            "database-edit",
            "编辑连接…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Database(DatabaseDialog::Connection {
                        edit: Some(edit_id.clone()),
                        group: None,
                    }),
                    window,
                    cx,
                );
            },
        );
        let mut items = vec![open, new_query, edit, MenuItem::separator()];
        let ungroup_id = connection.id.clone();
        items.push(self.menu_action(
            "database-move-ungrouped",
            "移至未分组",
            cx,
            move |this, _, cx| {
                this.modules
                    .database
                    .registry
                    .move_connection(&ungroup_id, None, None);
                this.changed(cx);
            },
        ));
        for group in self.modules.database.registry.groups.clone() {
            let id = connection.id.clone();
            items.push(self.menu_action(
                SharedString::from(format!("database-move-{}", group.id)),
                format!("移至分组：{}", group.name),
                cx,
                move |this, _, cx| {
                    this.modules.database.registry.move_connection(
                        &id,
                        Some(group.id.clone()),
                        None,
                    );
                    this.changed(cx);
                },
            ));
        }
        items.push(MenuItem::separator());
        let peers: Vec<_> = self
            .modules
            .database
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
        items.push(
            self.menu_action("database-up", "上移", cx, move |this, _, cx| {
                this.modules.database.registry.shift_connection(&up_id, -1);
                this.changed(cx);
            })
            .disabled(index == 0),
        );
        let down_id = connection.id.clone();
        items.push(
            self.menu_action("database-down", "下移", cx, move |this, _, cx| {
                this.modules.database.registry.shift_connection(&down_id, 1);
                this.changed(cx);
            })
            .disabled(index + 1 >= peers.len()),
        );
        items.push(MenuItem::separator());
        let remove_id = connection.id.clone();
        let remove_name = connection.name.clone();
        items.push(self.menu_action(
            "database-remove",
            "移除连接…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Database(DatabaseDialog::Remove {
                        id: remove_id.clone(),
                        name: remove_name.clone(),
                    }),
                    window,
                    cx,
                );
            },
        ));
        self.menu = Some((position, items));
        cx.notify();
    }

    pub(crate) fn database_group_menu(
        &mut self,
        id: String,
        name: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let add_group = id.clone();
        let add = self.menu_action(
            "database-group-add",
            "新建数据库到此分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Database(DatabaseDialog::Connection {
                        edit: None,
                        group: Some(add_group.clone()),
                    }),
                    window,
                    cx,
                );
            },
        );
        let collapsed = self
            .modules
            .database
            .registry
            .groups
            .iter()
            .any(|group| group.id == id && group.collapsed);
        let toggle_id = id.clone();
        let toggle = self.menu_action(
            "database-group-toggle",
            if collapsed {
                "展开分组"
            } else {
                "折叠分组"
            },
            cx,
            move |this, _, cx| {
                if let Some(group) = this
                    .modules
                    .database
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
            "database-group-rename",
            "重命名分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Database(DatabaseDialog::GroupRename {
                        id: rename_id.clone(),
                        name: rename_name.clone(),
                    }),
                    window,
                    cx,
                );
            },
        );
        let index = self
            .modules
            .database
            .registry
            .groups
            .iter()
            .position(|group| group.id == id)
            .unwrap_or(0);
        let up_id = id.clone();
        let up = self
            .menu_action("database-group-up", "上移", cx, move |this, _, cx| {
                this.modules.database.registry.shift_group(&up_id, -1);
                this.changed(cx);
            })
            .disabled(index == 0);
        let down_id = id.clone();
        let down = self
            .menu_action("database-group-down", "下移", cx, move |this, _, cx| {
                this.modules.database.registry.shift_group(&down_id, 1);
                this.changed(cx);
            })
            .disabled(index + 1 >= self.modules.database.registry.groups.len());
        let remove = self.menu_action(
            "database-group-remove",
            "删除分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::Database(DatabaseDialog::GroupRemove {
                        id: id.clone(),
                        name: name.clone(),
                    }),
                    window,
                    cx,
                );
            },
        );
        self.menu = Some((
            position,
            vec![add, toggle, rename, up, down, MenuItem::separator(), remove],
        ));
        cx.notify();
    }

    pub(crate) fn database_navigation_menu(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let add = self.menu_action(
            "database-navigation-add",
            "添加或新建 SQLite…",
            cx,
            |this, window, cx| {
                this.show_dialog(
                    Dialog::Database(DatabaseDialog::Connection {
                        edit: None,
                        group: None,
                    }),
                    window,
                    cx,
                );
            },
        );
        let group = self.menu_action(
            "database-navigation-group",
            "新建分组…",
            cx,
            |this, window, cx| {
                this.show_dialog(Dialog::Database(DatabaseDialog::GroupAdd), window, cx);
            },
        );
        self.menu = Some((position, vec![add, group]));
        cx.notify();
    }
}
