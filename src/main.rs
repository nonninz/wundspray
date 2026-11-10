use std::net::SocketAddr;

use tracing::info;
use wundspray::logging;
use wundspray::routes::{app, AppState};
use wundspray::sanitize::shared_sanitizer;

#[tokio::main]
async fn main() {
    let filter = logging::init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let sanitizer = shared_sanitizer().unwrap_or_else(|err| {
        tracing::error!(error = %err, "failed to load sanitizer catalog");
        std::process::exit(1);
    });

    let state = AppState::new(sanitizer);
    let trusted_proxy_count = state.trusted_proxies.len();
    let trusted_proxies = state
        .trusted_proxies
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let router = app(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!(
        version = env!("CARGO_PKG_VERSION"),
        filter = %filter,
        %addr,
        trusted_proxy_count,
        trusted_proxies = %trusted_proxies,
        headers_trusted = trusted_proxy_count > 0,
        "wundspray starting"
    );

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|err| {
            tracing::error!(%addr, error = %err, "failed to bind");
            std::process::exit(1);
        });

    info!(%addr, "wundspray listening");

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap_or_else(|err| {
        tracing::error!(error = %err, "server error");
        std::process::exit(1);
    });
}
