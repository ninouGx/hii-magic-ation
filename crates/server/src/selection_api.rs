use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use hma_core::export::{self, Export};
use hma_core::filter::{FilterError, Group};
use hma_core::selection::{
    self, CardChange, Selection, SelectionError, SelectionMeta, SelectionSummary,
};
use serde::{Deserialize, Serialize};

use crate::app::{ApiError, AppState};
use crate::search_api::on_catalog;

impl From<SelectionError> for ApiError {
    fn from(error: SelectionError) -> Self {
        let status = match &error {
            SelectionError::NotFound => StatusCode::NOT_FOUND,
            SelectionError::UnknownCard(_)
            | SelectionError::ForeignPrint { .. }
            | SelectionError::PrintWithoutCard(_)
            | SelectionError::EmptyQuantity
            | SelectionError::EmptyName
            | SelectionError::PartnerIsCommander
            | SelectionError::Filter(FilterError::InvalidRegex { .. }) => StatusCode::BAD_REQUEST,
            SelectionError::Filter(FilterError::Db(_)) | SelectionError::Db(_) => {
                return ApiError::internal(error);
            }
        };
        ApiError::new(status, error)
    }
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<SelectionSummary>>, ApiError> {
    on_catalog(state, |conn| Ok(selection::list(conn)?)).await
}

#[derive(Deserialize)]
pub struct NewSelection {
    name: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(new): Json<NewSelection>,
) -> Result<(StatusCode, Json<Selection>), ApiError> {
    let created = on_catalog(state, move |conn| Ok(selection::create(conn, &new.name)?)).await?;
    Ok((StatusCode::CREATED, created))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Selection>, ApiError> {
    on_catalog(state, move |conn| Ok(selection::get(conn, &id)?)).await
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(meta): Json<SelectionMeta>,
) -> Result<Json<Selection>, ApiError> {
    on_catalog(state, move |conn| Ok(selection::update(conn, &id, &meta)?)).await
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let Json(()) = on_catalog(state, move |conn| Ok(selection::delete(conn, &id)?)).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_card(
    State(state): State<AppState>,
    Path((id, oracle_id)): Path<(String, String)>,
) -> Result<Json<Selection>, ApiError> {
    on_catalog(state, move |conn| {
        Ok(selection::add_card(conn, &id, &oracle_id)?)
    })
    .await
}

pub async fn set_card(
    State(state): State<AppState>,
    Path((id, oracle_id)): Path<(String, String)>,
    Json(change): Json<CardChange>,
) -> Result<Json<Selection>, ApiError> {
    on_catalog(state, move |conn| {
        Ok(selection::set_card(conn, &id, &oracle_id, &change)?)
    })
    .await
}

pub async fn remove_card(
    State(state): State<AppState>,
    Path((id, oracle_id)): Path<(String, String)>,
) -> Result<Json<Selection>, ApiError> {
    on_catalog(state, move |conn| {
        Ok(selection::remove_card(conn, &id, &oracle_id)?)
    })
    .await
}

pub async fn export_moxfield(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Export>, ApiError> {
    on_catalog(state, move |conn| Ok(export::moxfield(conn, &id)?)).await
}

#[derive(Deserialize)]
pub struct AddFiltered {
    filter: Group,
}

#[derive(Serialize)]
pub struct Added {
    added: usize,
}

pub async fn add_filtered(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<AddFiltered>,
) -> Result<Json<Added>, ApiError> {
    on_catalog(state, move |conn| {
        Ok(Added {
            added: selection::add_filtered(conn, &id, &request.filter)?,
        })
    })
    .await
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

    async fn send(
        state: &AppState,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
            .unwrap();
        let response = router(state.clone()).oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, body)
    }

    async fn oracle_id(state: &AppState, name: &str) -> String {
        let (_, page) = send(
            state,
            "POST",
            "/api/cards/search",
            Some(json!({ "filter": { "combinator": "and", "rules": [
                { "field": "name", "operator": "contains", "value": name }
            ]}})),
        )
        .await;
        page["cards"][0]["oracle_id"].as_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn selection_round_trip_over_http() {
        let dir = tempfile::tempdir().unwrap();
        catalog::build(
            &RawLayout::new(INGEST_FIXTURES),
            &dir.path().join(CATALOG_NEW_FILE),
        )
        .unwrap();
        let state = AppState::new(Db::open(dir.path()).unwrap());
        let atarka = oracle_id(&state, "Dragonlord Atarka").await;
        let delver = oracle_id(&state, "Delver of Secrets").await;

        let (created, deck) = send(
            &state,
            "POST",
            "/api/selections",
            Some(json!({ "name": "Dragons" })),
        )
        .await;
        let id = deck["selection_id"].as_str().unwrap().to_owned();
        let (updated, _) = send(
            &state,
            "PUT",
            &format!("/api/selections/{id}"),
            Some(json!({ "name": "Dragons", "commander_oracle_id": atarka, "partner_oracle_id": null })),
        )
        .await;
        let (_, deck) = send(
            &state,
            "POST",
            &format!("/api/selections/{id}/cards/{delver}"),
            None,
        )
        .await;
        let (_, added) = send(
            &state,
            "POST",
            &format!("/api/selections/{id}/cards"),
            Some(json!({ "filter": { "combinator": "and", "rules": [
                { "field": "name", "operator": "contains", "value": "dragon" }
            ]}})),
        )
        .await;
        let (bad_quantity, _) = send(
            &state,
            "PUT",
            &format!("/api/selections/{id}/cards/{delver}"),
            Some(json!({ "quantity": 0, "print_id": null, "category": null })),
        )
        .await;
        let (exported, export) =
            send(&state, "GET", &format!("/api/selections/{id}/export"), None).await;
        let (_, list) = send(&state, "GET", "/api/selections", None).await;
        let (deleted, _) = send(&state, "DELETE", &format!("/api/selections/{id}"), None).await;
        let (missing, _) = send(&state, "GET", &format!("/api/selections/{id}"), None).await;
        let (missing_export, _) =
            send(&state, "GET", &format!("/api/selections/{id}/export"), None).await;

        assert_eq!((created, updated), (StatusCode::CREATED, StatusCode::OK));
        assert_eq!(deck["count"], 2);
        assert_eq!(deck["alerts"][0]["kind"], "outside_identity");
        assert_eq!(added["added"], 2);
        assert_eq!(bad_quantity, StatusCode::BAD_REQUEST);
        assert_eq!(exported, StatusCode::OK);
        assert!(
            export["commanders"][0]
                .as_str()
                .unwrap()
                .starts_with("1 Dragonlord Atarka (")
        );
        assert!(!export["text"].as_str().unwrap().contains("Atarka"));
        assert_eq!(missing_export, StatusCode::NOT_FOUND);
        assert_eq!(list[0]["commander_name"], "Dragonlord Atarka");
        assert_eq!(
            (deleted, missing),
            (StatusCode::NO_CONTENT, StatusCode::NOT_FOUND)
        );
    }
}
