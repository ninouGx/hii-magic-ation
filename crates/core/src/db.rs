use std::path::Path;
use std::sync::{Arc, Mutex};

use duckdb::Connection;

pub const USER_DB_FILE: &str = "user.duckdb";
pub const CATALOG_DB_FILE: &str = "catalog.duckdb";
pub const CATALOG_SCHEMA: &str = "catalog";

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    DuckDb(#[from] duckdb::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("database connection lock poisoned")]
    Poisoned,
}

/// One `DuckDB` instance per process. `user.duckdb` holds the data that cannot be
/// rebuilt and is the default database. The catalog is rebuilt by each sync, so
/// it is attached read-only and only a sync replaces it.
#[derive(Clone)]
pub struct Db {
    root: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DbInfo {
    pub duckdb_version: String,
    pub databases: Vec<String>,
}

impl Db {
    pub fn open(data_dir: &Path) -> Result<Self, DbError> {
        std::fs::create_dir_all(data_dir)?;
        let catalog_path = data_dir.join(CATALOG_DB_FILE);
        if !catalog_path.exists() {
            Connection::open(&catalog_path)?;
        }
        let conn = Connection::open(data_dir.join(USER_DB_FILE))?;
        conn.execute_batch(&format!(
            "ATTACH {} AS {CATALOG_SCHEMA} (READ_ONLY)",
            sql_string_literal(&catalog_path.to_string_lossy())
        ))?;
        Ok(Self {
            root: Arc::new(Mutex::new(conn)),
        })
    }

    /// Connections share the instance and its attachments, and each one can run
    /// on its own thread.
    pub fn connection(&self) -> Result<Connection, DbError> {
        let root = self.root.lock().map_err(|_| DbError::Poisoned)?;
        Ok(root.try_clone()?)
    }

    pub fn info(&self) -> Result<DbInfo, DbError> {
        let conn = self.connection()?;
        let duckdb_version = conn.query_row("SELECT version()", [], |row| row.get(0))?;
        let mut stmt = conn.prepare(
            "SELECT database_name FROM duckdb_databases() WHERE NOT internal ORDER BY 1",
        )?;
        let databases = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(DbInfo {
            duckdb_version,
            databases,
        })
    }
}

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_string_literal_escapes_quotes() {
        assert_eq!(sql_string_literal("it's"), "'it''s'");
    }
}
