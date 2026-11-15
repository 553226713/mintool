use super::{
    dialogs::DatabaseDialog,
    model::{DatabaseConnection, DatabaseEngine},
};
use crate::app::MintoolApp;
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
use gpui::{prelude::FluentBuilder, *};

impl MintoolApp {
    pub(crate) fn render_database_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (left_width, _) = self
            .settings
            .panel_widths(window.viewport_size().width.into());
        let filter = self
            .modules
            .database
            .search
            .read(cx)
            .content()
            .trim()
            .to_lowercase();
        let mut rows = Vec::new();
        rows.push(
            div()
                .id("database-ungrouped")
                .px_4()
                .pt_3()
                .pb_2()
                .text_size(px(11.))
                .text_color(theme::muted())
                .child("未分组")
                .on_drop(cx.listener(|this, item: &DragItem, _, cx| {
                    if let DragKind::DatabaseConnection(id) = &item.kind {
                        this.modules
                            .database
                            .registry
                            .move_connection(id, None, None);
                        this.changed(cx);
                    }
                }))
                .into_any_element(),
        );
        for connection in self
            .modules
            .database
            .registry
            .connections
            .iter()
            .filter(|connection| {
                connection.group.is_none() && database_matches(connection, &filter)
            })
        {
            rows.extend(self.database_connection_tree(connection, cx));
        }
        let groups = self.modules.database.registry.groups.clone();
        for group in &groups {
            let group_id = group.id.clone();
            let toggle = group_id.clone();
            let menu_id = group_id.clone();
            let menu_name = group.name.clone();
            let drop_id = group_id.clone();
            let drop_group = group_id.clone();
            let payload = DragItem {
                kind: DragKind::DatabaseGroup(group_id.clone()),
                name: group.name.clone(),
            };
            let count = self
                .modules
                .database
                .registry
                .connections
                .iter()
                .filter(|connection| connection.group.as_ref() == Some(&group_id))
                .count();
            rows.push(
                crate::modules::common::sidebar::group_header(
                    SharedString::from(format!("database-group-{group_id}")),
                    group.name.clone(),
                    count,
                    group.collapsed,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(group) = this
                        .modules
                        .database
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
                        this.database_group_menu(
                            menu_id.clone(),
                            menu_name.clone(),
                            event.position,
                            cx,
                        );
                        cx.stop_propagation();
                    }),
                )
                .on_drag(payload, |item, _, _, cx| cx.new(|_| item.clone()))
                .on_drop(cx.listener(move |this, item: &DragItem, _, cx| {
                    match &item.kind {
                        DragKind::DatabaseConnection(id) => this
                            .modules
                            .database
                            .registry
                            .move_connection(id, Some(drop_group.clone()), None),
                        DragKind::DatabaseGroup(id) => {
                            this.modules.database.registry.move_group(id, &drop_id)
                        }
                        _ => {}
                    }
                    this.changed(cx);
                }))
                .into_any_element(),
            );
            if !group.collapsed || !filter.is_empty() {
                for connection in
                    self.modules
                        .database
                        .registry
                        .connections
                        .iter()
                        .filter(|connection| {
                            connection.group.as_ref() == Some(&group_id)
                                && database_matches(connection, &filter)
                        })
                {
                    rows.extend(self.database_connection_tree(connection, cx));
                }
            }
        }
        let has_connections = !self.modules.database.registry.connections.is_empty();
        div()
            .id("database-navigation")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::panel())
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.database_navigation_menu(event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_1()
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
                                    .child("数据库"),
                            )
                            .child(div().text_size(px(10.)).text_color(theme::muted()).child(
                                format!(
                                    "{} 个 SQLite 连接",
                                    self.modules.database.registry.connections.len()
                                ),
                            )),
                    )
                    .child(
                        Button::new("database-add-group", "")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .icon("folder-plus")
                            .tooltip("新建分组")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_dialog(
                                    Dialog::Database(DatabaseDialog::GroupAdd),
                                    window,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("database-add", "")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Sm)
                            .icon("plus")
                            .tooltip("添加或新建 SQLite 数据库")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_dialog(
                                    Dialog::Database(DatabaseDialog::Connection {
                                        edit: None,
                                        group: None,
                                    }),
                                    window,
                                    cx,
                                )
                            })),
                    ),
            )
            .child(
                div().w_full().flex().flex_col().px_4().pb_2().child(
                    Input::new(&self.modules.database.search)
                        .map(|mut input| {
                            input.style().align_items = Some(AlignItems::Stretch);
                            input
                        })
                        .w(px((left_width - 32.).max(120.)))
                        .min_w(px((left_width - 32.).max(120.)))
                        .size(InputSize::Sm)
                        .placeholder("搜索数据库连接…")
                        .prefix(Icon::new("search").size(px(13.)).color(theme::muted())),
                ),
            )
            .child(
                div()
                    .id("database-connection-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(!has_connections, |list| {
                        list.child(
                            div()
                                .p_4()
                                .text_size(px(11.))
                                .text_color(theme::muted())
                                .child("还没有数据库连接"),
                        )
                    })
                    .when(has_connections, |list| list.children(rows)),
            )
            .child(
                div()
                    .p_4()
                    .border_t_1()
                    .border_color(theme::border())
                    .text_size(px(10.))
                    .text_color(theme::muted())
                    .text_center()
                    .child("右键管理分组与连接"),
            )
            .into_any_element()
    }

    fn database_connection_row(
        &self,
        connection: &DatabaseConnection,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active =
            self.modules.database.registry.selected.as_deref() == Some(connection.id.as_str());
        let open_id = connection.id.clone();
        let menu_connection = connection.clone();
        let payload = DragItem {
            kind: DragKind::DatabaseConnection(connection.id.clone()),
            name: connection.name.clone(),
        };
        let drop_group = connection.group.clone();
        let drop_before = connection.id.clone();
        let expanded = self
            .modules
            .database
            .expanded_nodes
            .contains(&connection.id);
        let toggle_id = connection.id.clone();
        div()
            .id(SharedString::from(format!(
                "database-connection-{}",
                connection.id
            )))
            .mx_3()
            .my(px(2.))
            .h(px(44.))
            .px_2()
            .rounded_lg()
            .border_1()
            .border_color(if active {
                theme::selected_border()
            } else {
                theme::panel()
            })
            .bg(if active {
                theme::selected()
            } else {
                theme::panel()
            })
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .hover(|row| row.bg(theme::elevated()))
            .child(
                Button::new(
                    SharedString::from(format!("database-toggle-{}", connection.id)),
                    "",
                )
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Sm)
                .icon(if expanded {
                    "chevron-down"
                } else {
                    "chevron-right"
                })
                .tooltip(if expanded {
                    "收起数据库"
                } else {
                    "展开数据库"
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_database_connection(toggle_id.clone(), cx);
                    cx.stop_propagation();
                })),
            )
            .child(Icon::new("file-database").size(px(15.)).color(if active {
                theme::accent()
            } else {
                theme::muted()
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
                            .text_color(if active {
                                theme::accent()
                            } else {
                                theme::text()
                            })
                            .child(connection.name.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(9.))
                            .text_color(theme::muted())
                            .child(connection.subtitle()),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_database_connection(open_id.clone(), cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.database_connection_menu(menu_connection.clone(), event.position, cx);
                    cx.stop_propagation();
                }),
            )
            .on_drag(payload, |item, _, _, cx| cx.new(|_| item.clone()))
            .on_drop(cx.listener(move |this, item: &DragItem, _, cx| {
                if let DragKind::DatabaseConnection(id) = &item.kind {
                    this.modules.database.registry.move_connection(
                        id,
                        drop_group.clone(),
                        Some(&drop_before),
                    );
                    this.changed(cx);
                }
            }))
            .into_any_element()
    }

    fn database_connection_tree(
        &self,
        connection: &DatabaseConnection,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = vec![self.database_connection_row(connection, cx)];
        let connection_id = connection.id.clone();
        if !self
            .modules
            .database
            .expanded_nodes
            .contains(&connection_id)
        {
            return rows;
        }
        let Some(state) = self.modules.database.catalogs.get(&connection_id) else {
            return rows;
        };
        if state.loading {
            rows.push(tree_message("正在读取数据库结构…", theme::muted()));
            return rows;
        }
        if let Some(error) = &state.error {
            rows.push(tree_message(error, theme::danger()));
            return rows;
        }
        let Some(catalog) = state.catalog.clone() else {
            return rows;
        };
        for database in catalog.databases {
            let database_key = format!("{connection_id}/{}", database.name);
            let database_expanded = self.modules.database.expanded_nodes.contains(&database_key);
            rows.push(tree_folder_row(
                cx,
                database_key.clone(),
                1.0,
                database.name.clone(),
                database
                    .schemas
                    .iter()
                    .map(|schema| schema.tables.len())
                    .sum(),
                database_expanded,
            ));
            if !database_expanded {
                continue;
            }
            let schema_count = database.schemas.len();
            for schema in database.schemas {
                let schema_key = format!("{database_key}/{}", schema.name);
                let show_schema = !(schema_count == 1
                    && (schema.name == database.name
                        || connection.engine == DatabaseEngine::Sqlite));
                let schema_expanded =
                    !show_schema || self.modules.database.expanded_nodes.contains(&schema_key);
                if show_schema {
                    rows.push(tree_folder_row(
                        cx,
                        schema_key.clone(),
                        2.0,
                        schema.name.clone(),
                        schema.tables.len(),
                        schema_expanded,
                    ));
                }
                if show_schema && !schema_expanded {
                    continue;
                }
                for table in schema.tables {
                    let table_key = format!("{schema_key}/{}", table.name);
                    let table_expanded = self.modules.database.expanded_nodes.contains(&table_key);
                    let preview_id = connection_id.clone();
                    let preview_database = database.name.clone();
                    let preview_schema = schema.name.clone();
                    let preview_table = table.name.clone();
                    let toggle_key = table_key.clone();
                    rows.push(
                        div()
                            .id(SharedString::from(format!("database-table-{table_key}")))
                            .ml(px(54.))
                            .mr_3()
                            .h(px(30.))
                            .px_2()
                            .rounded_md()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .hover(|row| row.bg(theme::elevated()))
                            .child(
                                Icon::new(if table_expanded {
                                    "chevron-down"
                                } else {
                                    "chevron-right"
                                })
                                .size(px(11.))
                                .color(theme::muted()),
                            )
                            .child(
                                Icon::new(if table.kind.to_lowercase().contains("view") {
                                    "files"
                                } else {
                                    "file-database"
                                })
                                .size(px(13.))
                                .color(theme::muted()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(table.name.clone()),
                            )
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                if event.click_count() >= 2 {
                                    this.preview_database_table(
                                        preview_id.clone(),
                                        preview_database.clone(),
                                        preview_schema.clone(),
                                        preview_table.clone(),
                                        cx,
                                    );
                                } else {
                                    this.toggle_database_node(toggle_key.clone(), cx);
                                }
                            }))
                            .into_any_element(),
                    );
                    if table_expanded {
                        for column in table.columns {
                            rows.push(
                                div()
                                    .ml(px(78.))
                                    .mr_3()
                                    .h(px(26.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(10.))
                                    .text_color(theme::muted())
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_color(if column.primary_key {
                                                theme::warning()
                                            } else {
                                                theme::text()
                                            })
                                            .child(format!(
                                                "{}{}",
                                                column.name,
                                                if column.primary_key { " · PK" } else { "" }
                                            )),
                                    )
                                    .child(div().max_w(px(92.)).truncate().child(column.data_type))
                                    .when(!column.nullable, |row| {
                                        row.child(div().child("NOT NULL"))
                                    })
                                    .into_any_element(),
                            );
                        }
                    }
                }
            }
        }
        rows
    }
}

