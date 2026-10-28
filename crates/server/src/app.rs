use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use hma_core::db::{Db, DbInfo};
use rust_embed::Embed;
use serde::Serialize;

#[derive(Embed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Front;

const INDEX_HTML: &str = "index.html";

pub fn router(db: Db) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .fallback(api_not_found);
    Router::new()
        .nest("/api", api)
        .fallback(front)
        .with_state(db)
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    #[serde(flatten)]
    db: DbInfo,
}

async fn health(State(db): State<Db>) -> Result<Json<Health>, ApiError> {
    let info = tokio::task::spawn_blocking(move || db.info())
        .await
        .map_err(|e| ApiError(e.to_string()))?
        .map_err(|e| ApiError(e.to_string()))?;
    Ok(Json(Health {
        status: "ok",
        db: info,
    }))
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

/// Unknown paths get `index.html` so that client-side routes survive a reload.
async fn front(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (path, file) = match Front::get(path) {
        Some(file) if !path.is_empty() => (path, file),
        _ => match Front::get(INDEX_HTML) {
            Some(file) => (INDEX_HTML, file),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    "front not built, run `npm run build` in web/",
                )
                    .into_response();
            }
        },
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response()
}

struct ApiError(String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!(error = %self.0, "request failed");
        (StatusCode::INTERNAL_SERVER_ERROR, self.0).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get(path: &str) -> (StatusCode, Option<String>, Vec<u8>) {
        let dir = tempfile::tempdir().unwrap();
        let app = router(Db::open(dir.path()).unwrap());
        let response = app
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_owned());
        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (status, content_type, body)
    }

    #[tokio::test]
    async fn health_reports_both_databases() {
        let (status, _, body) = get("/api/health").await;
        let health: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(health["status"], "ok");
        assert_eq!(health["databases"], serde_json::json!(["catalog", "user"]));
    }

    #[tokio::test]
    async fn unknown_api_route_is_not_found() {
        let (status, _, _) = get("/api/nope").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn client_route_serves_index_html() {
        let (status, content_type, _) = get("/explorer").await;

        assert_eq!(
            status,
            StatusCode::OK,
            "build the front first: npm run build in web/"
        );
        assert_eq!(content_type.as_deref(), Some("text/html"));
    }
}
