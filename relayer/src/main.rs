use axum::{routing::post, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber;

mod captcha;
mod handlers;
mod rate_limit;

use captcha::CaptchaService;
use rate_limit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub rate_limiter: Arc<RateLimiter>,
    pub captcha_service: Arc<CaptchaService>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Initialize shared state
    let state = AppState {
        rate_limiter: Arc::new(RateLimiter::new(
            5,                                  // Max 5 requests per IP
            5,                                  // Max 5 requests per Session
            std::time::Duration::from_secs(60), // Time window: 60 seconds
        )),
        captcha_service: Arc::new(CaptchaService::new(
            // Use environment variables in production, hardcoded dummy for testing
            std::env::var("CAPTCHA_SECRET").unwrap_or_else(|_| "dummy_secret".to_string()),
        )),
    };

    let app = Router::new()
        .route("/submit_transaction", post(handlers::submit_transaction))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Relayer API listening on {}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
