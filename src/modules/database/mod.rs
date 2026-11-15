pub mod actions;
pub mod connection_editor;
pub mod dialogs;
pub mod driver;
pub mod form;
pub mod menu;
pub mod model;
mod mysql_driver;
mod postgres_driver;
pub mod query_view;
pub mod sidebar;
pub mod sqlite;

use adabraka_ui::components::input_state::InputState;
use gpui::Entity;
use model::DatabaseRegistry;
use std::collections::{HashMap, HashSet};

use crate::model::session::SessionKey;
use form::{CatalogState, DatabaseForm};

/// Database connections and their live query panes. Only connection metadata
/// and query text are persisted; result rows and active executions stay in
/// memory.
pub struct DatabaseModule {
    pub registry: DatabaseRegistry,
    pub search: Entity<InputState>,
    pub form: DatabaseForm,
    pub expanded: bool,
    pub query_views: HashMap<(SessionKey, u64), Entity<query_view::DatabaseQueryView>>,
    pub catalogs: HashMap<String, CatalogState>,
    pub expanded_nodes: HashSet<String>,
}

impl DatabaseModule {
    pub fn with_form(
        registry: DatabaseRegistry,
        search: Entity<InputState>,
        form: DatabaseForm,
        expanded: bool,
    ) -> Self {
        Self {
            registry,
            search,
            form,
            expanded,
            query_views: HashMap::new(),
            catalogs: HashMap::new(),
            expanded_nodes: HashSet::new(),
        }
    }

    pub fn connection(&self, id: &str) -> Option<&model::DatabaseConnection> {
        self.registry
            .connections
            .iter()
            .find(|connection| connection.id == id)
    }

    pub fn drop_query(&mut self, key: &SessionKey, pane: u64) {
        self.query_views.remove(&(key.clone(), pane));
    }

    pub fn drop_session(&mut self, key: &SessionKey) {
        self.query_views.retain(|(session, _), _| session != key);
    }
}
