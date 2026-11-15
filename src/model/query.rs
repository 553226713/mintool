use serde::{Deserialize, Serialize};

/// Module-neutral persisted state for a query document in the workspace.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct QueryPane {
    pub title: String,
    pub sql: String,
    #[serde(skip)]
    pub auto_run: bool,
}

impl QueryPane {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            sql: "SELECT 1 AS value;".into(),
            auto_run: false,
        }
    }
}

/// Return the SQL statement containing `cursor`, falling back to the complete
/// non-empty input when no statement can be isolated.
pub fn statement_at(sql: &str, cursor: usize) -> String {
    let bytes = sql.as_bytes();
    let cursor = cursor.min(bytes.len());
    let mut start = 0;
    let mut state = ScanState::Normal;
    let mut index = 0;
    let mut previous_end = 0;
    let mut last_statement = None;

    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        match state {
            ScanState::Normal => match (byte, next) {
                (b'-', Some(b'-')) => {
                    state = ScanState::LineComment;
                    index += 1;
                }
                (b'/', Some(b'*')) => {
                    state = ScanState::BlockComment;
                    index += 1;
                }
                (b'\'', _) => state = ScanState::SingleQuote,
                (b'"', _) => state = ScanState::DoubleQuote,
                (b'`', _) => state = ScanState::Backtick,
                (b';', _) => {
                    if cursor <= index {
                        let statement = sql[start..index].trim();
                        if !statement.is_empty() {
                            return statement.to_string();
                        }
                    }
                    let statement = sql[start..index].trim();
                    if !statement.is_empty() {
                        last_statement = Some(statement.to_string());
                    }
                    previous_end = index + 1;
                    start = index + 1;
                }
                _ => {}
            },
            ScanState::LineComment => {
                if byte == b'\n' {
                    state = ScanState::Normal;
                }
            }
            ScanState::BlockComment => {
                if byte == b'*' && next == Some(b'/') {
                    state = ScanState::Normal;
                    index += 1;
                }
            }
            ScanState::SingleQuote => {
                if byte == b'\'' {
                    state = if next == Some(b'\'') {
                        index += 1;
                        ScanState::SingleQuote
                    } else {
                        ScanState::Normal
                    };
                }
            }
            ScanState::DoubleQuote => {
                if byte == b'"' {
                    state = if next == Some(b'"') {
                        index += 1;
                        ScanState::DoubleQuote
                    } else {
                        ScanState::Normal
                    };
                }
            }
            ScanState::Backtick => {
                if byte == b'`' {
                    state = ScanState::Normal;
                }
            }
        }
        index += 1;
    }

    let tail = sql[previous_end..].trim();
    if !tail.is_empty() {
        return tail.to_string();
    }
    last_statement.unwrap_or_else(|| sql.trim().to_string())
}

#[derive(Clone, Copy)]
enum ScanState {
    Normal,
    SingleQuote,
    DoubleQuote,
    Backtick,
    LineComment,
    BlockComment,
}

#[cfg(test)]
mod tests {
    use super::statement_at;

    #[test]
    fn selects_statement_around_cursor() {
        let sql = "SELECT 1;\nSELECT ';' AS value;\nSELECT 3;";
        assert_eq!(statement_at(sql, 3), "SELECT 1");
        assert_eq!(statement_at(sql, 15), "SELECT ';' AS value");
        assert_eq!(statement_at(sql, sql.len()), "SELECT 3");
    }

    #[test]
    fn ignores_comments_and_escaped_quotes() {
        let sql = "-- first; comment\nSELECT 'it''s'; /* second; */ SELECT 2";
        assert_eq!(statement_at(sql, 21), "-- first; comment\nSELECT 'it''s'");
        assert_eq!(statement_at(sql, sql.len()), "/* second; */ SELECT 2");
    }
}
