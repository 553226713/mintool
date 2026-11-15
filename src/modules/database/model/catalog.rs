use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DatabaseCatalog {
    pub databases: Vec<CatalogDatabase>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CatalogDatabase {
    pub name: String,
    pub schemas: Vec<CatalogSchema>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CatalogSchema {
    pub name: String,
    pub tables: Vec<CatalogTable>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CatalogTable {
    pub name: String,
    pub kind: String,
    pub columns: Vec<CatalogColumn>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CatalogColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

impl DatabaseCatalog {
    pub fn table_count(&self) -> usize {
        self.databases
            .iter()
            .flat_map(|database| &database.schemas)
            .map(|schema| schema.tables.len())
            .sum()
    }
}
