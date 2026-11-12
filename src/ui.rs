//! Embedded clean-tool UI (HTML/CSS/JS).

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_CSS: &str = include_str!("../static/app.css");
const APP_JS: &str = include_str!("../static/app.js");

const CSP: &str = "default-src 'self'; frame-ancestors 'none'";

fn security_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers
}

fn static_response(body: &'static str, content_type: &'static str) -> Response {
    let mut headers = security_headers();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    (StatusCode::OK, headers, body).into_response()
}

pub async fn index() -> Response {
    static_response(INDEX_HTML, "text/html; charset=utf-8")
}

pub async fn app_css() -> Response {
    static_response(APP_CSS, "text/css; charset=utf-8")
}

pub async fn app_js() -> Response {
    static_response(APP_JS, "text/javascript; charset=utf-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_contain_expected_markers() {
        assert!(INDEX_HTML.contains("wundspray"));
        assert!(INDEX_HTML.contains("/assets/app.css"));
        assert!(INDEX_HTML.contains("/assets/app.js"));
        assert!(APP_JS.contains("/clean"));
        assert!(!APP_JS.contains("innerHTML"));
    }
}
