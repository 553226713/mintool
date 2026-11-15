use super::model::{DatabaseCatalog, DatabaseEngine};
use adabraka_ui::components::{input_state::InputState, select::Select};
use gpui::Entity;

pub struct DatabaseForm {
    pub engine: Entity<Select<DatabaseEngine>>,
    pub group: Entity<Select<Option<String>>>,
    pub name: Entity<InputState>,
    pub path: Entity<InputState>,
    pub host: Entity<InputState>,
    pub port: Entity<InputState>,
    pub database: Entity<InputState>,
    pub user: Entity<InputState>,
    pub password: Entity<InputState>,
    pub testing: bool,
    pub test_status: Option<(bool, String)>,
}

#[derive(Default)]
pub struct CatalogState {
    pub loading: bool,
    pub catalog: Option<DatabaseCatalog>,
    pub error: Option<String>,
}
