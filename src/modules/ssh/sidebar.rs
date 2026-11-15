use crate::app::MintoolApp;
use crate::modules::ssh::dialogs::SshDialog;
use crate::modules::ssh::model::SshConnection;
use crate::ui::{
    dialogs::Dialog,
    drag_item::{DragItem, DragKind},
    theme,
};
use adabraka_ui::components::{
    button::{Button, ButtonSize, ButtonVariant},
    icon::Icon,
    input::{Input, InputSize},
};
use gpui::prelude::FluentBuilder;
use gpui::*;

impl MintoolApp {
    pub(crate) fn render_ssh_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filter = self.modules.ssh.search.read(cx).content().to_lowercase();
        let (left_width, _) = self
            .settings
            .panel_widths(window.viewport_size().width.into());
        let mut rows = Vec::new();
        rows.push(
            div()
                .px_4()
                .pt_4()
                .pb_2()
                .text_size(px(11.))
                .text_color(theme::muted())
                .child("未分组")
                .on_drop(cx.listener(|this, item: &DragItem, _, cx| {
                    if let DragKind::SshConnection(id) = &item.kind {
                        this.modules.ssh.registry.move_connection(id, None, None);
                        this.changed(cx);
                    }
                }))
                .into_any_element(),
        );
        for connection in self
            .modules
            .ssh
            .registry
            .connections
            .iter()
            .filter(|connection| {
                connection.group.is_none()
                    && (connection.name.to_lowercase().contains(&filter)
                        || connection.host.to_lowercase().contains(&filter))
            })
        {
            rows.push(self.ssh_connection_row(connection, cx));
        }
        let groups = self.modules.ssh.registry.groups.clone();
        for group in &groups {
            let group_id = group.id.clone();
            let toggle = group_id.clone();
            let menu_id = group_id.clone();
            let menu_name = group.name.clone();
            let payload = DragItem {
                kind: DragKind::SshGroup(group_id.clone()),
                name: group.name.clone(),
            };
            let drop_id = group_id.clone();
            let drop_group = group_id.clone();
            let count = self
                .modules
                .ssh
                .registry
                .connections
                .iter()
                .filter(|connection| connection.group.as_ref() == Some(&group_id))
                .count();
            rows.push(
                crate::modules::common::sidebar::group_header(
                    SharedString::from(format!("ssh-group-{group_id}")),
                    group.name.clone(),
                    count,
                    group.collapsed,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(group) = this
                        .modules
                        .ssh
                        .registry
                        .groups
                        .iter_mut()
                        .find(|group| group.id == toggle)
                    {
                        group.collapsed = !group.collapsed;
                    }
                    this.changed(cx);
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        this.ssh_group_menu(menu_id.clone(), menu_name.clone(), event.position, cx);
                        cx.stop_propagation();
                    }),
                )
                .on_drag(payload, |item, _, _, cx| cx.new(|_| item.clone()))
                .on_drop(cx.listener(move |this, item: &DragItem, _, cx| {
                    match &item.kind {
                        DragKind::SshConnection(id) => this.modules.ssh.registry.move_connection(
                            id,
                            Some(drop_group.clone()),
                            None,
                        ),
                        DragKind::SshGroup(id) => {
                            this.modules.ssh.registry.move_group(id, &drop_id)
                        }
                        _ => (),
                    }
                    this.changed(cx);
                }))
                .into_any_element(),
            );
            if !group.collapsed || !filter.is_empty() {
                for connection in
                    self.modules
                        .ssh
                        .registry
                        .connections
                        .iter()
                        .filter(|connection| {
                            connection.group.as_ref() == Some(&group_id)
                                && (connection.name.to_lowercase().contains(&filter)
                                    || connection.host.to_lowercase().contains(&filter))
                        })
                {
                    rows.push(self.ssh_connection_row(connection, cx));
                }
            }
        }
        div()
            .id("ssh-navigation")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::panel())
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.ssh_navigation_menu(event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .px_5()
                    .pt_3()
                    .pb_2()
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
                                    .child("SSH 连接"),
                            )
                            .child(div().text_size(px(10.)).text_color(theme::muted()).child(
                                format!("{} 个连接", self.modules.ssh.registry.connections.len()),
                            )),
                    )
                    .child(
                        Button::new("import-ssh-config", "")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .icon("file-config")
                            .tooltip("从 ~/.ssh/config 导入")
                            .on_click(cx.listener(|this, _, _, cx| this.import_ssh_config(cx))),
                    )
                    .child(
                        Button::new("add-ssh-group", "")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .icon("folder-plus")
                            .tooltip("新建分组")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_dialog(Dialog::Ssh(SshDialog::GroupAdd), window, cx)
                            })),
                    ),
            )
            .child(
                div().w_full().flex().flex_col().px_4().pb_2().child(
                    Input::new(&self.modules.ssh.search)
                        .map(|mut input| {
                            input.style().align_items = Some(AlignItems::Stretch);
                            input
                        })
                        .w(px(left_width - 32.))
                        .min_w(px(left_width - 32.))
                        .size(InputSize::Sm)
                        .placeholder("搜索连接…")
                        .prefix(Icon::new("search").size(px(13.)).color(theme::muted())),
                ),
            )
            .child(
                div()
                    .id("ssh-list")
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
                        Button::new("add-ssh-connection", "新建 SSH 连接")
                            .variant(ButtonVariant::Outline)
                            .size(ButtonSize::Sm)
                            .icon("plus")
                            .w_full()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_dialog(
                                    Dialog::Ssh(SshDialog::Connection {
                                        edit: None,
                                        group: None,
                                    }),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme::muted())
                            .text_center()
                            .child("右键管理 · 同一连接可开多个会话"),
                    ),
            )
            .into_any_element()
    }

    fn ssh_connection_row(&self, connection: &SshConnection, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.modules.ssh.registry.selected.as_deref() == Some(&connection.id);
        let opened = self.modules.ssh.opened.as_deref() == Some(&connection.id);
        let icon_color = connection
            .color
            .as_deref()
            .map(ssh_color)
            .unwrap_or_else(|| {
                if selected {
                    theme::accent()
                } else {
                    theme::muted()
                }
            });
        let id = connection.id.clone();
        let menu_connection = connection.clone();
        let payload = DragItem {
            kind: DragKind::SshConnection(id.clone()),
            name: connection.name.clone(),
        };
        let drop_group = connection.group.clone();
        let drop_before = connection.id.clone();
        div()
            .id(SharedString::from(format!("ssh-connection-{id}")))
            .mx_3()
            .my(px(3.))
            .h(px(44.))
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
            .hover(move |style| {
                style.bg(if selected {
                    theme::selected_hover()
                } else {
                    theme::elevated()
                })
            })
            .child(Icon::new("square-terminal").size(px(15.)).color(icon_color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .truncate()
                            .text_color(if selected {
                                theme::accent()
                            } else {
                                theme::text()
                            })
                            .child(connection.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .truncate()
                            .text_color(theme::muted())
                            .child(connection.display_address()),
                    ),
            )
            .when(opened, |row| {
                row.child(div().size(px(5.)).rounded_full().bg(theme::success()))
            })
            .when(selected && !opened, |row| {
                row.child(div().size(px(5.)).rounded_full().bg(theme::accent()))
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.click_count() >= 2 {
                    this.open_ssh_connection(id.clone(), cx);
                } else {
                    this.select_ssh_connection(id.clone(), cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.ssh_connection_menu(menu_connection.clone(), event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .on_drag(payload, |item, _, _, cx| cx.new(|_| item.clone()))
            .on_drop(cx.listener(move |this, item: &DragItem, _, cx| {
                if let DragKind::SshConnection(id) = &item.kind {
                    this.modules.ssh.registry.move_connection(
                        id,
                        drop_group.clone(),
                        Some(&drop_before),
                    );
                    this.changed(cx);
                }
            }))
            .into_any_element()
    }
}

fn ssh_color(hex: &str) -> Hsla {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x8c9aaa);
    rgb(value).into()
}
