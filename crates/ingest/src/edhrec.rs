use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::catalog::RawLayout;

const BASE_URL: &str = "https://json.edhrec.com/pages/";
const REQUEST_DELAY: Duration = Duration::from_secs(1);
const CACHE_SECONDS: u64 = 24 * 3600;
const MANIFEST_FILE: &str = "manifest.json";
/// Guards against a `more` link that loops back.
const MAX_PAGES: usize = 500;

pub const WINDOWS: [&str; 3] = ["week", "month", "year"];

#[derive(Debug, thiserror::Error)]
pub enum EdhrecError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("EDHREC page {path} is not valid JSON: {source}")]
    Json {
        path: String,
        source: serde_json::Error,
    },
    #[error("EDHREC page {0} has no cardviews list, the format may have changed")]
    UnknownShape(String),
    #[error("EDHREC window {0} has more than {MAX_PAGES} pages")]
    TooManyPages(&'static str),
}

pub trait PopularitySource: Sync {
    /// `path` is relative to the EDHREC JSON pages root, like `commanders/week.json`.
    fn fetch(&self, path: &str) -> impl Future<Output = Result<Vec<u8>, EdhrecError>> + Send;
}

pub struct EdhrecClient {
    http: reqwest::Client,
}

impl EdhrecClient {
    pub fn new(user_agent: &str) -> Result<Self, EdhrecError> {
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { http })
    }
}

impl PopularitySource for EdhrecClient {
    async fn fetch(&self, path: &str) -> Result<Vec<u8>, EdhrecError> {
        let bytes = self
            .http
            .get(format!("{BASE_URL}{path}"))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        Ok(bytes.to_vec())
    }
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct EdhrecSync {
    pub fetched: BTreeMap<&'static str, usize>,
    pub cached: Vec<&'static str>,
}

/// Fetches every page of each commander ranking window, following the `more`
/// links, at most one request per second. A window fetched less than 24 h ago
/// is kept. A window is replaced only once all its pages are downloaded.
pub async fn sync_commander_pages(
    source: &impl PopularitySource,
    raw: &RawLayout,
    now_unix: u64,
) -> Result<EdhrecSync, EdhrecError> {
    let dir = raw.edhrec_commanders_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let manifest_path = dir.join(MANIFEST_FILE);
    let mut manifest = read_manifest(&manifest_path).await?;

    let mut outcome = EdhrecSync::default();
    let mut first_request = true;
    for window in WINDOWS {
        let window_dir = dir.join(window);
        let fresh = manifest
            .get(window)
            .is_some_and(|fetched| now_unix.saturating_sub(*fetched) < CACHE_SECONDS);
        if fresh && window_dir.is_dir() {
            outcome.cached.push(window);
            continue;
        }

        let partial = dir.join(format!("{window}.part"));
        remove_dir_if_exists(&partial).await?;
        tokio::fs::create_dir_all(&partial).await?;
        let mut next = Some(format!("commanders/{window}.json"));
        let mut pages = 0;
        while let Some(path) = next {
            if pages == MAX_PAGES {
                return Err(EdhrecError::TooManyPages(window));
            }
            if !first_request {
                tokio::time::sleep(REQUEST_DELAY).await;
            }
            first_request = false;
            tracing::debug!(path, "fetching EDHREC page");
            let body = source.fetch(&path).await?;
            next = next_page(&path, &body)?;
            pages += 1;
            tokio::fs::write(partial.join(format!("{pages:03}.json")), body).await?;
        }

        remove_dir_if_exists(&window_dir).await?;
        tokio::fs::rename(&partial, &window_dir).await?;
        manifest.insert(window.to_owned(), now_unix);
        write_manifest(&manifest_path, &manifest).await?;
        outcome.fetched.insert(window, pages);
    }
    Ok(outcome)
}

/// Page 1 nests the list under `container.json_dict.cardlists[0]`, later pages
/// put it at the top level. `more` is relative to the pages root.
fn next_page(path: &str, body: &[u8]) -> Result<Option<String>, EdhrecError> {
    let page: Value = serde_json::from_slice(body).map_err(|source| EdhrecError::Json {
        path: path.to_owned(),
        source,
    })?;
    let list = if page.get("cardviews").is_some() {
        &page
    } else {
        &page["container"]["json_dict"]["cardlists"][0]
    };
    if !list["cardviews"].is_array() {
        return Err(EdhrecError::UnknownShape(path.to_owned()));
    }
    Ok(list["more"].as_str().map(str::to_owned))
}

async fn remove_dir_if_exists(path: &Path) -> std::io::Result<()> {
    match tokio::fs::remove_dir_all(path).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

async fn read_manifest(path: &PathBuf) -> Result<BTreeMap<String, u64>, EdhrecError> {
    match tokio::fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|source| EdhrecError::Json {
            path: path.display().to_string(),
            source,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.into()),
    }
}

async fn write_manifest(path: &Path, manifest: &BTreeMap<String, u64>) -> Result<(), EdhrecError> {
    let json = serde_json::to_vec_pretty(manifest).map_err(|source| EdhrecError::Json {
        path: path.display().to_string(),
        source,
    })?;
    tokio::fs::write(path, json).await?;
    Ok(())
}
