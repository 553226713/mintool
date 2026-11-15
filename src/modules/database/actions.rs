use super::{
    driver,
    form::CatalogState,
    model::{DatabaseConnection, DatabaseEngine},
};
use crate::app::MintoolApp;
use crate::model::query::QueryPane;
use crate::modules::{ModuleId, SessionKey};
use gpui::Context;

impl MintoolApp {
    pub(crate) fn open_database_connection(&mut self, id: String, cx: &mut Context<Self>) {
        let key = SessionKey::database(id);
        self.select_session(key.clone(), cx);
        self.ensure_session(&key);
        self.focus_pending = true;
        self.changed(cx);
    }

    pub(crate) fn remove_database_connection(&mut self, id: String, cx: &mut Context<Self>) {
        self.modules.database.registry.remove(&id);
        self.modules.database.catalogs.remove(&id);
        self.modules
            .database
            .expanded_nodes
            .retain(|node| !node.starts_with(&format!("{id}/")) && node != &id);
        self.remove_session(&SessionKey::database(id), cx);
        self.modules.active = ModuleId::Database;
        self.settings.active_module = ModuleId::Database;
        self.changed(cx);
    }

    pub(crate) fn toggle_database_connection(&mut self, id: String, cx: &mut Context<Self>) {
        if self.modules.database.expanded_nodes.remove(&id) {
            self.changed(cx);
            return;
        }
        self.modules.database.expanded_nodes.insert(id.clone());
        if self
            .modules
            .database
            .catalogs
            .get(&id)
            .is_none_or(|catalog| catalog.catalog.is_none() && !catalog.loading)
        {
            self.load_database_catalog(id, cx);
        }
        self.changed(cx);
    }

    pub(crate) fn toggle_database_node(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.modules.database.expanded_nodes.remove(&key) {
            self.modules.database.expanded_nodes.insert(key);
        }
        self.changed(cx);
    }

    pub(crate) fn load_database_catalog(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(connection) = self.modules.database.connection(&id).cloned() else {
            return;
        };
        self.modules.database.catalogs.insert(
            id.clone(),
            CatalogState {
                loading: true,
                ..CatalogState::default()
            },
        );
        let executor = cx.background_executor().clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = executor
                .spawn(async move { driver::catalog(&connection) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.modules.database.catalogs.insert(
                    id,
                    match result {
                        Ok(catalog) => CatalogState {
                            loading: false,
                            catalog: Some(catalog),
                            error: None,
                        },
                        Err(error) => CatalogState {
                            loading: false,
                            catalog: None,
                            error: Some(format!("{error:#}")),
                        },
                    },
                );
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn preview_database_table(
        &mut self,
        id: String,
        database: String,
        schema: String,
        table: String,
        cx: &mut Context<Self>,
    ) {
        let Some(connection) = self.modules.database.connection(&id).cloned() else {
            return;
        };
        self.open_database_connection(id.clone(), cx);
        let key = SessionKey::database(id);
        let sql = table_preview_sql(&connection, &database, &schema, &table);
        if let Some(workspace) = self.workspaces.get_mut(&key) {
            let anchor = if workspace
                .state
                .layout
                .find(workspace.state.focused)
                .is_some()
                || workspace.state.files.contains_key(&workspace.state.focused)
                || workspace
                    .state
                    .queries
                    .contains_key(&workspace.state.focused)
            {
                workspace.state.focused
            } else {
                workspace.state.layout.first_id()
            };
            let query_id = workspace.state.next_id;
            workspace.state.next_id = workspace.state.next_id.saturating_add(1);
            workspace.state.queries.insert(
                query_id,
                QueryPane {
                    title: table.clone(),
                    sql,
                    auto_run: true,
                },
            );
            self.dock_file_tabs.push((key.clone(), anchor, query_id));
            if let Some(workspace) = self.workspaces.get_mut(&key) {
                workspace.state.focused = query_id;
            }
        }
        self.modules
            .database
            .drop_query(&key, self.workspaces[&key].state.focused);
        self.changed(cx);
    }
}

fn table_preview_sql(
    connection: &DatabaseConnection,
    database: &str,
    schema: &str,
    table: &str,
) -> String {
    match connection.engine {
        DatabaseEngine::Sqlite => format!(
            "SELECT * FROM \"{}\" LIMIT 100;\n",
            table.replace('"', "\"\"")
        ),
        DatabaseEngine::Postgres => format!(
            "SELECT * FROM \"{}\".\"{}\" LIMIT 100;\n",
            schema.replace('"', "\"\""),
            table.replace('"', "\"\"")
        ),
        DatabaseEngine::Mysql | DatabaseEngine::Mariadb => format!(
            "SELECT * FROM `{}`.`{}` LIMIT 100;\n",
            database.replace('`', "``"),
            table.replace('`', "``")
        ),
    }
}
