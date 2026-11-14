use crate::{
    app::MintoolApp,
    model::project::Project,
    ui::{
        dialogs::Dialog,
        drag_item::{DragItem, DragKind},
        theme,
    },
};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
    input::{Input, InputSize},
};
use gpui::{prelude::FluentBuilder, *};
impl MintoolApp {
    pub(crate) fn render_navigation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filter = self.search.read(cx).content().to_lowercase();
        let (left_width, _) = self
            .settings
            .panel_widths(window.viewport_size().width.into());
        let mut rows = Vec::new();
        rows.push(
            div()
                .id("ungrouped")
                .px_4()
                .pt_4()
                .pb_2()
                .text_size(px(11.))
                .text_color(theme::muted())
                .child("未分组")
                .on_drop(cx.listener(|this, item: &DragItem, _, cx| {
                    if let DragKind::Project(id) = &item.kind {
                        this.projects.move_project(id, None, None);
                        this.changed(cx);
                    }
                }))
                .into_any_element(),
        );
        for project in self
            .projects
            .projects
            .iter()
            .filter(|p| p.group.is_none() && p.name.to_lowercase().contains(&filter))
        {
            rows.push(self.project_row(project, cx));
        }
        for group in &self.projects.groups {
            let id = group.id.clone();
            let toggle = id.clone();
            let drop_id = id.clone();
            let menu_id = id.clone();
            let menu_name = group.name.clone();
            let payload = DragItem {
                kind: DragKind::Group(id.clone()),
                name: group.name.clone(),
            };
            rows.push(
                div()
                    .id(SharedString::from(format!("group-{id}")))
                    .mx_3()
                    .mt_4()
                    .mb_1()
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::elevated()))
                    .child(
                        Icon::new(if group.collapsed {
                            "chevron-right"
                        } else {
                            "chevron-down"
                        })
                        .size(px(12.))
                        .color(theme::muted()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::muted())
                            .child(group.name.clone()),
                    )
                    .child(
                        div().text_size(px(10.)).text_color(theme::muted()).child(
                            self.projects
                                .projects
                                .iter()
                                .filter(|p| p.group.as_ref() == Some(&id))
                                .count()
                                .to_string(),
                        ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(g) = this.projects.groups.iter_mut().find(|g| g.id == toggle) {
                            g.collapsed = !g.collapsed;
                        }
                        this.changed(cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.group_menu(menu_id.clone(), menu_name.clone(), event.position, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_drag(payload, |item, _, _, cx| cx.new(|_| item.clone()))
                    .on_drop(cx.listener(move |this, item: &DragItem, _, cx| {
                        match &item.kind {
                            DragKind::Project(id) => {
                                this.projects.move_project(id, Some(drop_id.clone()), None)
                            }
                            DragKind::Group(id) => this.projects.move_group(id, &drop_id),
                        }
                        this.changed(cx);
                    }))
                    .into_any_element(),
            );
            if !group.collapsed || !filter.is_empty() {
                for project in self.projects.projects.iter().filter(|p| {
                    p.group.as_ref() == Some(&id) && p.name.to_lowercase().contains(&filter)
                }) {
                    rows.push(self.project_row(project, cx));
                }
            }
        }
        div()
            .id("navigation")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::panel())
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.navigation_menu(event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .px_5()
                    .pt_5()
                    .pb_3()
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
                                    .child("项目空间"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(theme::muted())
                                    .child(format!("{} 个本地项目", self.projects.projects.len())),
                            ),
                    )
                    .child(
                        Button::new("add-group", "")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .icon("folder-plus")
                            .tooltip("新建分组")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_dialog(Dialog::AddGroup, window, cx)
                            })),
                    ),
            )
            .child(
                div().w_full().flex().flex_col().px_4().pb_2().child(
                    Input::new(&self.search)
                        .map(|mut input| {
                            input.style().align_items = Some(AlignItems::Stretch);
                            input
                        })
                        .w(px(left_width - 32.))
                        .min_w(px(left_width - 32.))
                        .size(InputSize::Sm)
                        .placeholder("搜索项目…")
                        .prefix(Icon::new("search").size(px(13.)).color(theme::muted())),
                ),
            )
            .child(
                div()
                    .id("project-list")
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
                    .gap_3()
                    .child(
                        Button::new("add-project", "添加项目")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .icon("plus")
                            .w_full()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_dialog(Dialog::AddProject(None), window, cx)
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme::muted())
                            .text_center()
                            .child("拖动排序 / 归组 · 右键管理"),
                    ),
            )
            .into_any_element()
    }
    fn project_row(&self, project: &Project, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.projects.selected.as_deref() == Some(&project.id);
        let missing = !project.path.is_dir();
        let id = project.id.clone();
        let before = id.clone();
        let group = project.group.clone();
        let menu_project = project.clone();
        let payload = DragItem {
            kind: DragKind::Project(id.clone()),
            name: project.name.clone(),
        };
        div()
            .id(SharedString::from(format!("project-{id}")))
            .mx_3()
            .my(px(3.))
            .h(px(39.))
            .px_3()
            .rounded_lg()
            .border_1()
            .border_color(if selected {
                theme::selected_border()
            } else {
                theme::panel()
            })
            .flex()
            .items_center()
            .gap_3()
            .cursor_pointer()
            .bg(if selected {
                theme::selected()
            } else {
                theme::panel()
            })
            .hover(move |s| {
                s.bg(if selected {
                    theme::selected_hover()
                } else {
                    theme::elevated()
                })
            })
            .child(
                Icon::new(if missing { "folder-x" } else { "folder-closed" })
                    .size(px(15.))
                    .color(if selected {
                        theme::accent()
                    } else {
                        theme::muted()
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.))
                    .truncate()
                    .text_color(if missing {
                        theme::danger()
                    } else if selected {
                        theme::accent()
                    } else {
                        theme::text()
                    })
                    .child(project.name.clone()),
            )
            .when(selected, |row| {
                row.child(div().size(px(5.)).rounded_full().bg(theme::accent()))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.select_project(id.clone(), cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.project_menu(menu_project.clone(), event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .on_drag(payload, |item, _, _, cx| cx.new(|_| item.clone()))
            .on_drop(cx.listener(move |this, item: &DragItem, _, cx| {
                if let DragKind::Project(id) = &item.kind {
                    this.projects.move_project(id, group.clone(), Some(&before));
                    this.changed(cx);
                }
            }))
            .into_any_element()
    }
}
