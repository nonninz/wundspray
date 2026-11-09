use std::net::SocketAddr;

use wundspray::routes::{app, AppState};
use wundspray::sanitize::shared_sanitizer;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let sanitizer = shared_sanitizer().unwrap_or_else(|err| {
        eprintln!("failed to load sanitizer catalog: {err}");
        std::process::exit(1);
    });

    let router = app(AppState { sanitizer });
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|err| {
            eprintln!("failed to bind {addr}: {err}");
            std::process::exit(1);
        });

    eprintln!("wundspray listening on http://{addr}");

    axum::serve(listener, router).await.unwrap_or_else(|err| {
        eprintln!("server error: {err}");
        std::process::exit(1);
    });
}
