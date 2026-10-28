use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hma_core::db::{CATALOG_DB_FILE, CATALOG_NEW_FILE};
use hma_core::duckdb::{AccessMode, Config, Connection};
use hma_ingest::catalog::{CatalogCounts, RawLayout};
use hma_ingest::edhrec::EdhrecClient;
use hma_ingest::scryfall::ScryfallClient;
use hma_ingest::sync::{self, SyncPlan, SyncReport};

pub const USER_AGENT: &str = concat!("hii-magic-ation/", env!("CARGO_PKG_VERSION"));

pub fn raw_layout(data_dir: &Path) -> RawLayout {
    RawLayout::new(data_dir.join("raw"))
}

/// Writes the next catalog to `catalog.duckdb.new` when a source changed.
pub async fn run(
    data_dir: &Path,
    previous: Option<CatalogCounts>,
    plan: SyncPlan,
) -> anyhow::Result<SyncReport> {
    let cards = ScryfallClient::new(USER_AGENT)?;
    let popularity = EdhrecClient::new(USER_AGENT)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(sync::run(
        &cards,
        &popularity,
        &raw_layout(data_dir),
        &data_dir.join(CATALOG_NEW_FILE),
        previous.as_ref(),
        plan,
        now,
    )
    .await?)
}

/// The newest catalog on disk: one pending promotion, else the current one.
fn latest_catalog_file(data_dir: &Path) -> PathBuf {
    let pending = data_dir.join(CATALOG_NEW_FILE);
    if pending.is_file() {
        pending
    } else {
        data_dir.join(CATALOG_DB_FILE)
    }
}

/// Opens the newest catalog read-only, which works while the server has it attached.
pub fn open_latest_catalog(data_dir: &Path) -> anyhow::Result<Connection> {
    let path = latest_catalog_file(data_dir);
    anyhow::ensure!(path.is_file(), "no catalog yet, run `sync` first");
    let config = Config::default().access_mode(AccessMode::ReadOnly)?;
    Ok(Connection::open_with_flags(path, config)?)
}

/// `None` when no catalog was built yet.
pub fn latest_catalog_counts(data_dir: &Path) -> Option<CatalogCounts> {
    let conn = open_latest_catalog(data_dir).ok()?;
    CatalogCounts::read(&conn, "main").ok()
}
