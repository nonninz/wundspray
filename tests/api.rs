use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use wundspray::routes::{app, AppState};
use wundspray::sanitize::shared_sanitizer;

fn test_app() -> axum::Router {
    let sanitizer = shared_sanitizer().expect("catalog");
    app(AppState { sanitizer })
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
async fn sanitize_strips_tracking() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sanitize")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "url": "https://example.com/a?id=1&utm_source=x&utm_medium=email"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
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
async fn sanitize_unwraps_redirect() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sanitize")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "url": "https://www.google.com/url?q=https%3A%2F%2Fexample.org%2Fpage&sa=U"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
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
async fn sanitize_rejects_empty_url() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sanitize")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "url": "   " }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "invalid_url");
}

#[tokio::test]
async fn sanitize_rejects_invalid_json() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sanitize")
                .header("content-type", "application/json")
                .body(Body::from("{not-json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "invalid_json");
}

#[tokio::test]
async fn sanitize_rejects_missing_url_field() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sanitize")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "link": "https://example.com" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"], "invalid_json");
}
