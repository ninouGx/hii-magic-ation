use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;

use hma_ingest::catalog::{RawLayout, SCRYFALL_BULKS};
use hma_ingest::scryfall::{self, BulkFile, CardSource, ScryfallError};

struct FakeSource {
    index: Mutex<Vec<BulkFile>>,
    downloads: Mutex<Vec<String>>,
}

impl FakeSource {
    fn new(updated_at: &str) -> Self {
        let index = SCRYFALL_BULKS
            .iter()
            .map(|bulk| BulkFile {
                kind: (*bulk).to_owned(),
                updated_at: updated_at.to_owned(),
                jsonl_download_uri: format!("https://data.example/{bulk}-{updated_at}.jsonl.gz"),
            })
            .collect();
        Self {
            index: Mutex::new(index),
            downloads: Mutex::new(Vec::new()),
        }
    }

    fn bump(&self, bulk: &str, updated_at: &str) {
        let mut index = self.index.lock().unwrap();
        let entry = index.iter_mut().find(|e| e.kind == bulk).unwrap();
        updated_at.clone_into(&mut entry.updated_at);
        entry.jsonl_download_uri = format!("https://data.example/{bulk}-{updated_at}.jsonl.gz");
    }

    fn take_downloads(&self) -> Vec<String> {
        std::mem::take(&mut self.downloads.lock().unwrap())
    }
}

impl CardSource for FakeSource {
    async fn bulk_index(&self) -> Result<Vec<BulkFile>, ScryfallError> {
        Ok(self.index.lock().unwrap().clone())
    }

    async fn download(&self, url: &str, dest: &Path) -> Result<(), ScryfallError> {
        self.downloads.lock().unwrap().push(url.to_owned());
        tokio::fs::write(dest, url).await?;
        Ok(())
    }

    async fn search_oracle_ids(&self, _query: &str) -> Result<BTreeSet<String>, ScryfallError> {
        Ok(BTreeSet::new())
    }
}

#[tokio::test]
async fn first_sync_downloads_every_bulk() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawLayout::new(dir.path());
    let source = FakeSource::new("2026-10-28");

    let outcome = scryfall::sync_bulks(&source, &raw).await.unwrap();

    assert_eq!(outcome.downloaded, SCRYFALL_BULKS);
    for bulk in SCRYFALL_BULKS {
        assert!(raw.scryfall_file(bulk).is_file());
    }
}

#[tokio::test]
async fn second_sync_without_upstream_change_downloads_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawLayout::new(dir.path());
    let source = FakeSource::new("2026-10-28");
    scryfall::sync_bulks(&source, &raw).await.unwrap();
    source.take_downloads();

    let outcome = scryfall::sync_bulks(&source, &raw).await.unwrap();

    assert!(source.take_downloads().is_empty());
    assert_eq!(outcome.unchanged, SCRYFALL_BULKS);
}

#[tokio::test]
async fn sync_downloads_only_the_bulks_that_changed() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawLayout::new(dir.path());
    let source = FakeSource::new("2026-10-28");
    scryfall::sync_bulks(&source, &raw).await.unwrap();
    source.take_downloads();
    source.bump("oracle_tags", "2026-10-29");

    let outcome = scryfall::sync_bulks(&source, &raw).await.unwrap();

    assert_eq!(outcome.downloaded, ["oracle_tags"]);
    assert_eq!(
        source.take_downloads(),
        ["https://data.example/oracle_tags-2026-10-29.jsonl.gz"]
    );
    let versions = scryfall::bulk_versions(&raw).await.unwrap();
    assert_eq!(versions["oracle_tags"], "2026-10-29");
    assert_eq!(versions["rulings"], "2026-10-28");
}

#[tokio::test]
async fn deleted_raw_file_is_downloaded_again() {
    let dir = tempfile::tempdir().unwrap();
    let raw = RawLayout::new(dir.path());
    let source = FakeSource::new("2026-10-28");
    scryfall::sync_bulks(&source, &raw).await.unwrap();
    std::fs::remove_file(raw.scryfall_file("rulings")).unwrap();

    let outcome = scryfall::sync_bulks(&source, &raw).await.unwrap();

    assert_eq!(outcome.downloaded, ["rulings"]);
}

#[tokio::test]
async fn bulk_missing_from_index_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let source = FakeSource::new("2026-10-28");
    source
        .index
        .lock()
        .unwrap()
        .retain(|e| e.kind != "art_tags");

    let err = scryfall::sync_bulks(&source, &RawLayout::new(dir.path()))
        .await
        .unwrap_err();

    assert!(matches!(err, ScryfallError::MissingBulk("art_tags")));
}
