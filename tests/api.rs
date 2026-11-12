use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ipnet::IpNet;
use serde_json::{json, Value};
use tower::ServiceExt;
use wundspray::client_ip::parse_trusted_proxies;
use wundspray::routes::{app, AppState};
use wundspray::sanitize::shared_sanitizer;

fn test_app() -> axum::Router {
    test_app_with_proxies(&[])
}

fn test_app_with_proxies(trusted: &[IpNet]) -> axum::Router {
    let sanitizer = shared_sanitizer().expect("catalog");
    app(AppState {
        sanitizer,
        trusted_proxies: Arc::from(trusted.to_vec()),
    })
}

fn peer(addr: &str) -> SocketAddr {
    addr.parse().expect("socket addr")
}

fn clean_request(
    peer_addr: SocketAddr,
    body: String,
    extra_headers: &[(&str, &str)],
) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/clean")
        .header("content-type", "application/json");
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    let mut request = builder.body(Body::from(body)).unwrap();
    request.extensions_mut().insert(ConnectInfo(peer_addr));
    request
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn health_ok() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn clean_strips_tracking() {
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.10:12345"),
            json!({
                "url": "https://example.com/a?id=1&utm_source=x&utm_medium=email"
            })
            .to_string(),
            &[],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["action"], "cleaned");
    assert_eq!(body["changed"], true);
    assert_eq!(body["blocked"], false);
    assert_eq!(body["url"], "https://example.com/a?id=1");
    assert!(body["stripped_params"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == "utm_source"));
    assert!(body.get("matched_rules").is_none());
}

#[tokio::test]
async fn clean_unwraps_redirect() {
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.10:12345"),
            json!({
                "url": "https://www.google.com/url?q=https%3A%2F%2Fexample.org%2Fpage&sa=U"
            })
            .to_string(),
            &[],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["action"], "redirected");
    assert_eq!(body["changed"], true);
    assert!(body["url"]
        .as_str()
        .unwrap()
        .starts_with("https://example.org/"));
}

#[tokio::test]
async fn clean_rejects_empty_url() {
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.10:12345"),
            json!({ "url": "   " }).to_string(),
            &[],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "invalid_url");
}

#[tokio::test]
async fn clean_rejects_invalid_json() {
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.10:12345"),
            "{not-json".into(),
            &[],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "invalid_json");
}

#[tokio::test]
async fn clean_rejects_missing_url_field() {
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.10:12345"),
            json!({ "link": "https://example.com" }).to_string(),
            &[],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "invalid_json");
}

#[tokio::test]
async fn sanitize_redirects_permanently_to_clean() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sanitize")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "url": "https://example.com/?utm_source=x"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok()),
        Some("/clean")
    );
    assert!(
        response
            .headers()
            .get("deprecation")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.eq_ignore_ascii_case("true")),
        "Deprecation header must be present"
    );
}

#[tokio::test]
async fn clean_ignores_spoofed_xff_without_trusted_proxies() {
    // No trusted proxies → ConnectInfo peer wins; spoofed XFF must not break the handler.
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.50:443"),
            json!({ "url": "https://example.com/" }).to_string(),
            &[("x-forwarded-for", "198.51.100.1")],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-request-id").is_some());
    // Resolution itself is covered in client_ip unit tests; assert peer-only path here:
    let resolved = wundspray::client_ip::resolve_client_ip(
        peer("203.0.113.50:443"),
        &{
            let mut h = axum::http::HeaderMap::new();
            h.insert(
                "x-forwarded-for",
                axum::http::HeaderValue::from_static("198.51.100.1"),
            );
            h
        },
        &[],
    );
    assert_eq!(resolved.client_ip, IpAddr::from([203, 0, 113, 50]));
}

#[tokio::test]
async fn clean_trusted_proxy_xff_resolves_client() {
    let trusted = parse_trusted_proxies("10.0.0.0/8").unwrap();
    let peer_addr = peer("10.0.0.5:8080");
    let headers = {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            axum::http::HeaderValue::from_static("198.51.100.9, 10.0.0.2"),
        );
        h
    };
    let resolved = wundspray::client_ip::resolve_client_ip(peer_addr, &headers, &trusted);
    assert_eq!(resolved.client_ip, IpAddr::from([198, 51, 100, 9]));

    let response = test_app_with_proxies(&trusted)
        .oneshot(clean_request(
            peer_addr,
            json!({ "url": "https://example.com/" }).to_string(),
            &[("x-forwarded-for", "198.51.100.9, 10.0.0.2")],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("x-request-id").is_some());
}

#[tokio::test]
async fn clean_echoes_inbound_request_id() {
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.10:12345"),
            json!({ "url": "https://example.com/" }).to_string(),
            &[("x-request-id", "req-from-ingress-1")],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-request-id").unwrap(),
        "req-from-ingress-1"
    );
}

#[tokio::test]
async fn clean_generates_request_id_when_missing() {
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.10:12345"),
            json!({ "url": "https://example.com/" }).to_string(),
            &[],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let id = response
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(uuid::Uuid::parse_str(id).is_ok());
}

#[tokio::test]
async fn clean_echoes_request_id_on_error() {
    let response = test_app()
        .oneshot(clean_request(
            peer("203.0.113.10:12345"),
            json!({ "url": "   " }).to_string(),
            &[("x-request-id", "err-corr-9")],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers().get("x-request-id").unwrap(),
        "err-corr-9"
    );
}
