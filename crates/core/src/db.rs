use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use duckdb::Connection;

pub const USER_DB_FILE: &str = "user.duckdb";
pub const CATALOG_DB_FILE: &str = "catalog.duckdb";
pub const CATALOG_SCHEMA: &str = "catalog";
/// A sync writes the next catalog here, then the catalog swap renames it.
pub const CATALOG_NEW_FILE: &str = "catalog.duckdb.new";

/// Applied in order, once each. Never edit a released migration, add the next one.
const USER_MIGRATIONS: &[(i32, &str)] = &[(1, include_str!("../sql/user/001_selections.sql"))];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    DuckDb(#[from] duckdb::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("database connection lock poisoned")]
    Poisoned,
    #[error("{0} still has a write-ahead log, its writer did not close it")]
    UnclosedCatalog(PathBuf),
}

/// One `DuckDB` instance per process. `user.duckdb` holds the data that cannot be
/// rebuilt and is the default database. The catalog is rebuilt by each sync, so
/// it is attached read-only and only a sync replaces it.
#[derive(Clone)]
pub struct Db {
    root: Arc<Mutex<Connection>>,
    data_dir: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DbInfo {
    pub duckdb_version: String,
    pub databases: Vec<String>,
}

impl Db {
    /// A catalog left by a sync that ran while the server was down is
    /// promoted before the attach.
    pub fn open(data_dir: &Path) -> Result<Self, DbError> {
        std::fs::create_dir_all(data_dir)?;
        promote_pending_catalog(data_dir)?;
        let catalog_path = data_dir.join(CATALOG_DB_FILE);
        if !catalog_path.exists() {
            Connection::open(&catalog_path)?;
        }
        let conn = Connection::open(data_dir.join(USER_DB_FILE))?;
        migrate_user(&conn)?;
        attach_catalog(&conn, &catalog_path)?;
        Ok(Self {
            root: Arc::new(Mutex::new(conn)),
            data_dir: data_dir.to_owned(),
        })
    }

    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Replaces the attached catalog with `catalog.duckdb.new` when one is
    /// pending. Fails while another connection still runs a query on the
    /// catalog, and the previous catalog then stays attached.
    pub fn swap_catalog(&self) -> Result<bool, DbError> {
        let root = self.root.lock().map_err(|_| DbError::Poisoned)?;
        if !self.data_dir.join(CATALOG_NEW_FILE).is_file() {
            return Ok(false);
        }
        root.execute_batch(&format!("DETACH {CATALOG_SCHEMA}"))?;
        let promoted = promote_pending_catalog(&self.data_dir);
        attach_catalog(&root, &self.data_dir.join(CATALOG_DB_FILE))?;
        promoted
    }

    #[must_use]
    pub fn catalog_ready(&self) -> bool {
        self.connection()
            .and_then(|conn| {
                Ok(conn.query_row(
                    "SELECT count(*) > 0 FROM duckdb_tables() WHERE database_name = ? AND table_name = 'cards'",
                    [CATALOG_SCHEMA],
                    |row| row.get(0),
                )?)
            })
            .unwrap_or(false)
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

fn migrate_user(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS user_schema_version (version INTEGER NOT NULL)",
    )?;
    let current: i32 = conn.query_row(
        "SELECT coalesce(max(version), 0) FROM user_schema_version",
        [],
        |row| row.get(0),
    )?;
    for (version, sql) in USER_MIGRATIONS
        .iter()
        .filter(|(version, _)| *version > current)
    {
        conn.execute_batch(&format!(
            "BEGIN; {sql}; INSERT INTO user_schema_version VALUES ({version}); COMMIT;"
        ))?;
    }
    Ok(())
}

fn attach_catalog(conn: &Connection, path: &Path) -> Result<(), DbError> {
    conn.execute_batch(&format!(
        "ATTACH {} AS {CATALOG_SCHEMA} (READ_ONLY)",
        sql_string_literal(&path.to_string_lossy())
    ))?;
    Ok(())
}

fn promote_pending_catalog(data_dir: &Path) -> Result<bool, DbError> {
    let pending = data_dir.join(CATALOG_NEW_FILE);
    if !pending.is_file() {
        return Ok(false);
    }
    if wal_path(&pending).exists() {
        return Err(DbError::UnclosedCatalog(pending));
    }
    let catalog = data_dir.join(CATALOG_DB_FILE);
    match std::fs::remove_file(wal_path(&catalog)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    std::fs::rename(pending, catalog)?;
    Ok(true)
}

#[must_use]
pub fn wal_path(path: &Path) -> PathBuf {
    let mut wal = path.as_os_str().to_owned();
    wal.push(".wal");
    PathBuf::from(wal)
}

#[must_use]
pub fn sql_string_literal(value: &str) -> String {
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
