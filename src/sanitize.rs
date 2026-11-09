//! Catalog loading and URL sanitization via `url-sanitize-core`.

use std::sync::Arc;

use url_sanitize_core::{Catalog, SanitizeResult, Sanitizer, SanitizerOptions};

/// Merged ClearURLs / Brave / Firefox catalog pinned at upstream `v2.4.1`.
pub const CATALOG_JSON: &str = include_str!("../catalog/catalog.json");

/// Build the process-wide sanitizer with wundspray defaults.
///
/// Defaults intentionally differ from upstream's cautious CLI defaults:
/// referral-marketing params (affiliate `tag`, `ref`, …) are stripped.
pub fn build_sanitizer() -> Result<Sanitizer, String> {
    let catalog = Catalog::from_json(CATALOG_JSON).map_err(|e| e.to_string())?;
    Ok(catalog.compile(wundspray_options()))
}

/// Shared handle for axum state.
pub type SharedSanitizer = Arc<Sanitizer>;

pub fn shared_sanitizer() -> Result<SharedSanitizer, String> {
    build_sanitizer().map(Arc::new)
}

pub fn wundspray_options() -> SanitizerOptions {
    SanitizerOptions {
        strip_referral_marketing: true,
        unwrap_redirects: Some(true),
        domain_blocking: false,
    }
}

pub fn sanitize(sanitizer: &Sanitizer, url: &str) -> SanitizeResult {
    sanitizer.sanitize(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use url_sanitize_core::SanitizeResult;

    fn sanitizer() -> Sanitizer {
        build_sanitizer().expect("catalog must compile")
    }

    #[test]
    fn catalog_loads_and_reports_identity() {
        let catalog = Catalog::from_json(CATALOG_JSON).expect("parse");
        assert!(catalog.rules.len() > 1000, "expected merged catalog");
        assert!(catalog.catalog_identity().contains("ccc0b05a867d"));
    }

    #[test]
    fn strips_utm_tracking() {
        let result = sanitize(
            &sanitizer(),
            "https://example.com/article?id=123&utm_source=newsletter&utm_medium=email",
        );
        match result {
            SanitizeResult::Cleaned {
                url, stripped_params, ..
            } => {
                assert_eq!(url, "https://example.com/article?id=123");
                assert!(stripped_params.iter().any(|p| p == "utm_source"));
                assert!(stripped_params.iter().any(|p| p == "utm_medium"));
            }
            other => panic!("expected cleaned, got {other:?}"),
        }
    }

    #[test]
    fn strips_referral_marketing_amazon_tag() {
        let result = sanitize(
            &sanitizer(),
            "https://www.amazon.com/dp/B000123456?tag=someaffil-20&psc=1",
        );
        match result {
            SanitizeResult::Cleaned {
                url, stripped_params, ..
            } => {
                assert!(
                    !url.contains("tag="),
                    "affiliate tag should be stripped: {url}"
                );
                assert!(
                    stripped_params.iter().any(|p| p == "tag"),
                    "expected tag in stripped_params: {stripped_params:?}"
                );
            }
            other => panic!("expected cleaned, got {other:?}"),
        }
    }

    #[test]
    fn unwraps_google_redirect() {
        let result = sanitize(
            &sanitizer(),
            "https://www.google.com/url?q=https%3A%2F%2Fexample.org%2Fpage&sa=U",
        );
        match result {
            SanitizeResult::Redirected { url, .. } => {
                assert!(
                    url.starts_with("https://example.org/"),
                    "unexpected redirect target: {url}"
                );
            }
            other => panic!("expected redirected, got {other:?}"),
        }
    }

    #[test]
    fn leaves_clean_url_unchanged() {
        let input = "https://example.com/plain";
        let result = sanitize(&sanitizer(), input);
        match result {
            SanitizeResult::Unchanged { url } => assert_eq!(url, input),
            // WHATWG normalization alone can yield cleaned with empty rules
            SanitizeResult::Cleaned {
                url,
                matched_rules,
                stripped_params,
                ..
            } if matched_rules.is_empty() && stripped_params.is_empty() => {
                assert!(url.starts_with("https://example.com/"));
            }
            other => panic!("expected unchanged (or trivial clean), got {other:?}"),
        }
    }
}
