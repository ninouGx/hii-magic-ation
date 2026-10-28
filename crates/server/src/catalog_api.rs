use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use hma_core::db::{CATALOG_SCHEMA, Db};
use hma_core::duckdb::Connection;
use hma_ingest::catalog::CatalogCounts;
use hma_ingest::scryfall;
use hma_ingest::sync::SyncPlan;
use serde::Serialize;

use crate::app::{ApiError, AppState};
use crate::sync_job;

const UNRESOLVED_LIMIT: usize = 200;

#[derive(Serialize)]
pub struct CatalogStatus {
    ready: bool,
    counts: Option<CatalogCounts>,
    /// Share of EDHREC entries mapped to a card, `None` before the first EDHREC sync.
    edhrec_mapping_rate: Option<f64>,
    edhrec_unresolved: Vec<UnresolvedEntry>,
    bulk_versions: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct UnresolvedEntry {
    period: String,
    rank: Option<i32>,
    name: Option<String>,
}

pub async fn catalog(State(state): State<AppState>) -> Result<Json<CatalogStatus>, ApiError> {
    let bulk_versions = scryfall::bulk_versions(&sync_job::raw_layout(state.db.data_dir()))
        .await
        .map_err(ApiError::internal)?;
    let db = state.db.clone();
    let status = tokio::task::spawn_blocking(move || read_catalog_status(&db, bulk_versions))
        .await
        .map_err(ApiError::internal)?
        .map_err(ApiError::internal)?;
    Ok(Json(status))
}

fn read_catalog_status(
    db: &Db,
    bulk_versions: BTreeMap<String, String>,
) -> anyhow::Result<CatalogStatus> {
    if !db.catalog_ready() {
        return Ok(CatalogStatus {
            ready: false,
            counts: None,
            edhrec_mapping_rate: None,
            edhrec_unresolved: Vec::new(),
            bulk_versions,
        });
    }
    let conn = db.connection()?;
    let counts = CatalogCounts::read(&conn, CATALOG_SCHEMA)?;
    Ok(CatalogStatus {
        ready: true,
        edhrec_mapping_rate: edhrec_mapping_rate(&conn, counts.edhrec_unresolved)?,
        edhrec_unresolved: unresolved_entries(&conn)?,
        counts: Some(counts),
        bulk_versions,
    })
}

/// A partner pair is one EDHREC entry stored as two rank rows.
fn edhrec_mapping_rate(conn: &Connection, unresolved: i64) -> anyhow::Result<Option<f64>> {
    let resolved: i64 = conn.query_row(
        &format!(
            "SELECT count(*) FILTER (WHERE partner_oracle_id = '')
                  + count(*) FILTER (WHERE partner_oracle_id <> '') // 2
             FROM {CATALOG_SCHEMA}.edhrec_commander_ranks"
        ),
        [],
        |row| row.get(0),
    )?;
    let total = resolved + unresolved;
    #[allow(
        clippy::cast_precision_loss,
        reason = "entry counts stay far below 2^52"
    )]
    Ok((total > 0).then(|| resolved as f64 / total as f64))
}

