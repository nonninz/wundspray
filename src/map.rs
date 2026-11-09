//! Map `url-sanitize-core::SanitizeResult` into wundspray's stable API envelope.

use serde::Serialize;
use url_sanitize_core::{MatchedRule, SanitizeResult};

/// Stable JSON response for `POST /sanitize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SanitizeResponse {
    pub input: String,
    /// Cleaned / redirected URL. `None` only when `blocked` is true.
    pub url: Option<String>,
    pub changed: bool,
    pub action: Action,
    pub stripped_params: Vec<String>,
    pub blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Unchanged,
    Cleaned,
    Redirected,
    Blocked,
}

/// Explainability kept for DEBUG logs (not returned by the API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizeDetails {
    pub matched_rules: Vec<MatchedRule>,
    pub via: Option<MatchedRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedSanitize {
    pub response: SanitizeResponse,
    pub details: SanitizeDetails,
}

pub fn map_result(input: &str, result: SanitizeResult) -> MappedSanitize {
    match result {
        SanitizeResult::Unchanged { url } => MappedSanitize {
            response: SanitizeResponse {
                input: input.to_owned(),
                url: Some(url),
                changed: false,
                action: Action::Unchanged,
                stripped_params: Vec::new(),
                blocked: false,
            },
            details: SanitizeDetails {
                matched_rules: Vec::new(),
                via: None,
            },
        },
        SanitizeResult::Cleaned {
            original,
            url,
            stripped_params,
            matched_rules,
        } => MappedSanitize {
            response: SanitizeResponse {
                input: original,
                url: Some(url),
                changed: true,
                action: Action::Cleaned,
                stripped_params,
                blocked: false,
            },
            details: SanitizeDetails {
                matched_rules,
                via: None,
            },
        },
        SanitizeResult::Redirected {
            original,
            url,
            via,
        } => MappedSanitize {
            response: SanitizeResponse {
                input: original,
                url: Some(url),
                changed: true,
                action: Action::Redirected,
                stripped_params: Vec::new(),
                blocked: false,
            },
            details: SanitizeDetails {
                matched_rules: Vec::new(),
                via: Some(via),
            },
        },
        SanitizeResult::Blocked { original, via } => MappedSanitize {
            response: SanitizeResponse {
                input: original,
                url: None,
                changed: true,
                action: Action::Blocked,
                stripped_params: Vec::new(),
                blocked: true,
            },
            details: SanitizeDetails {
                matched_rules: Vec::new(),
                via: Some(via),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sanitize::{wundspray_options, CATALOG_JSON};
    use url_sanitize_core::Catalog;

    fn live(url: &str) -> MappedSanitize {
        let sanitizer = Catalog::from_json(CATALOG_JSON)
            .expect("catalog")
            .compile(wundspray_options());
        map_result(url, sanitizer.sanitize(url))
    }

    #[test]
    fn maps_unchanged() {
        let mapped = map_result(
            "https://example.com/plain-path",
            SanitizeResult::Unchanged {
                url: "https://example.com/plain-path".into(),
            },
        );
        assert_eq!(mapped.response.action, Action::Unchanged);
        assert!(!mapped.response.changed);
        assert!(!mapped.response.blocked);
        assert_eq!(
            mapped.response.url.as_deref(),
            Some("https://example.com/plain-path")
        );
        assert!(mapped.response.stripped_params.is_empty());
        assert!(mapped.details.matched_rules.is_empty());
        assert!(mapped.details.via.is_none());
    }

    #[test]
    fn maps_cleaned_from_live_sanitizer() {
        let input = "https://example.com/article?id=1&utm_source=newsletter";
        let mapped = live(input);
        assert_eq!(mapped.response.action, Action::Cleaned);
        assert!(mapped.response.changed);
        assert!(!mapped.response.blocked);
        assert_eq!(
            mapped.response.url.as_deref(),
            Some("https://example.com/article?id=1")
        );
        assert!(
            mapped
                .response
                .stripped_params
                .iter()
                .any(|p| p == "utm_source")
        );
        assert!(!mapped.details.matched_rules.is_empty());
    }

    #[test]
    fn maps_redirected_from_live_sanitizer() {
        let input = "https://www.google.com/url?q=https%3A%2F%2Fexample.org%2Fpage&sa=U";
        let mapped = live(input);
        assert_eq!(mapped.response.action, Action::Redirected);
        assert!(mapped.response.changed);
        assert!(!mapped.response.blocked);
        assert!(
            mapped
                .response
                .url
                .as_deref()
                .is_some_and(|u| u.starts_with("https://example.org/")),
            "url={:?}",
            mapped.response.url
        );
        assert!(mapped.response.stripped_params.is_empty());
        assert!(mapped.details.via.is_some());
    }

    #[test]
    fn maps_blocked_when_domain_blocking_enabled() {
        let input = "https://pagead2.googlesyndication.com/pagead/ads";
        let mut options = wundspray_options();
        options.domain_blocking = true;
        let sanitizer = Catalog::from_json(CATALOG_JSON)
            .expect("catalog")
            .compile(options);
        let mapped = map_result(input, sanitizer.sanitize(input));
        assert_eq!(mapped.response.action, Action::Blocked);
        assert!(mapped.response.changed);
        assert!(mapped.response.blocked);
        assert_eq!(mapped.response.url, None);
        assert!(mapped.details.via.is_some());
        // Service default keeps blocking off.
        assert!(!wundspray_options().domain_blocking);
    }

    #[test]
    fn response_serializes_snake_case_without_debug_fields() {
        let mapped = live("https://example.com/?utm_source=x");
        let json = serde_json::to_value(&mapped.response).unwrap();
        assert_eq!(json["action"], "cleaned");
        assert_eq!(json["stripped_params"][0], "utm_source");
        assert_eq!(json["blocked"], false);
        assert_eq!(json["changed"], true);
        assert!(json.get("matched_rules").is_none());
        assert!(json.get("via").is_none());
    }
}
