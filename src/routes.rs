//! HTTP routes for wundspray.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use url_sanitize_core::Sanitizer;

use crate::map::map_result;
use crate::sanitize::sanitize;

#[derive(Clone)]
pub struct AppState {
    pub sanitizer: Arc<Sanitizer>,
}

#[derive(Debug, Deserialize)]
pub struct SanitizeRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: &'static str,
    pub message: String,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/sanitize", post(sanitize_handler))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn sanitize_handler(
    State(state): State<AppState>,
    payload: Result<Json<SanitizeRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Json(req) = match payload {
        Ok(json) => json,
        Err(err) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_json",
                err.body_text(),
            );
        }
    };

    let url = req.url.trim();
    if url.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_url",
            "url must be a non-empty string".into(),
        );
    }

    let mapped = map_result(url, sanitize(&state.sanitizer, url));
    (StatusCode::OK, Json(mapped.response)).into_response()
}

fn error_response(status: StatusCode, error: &'static str, message: String) -> Response {
    (status, Json(ErrorBody { error, message })).into_response()
}
