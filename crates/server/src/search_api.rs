use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use hma_core::card::{self, CardDetail};
use hma_core::duckdb::Connection;
use hma_core::filter::FilterError;
use hma_core::search::{self, SearchPage, SearchRequest, TagSuggestion};
use serde::Deserialize;

use crate::app::{ApiError, AppState};

const SUGGESTION_LIMIT: u32 = 20;

pub async fn search(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchPage>, ApiError> {
    on_catalog(state, move |conn| {
        search::search(conn, &request).map_err(|error| match error {
            FilterError::InvalidRegex { .. } => ApiError::new(StatusCode::BAD_REQUEST, error),
            FilterError::Db(error) => ApiError::internal(error),
        })
    })
    .await
}

pub async fn card(
    State(state): State<AppState>,
    Path(oracle_id): Path<String>,
) -> Result<Json<CardDetail>, ApiError> {
    on_catalog(state, move |conn| {
        card::card_detail(conn, &oracle_id)
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "no card with this oracle_id"))
    })
    .await
}

#[derive(Deserialize)]
pub struct Suggest {
    #[serde(default)]
    q: String,
    limit: Option<u32>,
}

pub async fn tags(
    State(state): State<AppState>,
    Query(query): Query<Suggest>,
) -> Result<Json<Vec<TagSuggestion>>, ApiError> {
    on_catalog(state, move |conn| {
        search::suggest_tags(conn, &query.q, query.limit.unwrap_or(SUGGESTION_LIMIT))
            .map_err(ApiError::internal)
    })
    .await
}

pub async fn keywords(
    State(state): State<AppState>,
    Query(query): Query<Suggest>,
) -> Result<Json<Vec<String>>, ApiError> {
    on_catalog(state, move |conn| {
        search::suggest_keywords(conn, &query.q, query.limit.unwrap_or(SUGGESTION_LIMIT))
            .map_err(ApiError::internal)
    })
    .await
}

pub async fn formats(State(state): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    on_catalog(state, |conn| {
        search::formats(conn).map_err(ApiError::internal)
    })
    .await
}

/// Runs a catalog query off the async runtime. Before the first sync there is
/// no catalog to query, which the client can fix, so it is not a server error.
pub(crate) async fn on_catalog<T, F>(state: AppState, query: F) -> Result<Json<T>, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, ApiError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        if !state.db.catalog_ready() {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "no catalog yet, run a sync first",
            ));
        }
        let conn = state.db.connection().map_err(ApiError::internal)?;
        query(&conn)
    })
    .await
    .map_err(ApiError::internal)?
    .map(Json)
}

#[cfg(test)]
mod tests {
    use crate::app::{AppState, router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use hma_core::db::{CATALOG_NEW_FILE, Db};
    use hma_ingest::catalog::{self, RawLayout};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    const INGEST_FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../ingest/tests/fixtures");

    fn state_with_catalog(dir: &std::path::Path) -> AppState {
        catalog::build(
            &RawLayout::new(INGEST_FIXTURES),
            &dir.join(CATALOG_NEW_FILE),
        )
        .unwrap();
        AppState::new(Db::open(dir).unwrap())
    }

    async fn send(state: &AppState, request: Request<Body>) -> (StatusCode, Value) {
        let response = router(state.clone()).oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, body)
    }

    fn search(body: &Value) -> Request<Body> {
        Request::post("/api/cards/search")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn get(path: &str) -> Request<Body> {
        Request::get(path).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn search_returns_the_total_and_one_page() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_catalog(dir.path());

        let (status, page) = send(
            &state,
            search(&json!({
                "filter": { "combinator": "and", "rules": [
                    { "field": "name", "operator": "contains", "value": "dragon" }
                ]},
                "sort": [{ "field": "cmc", "direction": "desc" }],
                "limit": 1
            })),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["total"], 2);
        assert_eq!(page["cards"][0]["name"], "Dragonlord Atarka");
    }

    #[tokio::test]
    async fn invalid_regex_is_a_bad_request_with_the_re2_message() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_catalog(dir.path());

        let (status, body) = send(
            &state,
            search(&json!({
                "filter": { "combinator": "and", "rules": [
                    { "field": "name", "operator": "regex", "value": "(?=x)" }
                ]}
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.as_str().unwrap().contains("invalid perl operator"));
    }

    #[tokio::test]
    async fn unknown_field_is_rejected_with_its_name() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_catalog(dir.path());

        let (status, body) = send(
            &state,
            search(&json!({
                "filter": { "combinator": "and", "rules": [
                    { "field": "mana", "operator": "=", "value": 1 }
                ]}
            })),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body.as_str().unwrap().contains("mana"));
    }

    #[tokio::test]
    async fn search_before_the_first_sync_is_a_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(Db::open(dir.path()).unwrap());

        let (status, _) = send(
            &state,
            search(&json!({ "filter": { "combinator": "and", "rules": [] } })),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn card_detail_is_found_by_oracle_id() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_catalog(dir.path());
        let (_, page) = send(
            &state,
            search(&json!({
                "filter": { "combinator": "and", "rules": [
                    { "field": "name", "operator": "contains", "value": "delver" }
                ]}
            })),
        )
        .await;
        let oracle_id = page["cards"][0]["oracle_id"].as_str().unwrap();

        let (status, card) = send(&state, get(&format!("/api/cards/{oracle_id}"))).await;
        let (missing, _) = send(&state, get("/api/cards/nope")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(card["faces"][1]["name"], "Insectile Aberration");
        assert_eq!(missing, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn suggestions_and_formats_feed_the_filter_builder() {
        let dir = tempfile::tempdir().unwrap();
        let state = state_with_catalog(dir.path());

        let (_, tags) = send(&state, get("/api/tags?q=ramp&limit=3")).await;
        let (_, keywords) = send(&state, get("/api/keywords?q=fly")).await;
        let (_, formats) = send(&state, get("/api/formats")).await;

        assert_eq!(tags[0]["slug"], "ramp");
        assert!(tags.as_array().unwrap().len() <= 3);
        assert_eq!(keywords, json!(["Flying"]));
        assert!(formats.as_array().unwrap().contains(&json!("commander")));
    }
}
