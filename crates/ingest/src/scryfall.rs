use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::catalog::{RawLayout, SCRYFALL_BULKS};

const API: &str = "https://api.scryfall.com";
/// Scryfall asks for 50 to 100 ms between API requests.
const API_DELAY: Duration = Duration::from_millis(150);
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, thiserror::Error)]
pub enum ScryfallError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid manifest {path}: {source}")]
    Manifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("bulk `{0}` is missing from the Scryfall bulk index")]
    MissingBulk(&'static str),
}

#[derive(Debug, Clone, Deserialize)]
pub struct BulkFile {
    #[serde(rename = "type")]
    pub kind: String,
    pub updated_at: String,
    pub jsonl_download_uri: String,
}

pub trait CardSource: Sync {
    fn bulk_index(&self) -> impl Future<Output = Result<Vec<BulkFile>, ScryfallError>> + Send;
    fn download(
        &self,
        url: &str,
        dest: &Path,
    ) -> impl Future<Output = Result<(), ScryfallError>> + Send;
    /// Oracle ids of every card matching a Scryfall search query.
    fn search_oracle_ids(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<BTreeSet<String>, ScryfallError>> + Send;
}

pub struct ScryfallClient {
    http: reqwest::Client,
}

impl ScryfallClient {
    pub fn new(user_agent: &str) -> Result<Self, ScryfallError> {
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { http })
    }
}

#[derive(Deserialize)]
struct Page<T> {
    data: Vec<T>,
    next_page: Option<String>,
}

#[derive(Deserialize)]
struct SearchCard {
    oracle_id: Option<String>,
}

impl CardSource for ScryfallClient {
    async fn bulk_index(&self) -> Result<Vec<BulkFile>, ScryfallError> {
        let page: Page<BulkFile> = self
            .http
            .get(format!("{API}/bulk-data"))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(page.data)
    }

    async fn download(&self, url: &str, dest: &Path) -> Result<(), ScryfallError> {
        let mut response = self.http.get(url).send().await?.error_for_status()?;
        let mut file = tokio::fs::File::create(dest).await?;
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(())
    }

    async fn search_oracle_ids(&self, query: &str) -> Result<BTreeSet<String>, ScryfallError> {
        let mut ids = BTreeSet::new();
        let url = reqwest::Url::parse_with_params(
            &format!("{API}/cards/search"),
            &[("q", query), ("unique", "cards")],
        )
        .expect("static base URL is valid");
        let mut request = self.http.get(url);
        loop {
            let page: Page<SearchCard> = request
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            ids.extend(page.data.into_iter().filter_map(|card| card.oracle_id));
            let Some(next) = page.next_page else {
                return Ok(ids);
            };
            tokio::time::sleep(API_DELAY).await;
            request = self.http.get(next);
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct BulkSync {
    pub downloaded: Vec<&'static str>,
    pub unchanged: Vec<&'static str>,
}

/// Downloads each bulk file whose `updated_at` differs from the last download.
/// A file is renamed into place only once complete, so an interrupted sync
/// never leaves a truncated bulk behind.
pub async fn sync_bulks(
    source: &impl CardSource,
    raw: &RawLayout,
) -> Result<BulkSync, ScryfallError> {
    let dir = raw.scryfall_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let manifest_path = dir.join(MANIFEST_FILE);
    let mut manifest = read_manifest(&manifest_path).await?;
    let index = source.bulk_index().await?;

    let mut outcome = BulkSync::default();
    for bulk in SCRYFALL_BULKS {
        let entry = index
            .iter()
            .find(|entry| entry.kind == bulk)
            .ok_or(ScryfallError::MissingBulk(bulk))?;
        let dest = raw.scryfall_file(bulk);
        if manifest.get(bulk) == Some(&entry.updated_at) && dest.is_file() {
            outcome.unchanged.push(bulk);
            continue;
        }
        let partial = dest.with_extension("gz.part");
        tracing::info!(bulk, updated_at = %entry.updated_at, "downloading");
        source.download(&entry.jsonl_download_uri, &partial).await?;
        tokio::fs::rename(&partial, &dest).await?;
        manifest.insert(bulk.to_owned(), entry.updated_at.clone());
        write_manifest(&manifest_path, &manifest).await?;
        outcome.downloaded.push(bulk);
    }
    Ok(outcome)
}

/// Bulk versions that the raw files on disk come from.
pub async fn bulk_versions(raw: &RawLayout) -> Result<BTreeMap<String, String>, ScryfallError> {
    read_manifest(&raw.scryfall_dir().join(MANIFEST_FILE)).await
}

async fn read_manifest(path: &Path) -> Result<BTreeMap<String, String>, ScryfallError> {
    match tokio::fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|source| ScryfallError::Manifest {
            path: path.to_owned(),
            source,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.into()),
    }
}

async fn write_manifest(
    path: &Path,
    manifest: &BTreeMap<String, String>,
) -> Result<(), ScryfallError> {
    let json = serde_json::to_vec_pretty(manifest).map_err(|source| ScryfallError::Manifest {
        path: path.to_owned(),
        source,
    })?;
    tokio::fs::write(path, json).await?;
    Ok(())
}
