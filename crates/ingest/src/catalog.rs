use std::path::{Path, PathBuf};

use hma_core::db::sql_string_literal;
use hma_core::duckdb::{self, Connection};
use serde::Serialize;

pub const SCRYFALL_BULKS: [&str; 5] = [
    "oracle_cards",
    "default_cards",
    "rulings",
    "oracle_tags",
    "art_tags",
];

const SCRYFALL_STEPS: [(&str, &str); 5] = [
    ("cards", include_str!("../sql/01_cards.sql")),
    ("prints", include_str!("../sql/02_prints.sql")),
    ("rulings", include_str!("../sql/03_rulings.sql")),
    ("tags", include_str!("../sql/04_tags.sql")),
    ("edhrec_schema", include_str!("../sql/05_edhrec_schema.sql")),
];
const EDHREC_STEP: (&str, &str) = ("edhrec", include_str!("../sql/06_edhrec.sql"));
const VALIDATE_STEP: (&str, &str) = ("validate", include_str!("../sql/07_validate.sql"));
const EDHREC_PAGES: &str = "edhrec_commander_pages";

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("raw file missing: {0}")]
    MissingRaw(PathBuf),
    #[error("catalog step `{step}` failed: {source}")]
    Step {
        step: &'static str,
        source: duckdb::Error,
    },
    #[error(transparent)]
    DuckDb(#[from] duckdb::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Downloaded source files. The catalog is a pure function of this directory.
#[derive(Debug, Clone)]
pub struct RawLayout {
    root: PathBuf,
}

impl RawLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn scryfall_dir(&self) -> PathBuf {
        self.root.join("scryfall")
    }

    #[must_use]
    pub fn scryfall_file(&self, bulk: &str) -> PathBuf {
        self.scryfall_dir().join(format!("{bulk}.jsonl.gz"))
    }

    #[must_use]
    pub fn edhrec_commanders_dir(&self) -> PathBuf {
        self.root.join("edhrec").join("commanders")
    }

    fn edhrec_commander_pages(&self) -> Vec<PathBuf> {
        let Ok(windows) = std::fs::read_dir(self.edhrec_commanders_dir()) else {
            return Vec::new();
        };
        let mut pages: Vec<PathBuf> = windows
            .flatten()
            .filter(|entry| {
                entry.path().is_dir()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| crate::edhrec::WINDOWS.contains(&name))
            })
            .filter_map(|entry| std::fs::read_dir(entry.path()).ok())
            .flat_map(|dir| dir.flatten().map(|page| page.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        pages.sort();
        pages
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogCounts {
    pub cards: i64,
    pub commanders: i64,
    pub cards_without_print: i64,
    pub prints: i64,
    pub rulings: i64,
    pub oracle_tags: i64,
    pub art_tags: i64,
    pub card_tags: i64,
    pub edhrec_ranks: i64,
    pub edhrec_unresolved: i64,
}

impl CatalogCounts {
    /// `schema` is `main` inside a catalog file, `catalog` once attached.
    pub fn read(conn: &Connection, schema: &str) -> Result<Self, duckdb::Error> {
        conn.query_row(
            &format!(
                "SELECT
                    (SELECT count(*) FROM {schema}.cards),
                    (SELECT count(*) FROM {schema}.cards WHERE is_commander_eligible),
                    (SELECT count(*) FROM {schema}.cards WHERE default_print_id IS NULL),
                    (SELECT count(*) FROM {schema}.prints),
                    (SELECT count(*) FROM {schema}.rulings),
                    (SELECT count(*) FROM {schema}.tags WHERE kind = 'oracle'),
                    (SELECT count(*) FROM {schema}.tags WHERE kind = 'art'),
                    (SELECT count(*) FROM {schema}.card_tags),
                    (SELECT count(*) FROM {schema}.edhrec_commander_ranks),
                    (SELECT count(*) FROM {schema}.edhrec_unresolved)"
            ),
            [],
            |row| {
                Ok(Self {
                    cards: row.get(0)?,
                    commanders: row.get(1)?,
                    cards_without_print: row.get(2)?,
                    prints: row.get(3)?,
                    rulings: row.get(4)?,
                    oracle_tags: row.get(5)?,
                    art_tags: row.get(6)?,
                    card_tags: row.get(7)?,
                    edhrec_ranks: row.get(8)?,
                    edhrec_unresolved: row.get(9)?,
                })
            },
        )
    }
}

/// Writes a complete catalog to `out`, replacing any previous file there.
/// EDHREC pages are optional, without them the ranking tables stay empty.
/// The connection is closed on return, so `out` can be renamed and attached.
pub fn build(raw: &RawLayout, out: &Path) -> Result<CatalogCounts, CatalogError> {
    for bulk in SCRYFALL_BULKS {
        let path = raw.scryfall_file(bulk);
        if !path.is_file() {
            return Err(CatalogError::MissingRaw(path));
        }
    }
    remove_database_files(out)?;

    let edhrec_pages = raw.edhrec_commander_pages();
    let mut steps = SCRYFALL_STEPS.to_vec();
    if !edhrec_pages.is_empty() {
        steps.push(EDHREC_STEP);
    }
    steps.push(VALIDATE_STEP);

    let conn = Connection::open(out)?;
    for (step, sql) in steps {
        conn.execute_batch(&bind_raw_paths(sql, raw, &edhrec_pages))
            .map_err(|source| CatalogError::Step { step, source })?;
    }
    let counts = CatalogCounts::read(&conn, "main")?;
    conn.execute_batch("CHECKPOINT")?;
    conn.close().map_err(|(_, e)| e)?;
    Ok(counts)
}

pub fn remove_database_files(path: &Path) -> std::io::Result<()> {
    for file in [path.to_path_buf(), wal_path(path)] {
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

#[must_use]
pub fn wal_path(path: &Path) -> PathBuf {
    let mut wal = path.as_os_str().to_owned();
    wal.push(".wal");
    PathBuf::from(wal)
}

fn bind_raw_paths(sql: &str, raw: &RawLayout, edhrec_pages: &[PathBuf]) -> String {
    let pages = edhrec_pages
        .iter()
        .map(|page| sql_string_literal(&page.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = sql.replace(&format!("${EDHREC_PAGES}"), &format!("[{pages}]"));
    SCRYFALL_BULKS.iter().fold(sql, |sql, bulk| {
        let path = raw.scryfall_file(bulk);
        sql.replace(
            &format!("${bulk}"),
            &sql_string_literal(&path.to_string_lossy()),
        )
    })
}
