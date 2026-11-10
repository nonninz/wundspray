use std::net::SocketAddr;

use tracing::info;
use wundspray::logging;
use wundspray::routes::{app, AppState};
use wundspray::sanitize::shared_sanitizer;

#[tokio::main]
async fn main() {
    logging::init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let sanitizer = shared_sanitizer().unwrap_or_else(|err| {
        tracing::error!(error = %err, "failed to load sanitizer catalog");
        std::process::exit(1);
    });

    let router = app(AppState { sanitizer });
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

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
