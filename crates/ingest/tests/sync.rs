use std::collections::BTreeSet;
use std::path::Path;

use hma_ingest::catalog::{CatalogCounts, RawLayout, SCRYFALL_BULKS};
use hma_ingest::edhrec::{EdhrecError, PopularitySource};
use hma_ingest::scryfall::{BulkFile, CardSource, ScryfallError};
use hma_ingest::sync::{self, SyncError, SyncPlan};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const NOW: u64 = 1_800_000_000;
const BOTH: SyncPlan = SyncPlan {
    scryfall: true,
    edhrec: true,
    rebuild: false,
};

struct FixtureBulks;

impl CardSource for FixtureBulks {
    async fn bulk_index(&self) -> Result<Vec<BulkFile>, ScryfallError> {
        Ok(SCRYFALL_BULKS
            .iter()
            .map(|bulk| BulkFile {
                kind: (*bulk).to_owned(),
                updated_at: "2026-10-28".to_owned(),
                jsonl_download_uri: (*bulk).to_owned(),
            })
            .collect())
    }

    async fn download(&self, url: &str, dest: &Path) -> Result<(), ScryfallError> {
        tokio::fs::copy(RawLayout::new(FIXTURES).scryfall_file(url), dest).await?;
        Ok(())
    }

    async fn search_oracle_ids(&self, _query: &str) -> Result<BTreeSet<String>, ScryfallError> {
        Ok(BTreeSet::new())
    }
}

struct FixturePages;

impl PopularitySource for FixturePages {
    async fn fetch(&self, path: &str) -> Result<Vec<u8>, EdhrecError> {
        Ok(std::fs::read(
            Path::new(FIXTURES).join("edhrec").join(path),
        )?)
    }
}

async fn first_sync(dir: &Path) -> (RawLayout, CatalogCounts) {
    let raw = RawLayout::new(dir.join("raw"));
    let report = sync::run(
        &FixtureBulks,
        &FixturePages,
        &raw,
        &dir.join("catalog.duckdb.new"),
        None,
        BOTH,
        NOW,
    )
    .await
    .unwrap();
    (raw, report.catalog.unwrap())
}

#[tokio::test(start_paused = true)]
async fn first_sync_downloads_and_builds_the_catalog() {
    let dir = tempfile::tempdir().unwrap();

    let (_, counts) = first_sync(dir.path()).await;

    assert_eq!((counts.cards, counts.edhrec_ranks), (20, 15));
    assert!(dir.path().join("catalog.duckdb.new").is_file());
}

#[tokio::test(start_paused = true)]
async fn unchanged_sources_skip_the_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    let (raw, counts) = first_sync(dir.path()).await;
    let out = dir.path().join("next.duckdb");

    let report = sync::run(
        &FixtureBulks,
        &FixturePages,
        &raw,
        &out,
        Some(&counts),
        BOTH,
        NOW + 60,
    )
    .await
    .unwrap();

    assert!(report.catalog.is_none());
    assert!(!out.exists());
}

#[tokio::test(start_paused = true)]
async fn rebuild_flag_builds_without_source_change() {
    let dir = tempfile::tempdir().unwrap();
    let (raw, counts) = first_sync(dir.path()).await;
    let plan = SyncPlan {
        rebuild: true,
        ..BOTH
    };

    let report = sync::run(
        &FixtureBulks,
        &FixturePages,
        &raw,
        &dir.path().join("next.duckdb"),
        Some(&counts),
        plan,
        NOW + 60,
    )
    .await
    .unwrap();

    assert_eq!(report.catalog.unwrap().cards, 20);
}

#[tokio::test(start_paused = true)]
async fn catalog_that_lost_cards_is_rejected_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let (raw, counts) = first_sync(dir.path()).await;
    let previous = CatalogCounts {
        cards: 1000,
        ..counts
    };
    let out = dir.path().join("next.duckdb");
    let plan = SyncPlan {
        rebuild: true,
        ..BOTH
    };

    let err = sync::run(
        &FixtureBulks,
        &FixturePages,
        &raw,
        &out,
        Some(&previous),
        plan,
        NOW,
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err,
        SyncError::Shrunk {
            previous: 1000,
            built: 20
        }
    ));
    assert!(!out.exists());
}
