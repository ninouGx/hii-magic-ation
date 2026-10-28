use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::catalog::{self, CatalogCounts, CatalogError, RawLayout};
use crate::edhrec::{self, EdhrecError, EdhrecSync, PopularitySource};
use crate::scryfall::{self, BulkSync, CardSource, ScryfallError};

/// A new catalog with fewer cards than this share of the previous one is
/// treated as a broken upstream file, not as real data.
const MIN_KEPT_CARDS_PERCENT: i64 = 90;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Scryfall(#[from] ScryfallError),
    #[error(transparent)]
    Edhrec(#[from] EdhrecError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("the new catalog has {built} cards against {previous} before, kept the previous one")]
    Shrunk { previous: i64, built: i64 },
    #[error("catalog build task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

#[derive(Debug, Clone, Copy, Default, serde::Deserialize, Serialize)]
pub struct SyncPlan {
    pub scryfall: bool,
    pub edhrec: bool,
    /// Rebuild the catalog even when no source changed.
    pub rebuild: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct SyncReport {
    pub scryfall: Option<BulkSync>,
    pub edhrec: Option<EdhrecSync>,
    /// Counts of the catalog written to `out`, `None` when nothing needed a rebuild.
    pub catalog: Option<CatalogCounts>,
}

/// Downloads the planned sources, then builds a catalog at `out` when a source
/// changed, when `previous` is `None`, or when the plan asks for a rebuild.
pub async fn run(
    cards: &impl CardSource,
    popularity: &impl PopularitySource,
    raw: &RawLayout,
    out: &Path,
    previous: Option<&CatalogCounts>,
    plan: SyncPlan,
    now_unix: u64,
) -> Result<SyncReport, SyncError> {
    let mut report = SyncReport::default();
    if plan.scryfall {
        report.scryfall = Some(scryfall::sync_bulks(cards, raw).await?);
    }
    if plan.edhrec {
        report.edhrec = Some(edhrec::sync_commander_pages(popularity, raw, now_unix).await?);
    }

    let changed = report
        .scryfall
        .as_ref()
        .is_some_and(|sync| !sync.downloaded.is_empty())
        || report
            .edhrec
            .as_ref()
            .is_some_and(|sync| !sync.fetched.is_empty());
    if !(changed || plan.rebuild || previous.is_none()) {
        return Ok(report);
    }

    let built = build_catalog(raw.clone(), out.to_owned()).await?;
    if let Some(previous) = previous
        && built.cards * 100 < previous.cards * MIN_KEPT_CARDS_PERCENT
    {
        catalog::remove_database_files(out).map_err(CatalogError::from)?;
        return Err(SyncError::Shrunk {
            previous: previous.cards,
            built: built.cards,
        });
    }
    report.catalog = Some(built);
    Ok(report)
}

async fn build_catalog(raw: RawLayout, out: PathBuf) -> Result<CatalogCounts, SyncError> {
    Ok(tokio::task::spawn_blocking(move || catalog::build(&raw, &out)).await??)
}
