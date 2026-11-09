//! HTTP routes for wundspray.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use url_sanitize_core::Sanitizer;

use crate::map::{map_result, Action};
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
    let started = Instant::now();

    let Json(req) = match payload {
        Ok(json) => json,
        Err(err) => {
            info!(
                method = "POST",
                path = "/sanitize",
                status = 400,
                error = "invalid_json",
                latency_ms = started.elapsed().as_millis() as u64,
                "sanitize rejected"
            );
            return error_response(StatusCode::BAD_REQUEST, "invalid_json", err.body_text());
        }
    };

    let url = req.url.trim();
    if url.is_empty() {
        info!(
            method = "POST",
            path = "/sanitize",
            status = 400,
            error = "invalid_url",
            latency_ms = started.elapsed().as_millis() as u64,
            "sanitize rejected"
        );
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_url",
            "url must be a non-empty string".into(),
        );
    }

    let mapped = map_result(url, sanitize(&state.sanitizer, url));
    let response = &mapped.response;
    let latency_ms = started.elapsed().as_millis() as u64;

    info!(
        method = "POST",
        path = "/sanitize",
        status = 200,
        action = action_str(response.action),
        changed = response.changed,
        blocked = response.blocked,
        latency_ms,
        "sanitize"
    );

    if tracing::enabled!(tracing::Level::DEBUG) {
        debug!(
            input = %response.input,
            output = ?response.url,
            stripped_params = ?response.stripped_params,
            matched_rules = ?mapped.details.matched_rules,
            via = ?mapped.details.via,
            "sanitize details"
        );
    }

    (StatusCode::OK, Json(mapped.response)).into_response()
}

fn action_str(action: Action) -> &'static str {
    match action {
        Action::Unchanged => "unchanged",
        Action::Cleaned => "cleaned",
        Action::Redirected => "redirected",
        Action::Blocked => "blocked",
    }
}

fn error_response(status: StatusCode, error: &'static str, message: String) -> Response {
    (status, Json(ErrorBody { error, message })).into_response()
}
