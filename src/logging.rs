//! Process logging setup for wundspray.

use std::env;

use tracing_subscriber::EnvFilter;

const DEFAULT_LEVEL: &str = "info";

/// Resolve `LOG_LEVEL` into a tracing filter directive.
///
/// Missing or empty → `info`. Bare level names are scoped mainly to this crate
/// so dependency crates stay quiet. Filter expressions containing `=` are
/// passed through unchanged.
pub fn resolve_filter(raw: Option<&str>) -> String {
    let value = raw
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_LEVEL);

    if value.contains('=') {
        return value.to_owned();
    }

    let level = value.to_ascii_lowercase();
    match level.as_str() {
        "trace" | "debug" | "info" | "warn" | "error" | "off" => {
            format!("wundspray={level},tower=warn,hyper=warn")
        }
        other => {
            if EnvFilter::try_new(other).is_ok() {
                other.to_owned()
            } else {
                format!("wundspray={DEFAULT_LEVEL},tower=warn,hyper=warn")
            }
        }
    }
}

/// Install a compact, colored (ANSI) global subscriber.
///
/// Safe to call once at process start.
pub fn init() {
    let filter = resolve_filter(env::var("LOG_LEVEL").ok().as_deref());

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&filter).unwrap_or_else(|_| {
            EnvFilter::new(format!("wundspray={DEFAULT_LEVEL},tower=warn,hyper=warn"))
        }))
        .with_target(false)
        .with_level(true)
        .with_ansi(true)
        .compact()
        .init();

    tracing::debug!(filter = %filter, "logging initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_when_missing_or_empty() {
        assert!(resolve_filter(None).contains("wundspray=info"));
        assert!(resolve_filter(Some("")).contains("wundspray=info"));
        assert!(resolve_filter(Some("   ")).contains("wundspray=info"));
    }

    #[test]
    fn accepts_standard_levels_case_insensitively() {
        assert_eq!(
            resolve_filter(Some("DEBUG")),
            "wundspray=debug,tower=warn,hyper=warn"
        );
        assert_eq!(
            resolve_filter(Some("warn")),
            "wundspray=warn,tower=warn,hyper=warn"
        );
        assert!(resolve_filter(Some("trace")).contains("wundspray=trace"));
        assert!(resolve_filter(Some("error")).contains("wundspray=error"));
    }

    #[test]
    fn passes_through_explicit_filter_expressions() {
        let expr = "wundspray=trace,hyper=error";
        assert_eq!(resolve_filter(Some(expr)), expr);
    }
}
