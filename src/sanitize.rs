//! Catalog loading and URL sanitization via `url-sanitize-core`.

use std::sync::Arc;

use serde::Deserialize;
use url_sanitize_core::{Catalog, SanitizeResult, Sanitizer, SanitizerOptions, SanitizerRule};

/// Merged ClearURLs / Brave / Firefox catalog pinned at upstream `v2.4.1`.
pub const CATALOG_JSON: &str = include_str!("../catalog/catalog.json");

/// Local custom rules merged on top of the pinned upstream catalog.
pub const OVERLAY_JSON: &str = include_str!("../catalog/overlay.json");

#[derive(Debug, Deserialize)]
struct OverlayFile {
    rules: Vec<SanitizerRule>,
}

/// Build the process-wide sanitizer with wundspray defaults.
///
/// Defaults intentionally differ from upstream's cautious CLI defaults:
/// referral-marketing params (affiliate `tag`, `ref`, …) are stripped.
pub fn build_sanitizer() -> Result<Sanitizer, String> {
    Ok(load_catalog()?.compile(wundspray_options()))
}

fn load_catalog() -> Result<Catalog, String> {
    let mut catalog = Catalog::from_json(CATALOG_JSON).map_err(|e| e.to_string())?;
    let overlay: OverlayFile =
        serde_json::from_str(OVERLAY_JSON).map_err(|e| format!("overlay parse error: {e}"))?;
    catalog.rules.extend(overlay.rules);
    Ok(catalog)
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

    fn cleaned_url(result: SanitizeResult) -> String {
        match result {
            SanitizeResult::Cleaned { url, .. } => url,
            SanitizeResult::Unchanged { url } => url,
            SanitizeResult::Redirected { url, .. } => url,
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn catalog_loads_and_reports_identity() {
        let catalog = load_catalog().expect("parse");
        assert!(catalog.rules.len() > 1000, "expected merged catalog");
        assert!(catalog.catalog_identity().contains("ccc0b05a867d"));
    }

    #[test]
    fn overlay_rules_are_merged() {
        let catalog = load_catalog().expect("parse");
        let providers: Vec<&str> = catalog
            .rules
            .iter()
            .filter_map(|rule| match rule {
                SanitizerRule::KeepParam { provider, .. } => Some(provider.as_str()),
                SanitizerRule::StripParam { provider, .. } => Some(provider.as_str()),
                _ => None,
            })
            .collect();
        assert!(providers.contains(&"wundspray-instagram-keep"));
        assert!(providers.contains(&"wundspray-instagram-random4"));
    }

    #[test]
    fn strips_utm_tracking() {
        let result = sanitize(
            &sanitizer(),
            "https://example.com/article?id=123&utm_source=newsletter&utm_medium=email",
        );
        match result {
            SanitizeResult::Cleaned {
                url,
                stripped_params,
                ..
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
                url,
                stripped_params,
                ..
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

    #[test]
    fn strips_instagram_obrf_share_token() {
        let result = sanitize(
            &sanitizer(),
            "https://www.instagram.com/p/ABC123/?obrf=ZDNlZmFiYzEyM2FiYw==",
        );
        match result {
            SanitizeResult::Cleaned {
                url,
                stripped_params,
                ..
            } => {
                assert!(!url.contains("obrf="), "obrf should be stripped: {url}");
                assert!(
                    stripped_params.iter().any(|p| p == "obrf"),
                    "expected obrf in stripped_params: {stripped_params:?}"
                );
            }
            other => panic!("expected cleaned, got {other:?}"),
        }
    }

    #[test]
    fn strips_instagram_random_four_letter_share_token() {
        let result = sanitize(
            &sanitizer(),
            "https://www.instagram.com/reel/XYZ789/?psln=YWJjZGVmZ2hpams=",
        );
        match result {
            SanitizeResult::Cleaned {
                url,
                stripped_params,
                ..
            } => {
                assert!(!url.contains("psln="), "psln should be stripped: {url}");
                assert!(
                    stripped_params.iter().any(|p| p == "psln"),
                    "expected psln in stripped_params: {stripped_params:?}"
                );
            }
            other => panic!("expected cleaned, got {other:?}"),
        }
    }

    #[test]
    fn keeps_instagram_functional_params() {
        let input = "https://www.instagram.com/p/ABC123/?img_index=2&page=3&next=%2Fp%2FABC123%2F";
        let url = cleaned_url(sanitize(&sanitizer(), input));
        assert!(url.contains("img_index=2"), "img_index must be kept: {url}");
        assert!(url.contains("page=3"), "page must be kept: {url}");
        assert!(url.contains("next="), "next must be kept: {url}");
    }

    #[test]
    fn overlay_survives_without_depending_on_base_instagram_rules() {
        // Base catalog already strips igsh; overlay must still strip random-4 tokens.
        let result = sanitize(
            &sanitizer(),
            "https://instagram.com/p/ABC123/?igsh=YWJjZGVmZ2hpams=&cplk=b3ZlcmxheXRva2VuMTI=",
        );
        match result {
            SanitizeResult::Cleaned {
                url,
                stripped_params,
                ..
            } => {
                assert!(!url.contains("igsh="), "igsh should be stripped: {url}");
                assert!(!url.contains("cplk="), "cplk should be stripped: {url}");
                assert!(stripped_params.iter().any(|p| p == "cplk"));
            }
            other => panic!("expected cleaned, got {other:?}"),
        }
    }
}
