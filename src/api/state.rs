// src/api/state.rs
use std::sync::Arc;
use crate::storage::PostgresStorage;

#[derive(Clone)]
pub struct ApiState {
    pub storage: Arc<PostgresStorage>,
    pub admin_token: String,  // Простой токен для админ-операций
}

impl ApiState {
    pub fn new(storage: PostgresStorage, admin_token: String) -> Self {
        Self {
            storage: Arc::new(storage),
            admin_token,
        }
    }
}

/// Extractor для проверки админ-токена
pub async fn require_admin(
    headers: axum::http::HeaderMap,
    state: axum::extract::State<ApiState>,
) -> Result<(), axum::http::StatusCode> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    if token == state.admin_token {
        Ok(())
    } else {
        Err(axum::http::StatusCode::FORBIDDEN)
    }
}
