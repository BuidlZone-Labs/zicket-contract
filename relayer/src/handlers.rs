use crate::AppState;
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Deserialize)]
pub struct TransactionRequest {
    pub transaction_xdr: String,
    pub captcha_token: String,
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub status: String,
    pub tx_hash: Option<String>,
    pub error: Option<String>,
}

pub async fn submit_transaction(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<TransactionRequest>,
) -> impl IntoResponse {
    // 1. IP Rate Limiting
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

    if !state.rate_limiter.check_and_increment_ip(&ip) {
        tracing::warn!("Rate limit exceeded for IP: {}", ip);
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(TransactionResponse {
                status: "error".into(),
                tx_hash: None,
                error: Some("Rate limit exceeded for IP".into()),
            }),
        );
    }

    // 2. Session Rate Limiting
    if let Some(session_header) = headers.get("x-session-id") {
        if let Ok(session_id) = session_header.to_str() {
            if !state.rate_limiter.check_and_increment_session(session_id) {
                tracing::warn!("Rate limit exceeded for Session: {}", session_id);
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(TransactionResponse {
                        status: "error".into(),
                        tx_hash: None,
                        error: Some("Rate limit exceeded for Session".into()),
                    }),
                );
            }
        }
    }

    // 3. Captcha Verification
    if !state.captcha_service.verify(&payload.captcha_token).await {
        tracing::warn!("Invalid captcha token from IP: {}", ip);
        return (
            StatusCode::BAD_REQUEST,
            Json(TransactionResponse {
                status: "error".into(),
                tx_hash: None,
                error: Some("Invalid captcha token".into()),
            }),
        );
    }

    // 4. Submit to Soroban (Mocked)
    tracing::info!(
        "Successfully verified and rate-limited. Submitting TX: {}",
        payload.transaction_xdr
    );

    (
        StatusCode::OK,
        Json(TransactionResponse {
            status: "success".into(),
            tx_hash: Some("mocked_tx_hash_12345".into()),
            error: None,
        }),
    )
}
