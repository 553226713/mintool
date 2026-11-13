use crate::{app::MintoolApp, ui::dialogs::Dialog};
use adabraka_ui::navigation::menu::MenuItem;
use gpui::*;

impl MintoolApp {
    pub(crate) fn group_menu(
        &mut self,
        id: String,
        name: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let group = id.clone();
        let add = self.menu_action(
            "group-add-project",
            "添加项目到此分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(Dialog::AddProject(Some(group.clone())), window, cx)
            },
        );
        let group = id.clone();
        let collapsed = self
            .projects
            .groups
            .iter()
            .any(|g| g.id == id && g.collapsed);
        let toggle = self.menu_action(
            "toggle-group",
            if collapsed {
                "展开分组"
            } else {
                "折叠分组"
            },
            cx,
            move |this, _, cx| {
                if let Some(group) = this.projects.groups.iter_mut().find(|g| g.id == group) {
                    group.collapsed = !group.collapsed;
                }
                this.changed(cx);
            },
        );
        let (group, label) = (id.clone(), name.clone());
        let rename = self.menu_action(
            "rename-group",
            "重命名分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(
                    Dialog::RenameGroup(group.clone(), label.clone()),
                    window,
                    cx,
                )
            },
        );
        let index = self
            .projects
            .groups
            .iter()
            .position(|g| g.id == id)
            .unwrap_or(0);
        let group = id.clone();
        let up = self
            .menu_action("group-up", "上移", cx, move |this, _, cx| {
                this.projects.shift_group(&group, -1);
                this.changed(cx);
            })
            .disabled(index == 0);
        let group = id.clone();
        let down = self
            .menu_action("group-down", "下移", cx, move |this, _, cx| {
                this.projects.shift_group(&group, 1);
                this.changed(cx);
            })
            .disabled(index + 1 >= self.projects.groups.len());
        let remove = self.menu_action(
            "remove-group",
            "删除分组…",
            cx,
            move |this, window, cx| {
                this.show_dialog(Dialog::RemoveGroup(id.clone(), name.clone()), window, cx)
            },
        );
        self.menu = Some((
            position,
            vec![add, toggle, rename, up, down, MenuItem::separator(), remove],
        ));
        cx.notify();
    }
    pub(crate) fn navigation_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let add = self.menu_action("add-project", "添加项目…", cx, |this, window, cx| {
            this.show_dialog(Dialog::AddProject(None), window, cx)
        });
        let group = self.menu_action("add-group", "新建分组…", cx, |this, window, cx| {
            this.show_dialog(Dialog::AddGroup, window, cx)
        });
        let expand = self.menu_action("expand-groups", "展开所有分组", cx, |this, _, cx| {
            this.projects
                .groups
                .iter_mut()
                .for_each(|g| g.collapsed = false);
            this.changed(cx);
        });
        let collapse = self.menu_action(
            "collapse-groups",
            "折叠所有分组",
            cx,
            |this, _, cx| {
                this.projects
                    .groups
                    .iter_mut()
                    .for_each(|g| g.collapsed = true);
                this.changed(cx);
            },
        );
        self.menu = Some((
            position,
            vec![add, group, MenuItem::separator(), expand, collapse],
        ));
        cx.notify();
    }
}
