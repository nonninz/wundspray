//! HTTP routes for wundspray.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{ConnectInfo, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use url_sanitize_core::Sanitizer;
use uuid::Uuid;

use crate::client_ip::{parse_trusted_proxies, resolve_client_ip, ClientIpResolution};
use crate::map::{map_result, Action};
use crate::sanitize::sanitize;

#[derive(Clone)]
pub struct AppState {
    pub sanitizer: Arc<Sanitizer>,
    pub trusted_proxies: Arc<[IpNet]>,
}

impl AppState {
    /// Build state with `TRUSTED_PROXIES` loaded from the environment.
    pub fn new(sanitizer: Arc<Sanitizer>) -> Self {
        Self {
            sanitizer,
            trusted_proxies: Arc::from(load_trusted_proxies_from_env()),
        }
    }
}

fn load_trusted_proxies_from_env() -> Vec<IpNet> {
    match std::env::var("TRUSTED_PROXIES") {
        Ok(raw) => match parse_trusted_proxies(&raw) {
            Ok(nets) => nets,
            Err(err) => {
                warn!(error = %err, "invalid TRUSTED_PROXIES; treating as empty");
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
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
        .route("/clean", post(clean_handler))
        .route("/sanitize", post(sanitize_deprecated))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Deprecated: 308 Permanent Redirect to `/clean` (preserves method/body).
async fn sanitize_deprecated() -> Response {
    warn!(
        method = "POST",
        path = "/sanitize",
        status = 308,
        location = "/clean",
        "deprecated /sanitize used; use POST /clean"
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::LOCATION, HeaderValue::from_static("/clean"));
    headers.insert("deprecation", HeaderValue::from_static("true"));
    headers.insert(
        header::WARNING,
        HeaderValue::from_static(
            "299 - \"POST /sanitize is deprecated; use POST /clean. Will be removed in a future release.\"",
        ),
    );

    (StatusCode::PERMANENT_REDIRECT, headers).into_response()
}

async fn clean_handler(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    payload: Result<Json<SanitizeRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let started = Instant::now();
    let request_id = resolve_request_id(&headers);
    let resolved = resolve_client_ip(peer, &headers, &state.trusted_proxies);
    let client_ip = resolved.client_ip;
    log_clean_peer(peer, &resolved, &headers);

    let Json(req) = match payload {
        Ok(json) => json,
        Err(err) => {
            info!(
                method = "POST",
                path = "/clean",
                status = 400,
                error = "invalid_json",
                %client_ip,
                request_id = %request_id,
                latency_ms = started.elapsed().as_millis() as u64,
                "clean rejected"
            );
            return with_request_id(
                error_response(StatusCode::BAD_REQUEST, "invalid_json", err.body_text()),
                &request_id,
            );
        }
    };

    let url = req.url.trim();
    if url.is_empty() {
        info!(
            method = "POST",
            path = "/clean",
            status = 400,
            error = "invalid_url",
            %client_ip,
            request_id = %request_id,
            latency_ms = started.elapsed().as_millis() as u64,
            "clean rejected"
        );
        return with_request_id(
            error_response(
                StatusCode::BAD_REQUEST,
                "invalid_url",
                "url must be a non-empty string".into(),
            ),
            &request_id,
        );
    }

    let mapped = map_result(url, sanitize(&state.sanitizer, url));
    let response = &mapped.response;
    let latency_ms = started.elapsed().as_millis() as u64;

    info!(
        method = "POST",
        path = "/clean",
        status = 200,
        action = action_str(response.action),
        changed = response.changed,
        blocked = response.blocked,
        %client_ip,
        request_id = %request_id,
        latency_ms,
        "clean"
    );

    if tracing::enabled!(tracing::Level::DEBUG) {
        debug!(
            input = %response.input,
            output = ?response.url,
            stripped_params = ?response.stripped_params,
            matched_rules = ?mapped.details.matched_rules,
            via = ?mapped.details.via,
            "clean details"
        );
    }

    with_request_id(
        (StatusCode::OK, Json(mapped.response)).into_response(),
        &request_id,
    )
}

fn log_clean_peer(peer: SocketAddr, resolved: &ClientIpResolution, headers: &HeaderMap) {
    let forwarded = header_str(headers, "forwarded");
    let x_forwarded_for = header_str(headers, "x-forwarded-for");
    let x_real_ip = header_str(headers, "x-real-ip");
    debug!(
        peer_addr = %peer,
        peer_trusted = resolved.peer_trusted,
        ip_source = resolved.source.as_str(),
        forwarded,
        x_forwarded_for,
        x_real_ip,
        "clean peer"
    );
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn resolve_request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn with_request_id(mut response: Response, request_id: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(request_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-request-id"), value);
    }
    response
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