fn unresolved_entries(conn: &Connection) -> anyhow::Result<Vec<UnresolvedEntry>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT period, rank, name FROM {CATALOG_SCHEMA}.edhrec_unresolved
         ORDER BY period, rank LIMIT {UNRESOLVED_LIMIT}"
    ))?;
    let entries = stmt
        .query_map([], |row| {
            Ok(UnresolvedEntry {
                period: row.get(0)?,
                rank: row.get(1)?,
                name: row.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(entries)
}

#[derive(Serialize)]
pub struct Reloaded {
    swapped: bool,
}

pub async fn reload(State(state): State<AppState>) -> Result<Json<Reloaded>, ApiError> {
    let db = state.db.clone();
    let swapped = tokio::task::spawn_blocking(move || db.swap_catalog())
        .await
        .map_err(ApiError::internal)?
        .map_err(|e| ApiError::new(StatusCode::CONFLICT, e))?;
    Ok(Json(Reloaded { swapped }))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncState {
    #[default]
    Idle,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SyncStatus {
    pub state: SyncState,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub report: Option<serde_json::Value>,
    pub error: Option<String>,
}

pub async fn sync_status(State(state): State<AppState>) -> Json<SyncStatus> {
    Json(state.sync.lock().expect("sync status lock").clone())
}

pub async fn start_sync(
    State(state): State<AppState>,
    Json(plan): Json<SyncPlan>,
) -> Result<(StatusCode, Json<SyncStatus>), ApiError> {
    let status = {
        let mut status = state.sync.lock().expect("sync status lock");
        if status.state == SyncState::Running {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "a sync is already running",
            ));
        }
        *status = SyncStatus {
            state: SyncState::Running,
            started_at: Some(unix_now()),
            ..SyncStatus::default()
        };
        status.clone()
    };
    tokio::spawn(run_sync(state, plan));
    Ok((StatusCode::ACCEPTED, Json(status)))
}

async fn run_sync(state: AppState, plan: SyncPlan) {
    let result = sync_and_swap(&state.db, plan).await;
    let mut status = state.sync.lock().expect("sync status lock");
    status.finished_at = Some(unix_now());
    match result {
        Ok(report) => {
            status.state = SyncState::Done;
            status.report = Some(report);
        }
        Err(e) => {
            tracing::error!(error = %e, "sync failed");
            status.state = SyncState::Failed;
            status.error = Some(format!("{e:#}"));
        }
    }
}

async fn sync_and_swap(db: &Db, plan: SyncPlan) -> anyhow::Result<serde_json::Value> {
    let reader = db.clone();
    let previous = tokio::task::spawn_blocking(move || {
        reader
            .catalog_ready()
            .then(|| CatalogCounts::read(&reader.connection().ok()?, CATALOG_SCHEMA).ok())
            .flatten()
    })
    .await?;
    let report = sync_job::run(db.data_dir(), previous, plan).await?;
    if report.catalog.is_some() {
        let db = db.clone();
        tokio::task::spawn_blocking(move || db.swap_catalog()).await??;
    }
    Ok(serde_json::to_value(report)?)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::router;
    use axum::body::Body;
    use axum::http::Request;
    use hma_core::db::CATALOG_NEW_FILE;
    use hma_ingest::catalog::{self, RawLayout};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const INGEST_FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ingest/tests/fixtures");

    async fn send(state: &AppState, request: Request<Body>) -> (StatusCode, serde_json::Value) {
        let response = router(state.clone()).oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, json)
    }

    fn get(path: &str) -> Request<Body> {
        Request::get(path).body(Body::empty()).unwrap()
    }

    fn post(path: &str, body: &str) -> Request<Body> {
        Request::post(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap()
    }

    #[tokio::test]
    async fn catalog_is_not_ready_before_the_first_sync() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Db::open(dir.path()).unwrap());

        let (status, body) = send(&state, get("/api/catalog")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ready"], false);
        assert!(body["counts"].is_null());
    }

    #[tokio::test]
    async fn reload_swaps_in_the_pending_catalog_and_reports_counts() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Db::open(dir.path()).unwrap());
        catalog::build(
            &RawLayout::new(INGEST_FIXTURES),
            &dir.path().join(CATALOG_NEW_FILE),
        )
        .unwrap();

        let (status, reloaded) = send(&state, post("/api/catalog/reload", "")).await;
        let (_, catalog) = send(&state, get("/api/catalog")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(reloaded["swapped"], true);
        assert_eq!(catalog["ready"], true);
        assert_eq!(catalog["counts"]["cards"], 20);
        assert_eq!(catalog["counts"]["commanders"], 10);
    }

    #[tokio::test]
    async fn second_sync_is_refused_while_one_runs() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Db::open(dir.path()).unwrap());
        state.sync.lock().unwrap().state = SyncState::Running;

        let (status, _) = send(
            &state,
            post(
                "/api/sync",
                r#"{"scryfall":true,"edhrec":true,"rebuild":false}"#,
            ),
        )
        .await;
        let (_, current) = send(&state, get("/api/sync")).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(current["state"], "running");
    }
}