fn database_matches(connection: &DatabaseConnection, filter: &str) -> bool {
    filter.is_empty()
        || connection.name.to_lowercase().contains(filter)
        || connection.subtitle().to_lowercase().contains(filter)
}

fn tree_message(message: &str, color: gpui::Hsla) -> AnyElement {
    div()
        .ml(px(30.))
        .mr_3()
        .p_2()
        .text_size(px(10.))
        .text_color(color)
        .child(message.to_string())
        .into_any_element()
}

fn tree_folder_row(
    cx: &mut Context<MintoolApp>,
    key: String,
    depth: f32,
    name: String,
    count: usize,
    expanded: bool,
) -> AnyElement {
    let toggle = key.clone();
    div()
        .id(SharedString::from(format!("database-tree-{key}")))
        .ml(px(28. + depth * 14.))
        .mr_3()
        .h(px(30.))
        .px_2()
        .rounded_md()
        .flex()
        .items_center()
        .gap_2()
        .cursor_pointer()
        .hover(|row| row.bg(theme::elevated()))
        .child(
            Icon::new(if expanded {
                "chevron-down"
            } else {
                "chevron-right"
            })
            .size(px(11.))
            .color(theme::muted()),
        )
        .child(
            Icon::new("folder-closed")
                .size(px(13.))
                .color(theme::accent()),
        )
        .child(div().flex_1().min_w_0().truncate().child(name))
        .child(
            div()
                .text_size(px(9.))
                .text_color(theme::muted())
                .child(count.to_string()),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.toggle_database_node(toggle.clone(), cx);
        }))
        .into_any_element()
}
