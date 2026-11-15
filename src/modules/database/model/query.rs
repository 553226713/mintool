#[derive(Clone, Debug, PartialEq)]
pub enum CellValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(usize),
}

impl CellValue {
    pub fn display(&self) -> String {
        match self {
            Self::Null => "NULL".into(),
            Self::Integer(value) => value.to_string(),
            Self::Real(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::Blob(size) => format!("<{size} bytes>"),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

#[derive(Clone, Debug, Default)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub affected_rows: Option<usize>,
    pub truncated: bool,
    pub elapsed_ms: u128,
    pub sql: String,
}
