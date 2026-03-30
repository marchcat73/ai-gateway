// src/api/handlers/admin.rs
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::api::state::ApiState;

#[derive(Deserialize)]
pub struct ClearRequest {
    /// Подтверждение: должен быть "YES_DELETE_EVERYTHING"
    pub confirm: String,
    /// Опционально: удалить только конкретный сайт
    pub site_key: Option<String>,
}

#[derive(Serialize)]
pub struct ClearResponse {
    pub message: String,
    pub deleted_documents: i64,
    pub deleted_chunks: i64,
    pub deleted_sites: i64,
}

#[derive(Serialize)]
pub struct SiteInfo {
    pub site_key: String,
    pub name: Option<String>,
    pub url: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn clear_database(
    State(state): State<ApiState>,
    Json(req): Json<ClearRequest>,
) -> Result<Json<ClearResponse>, StatusCode> {
    // Проверка подтверждения
    if req.confirm != "YES_DELETE_EVERYTHING" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut tx = state.storage.pool().begin().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (deleted_docs, deleted_chunks, deleted_sites) = if let Some(site_key) = req.site_key {
        // Удаление только одного сайта
        let docs = sqlx::query("DELETE FROM documents WHERE site_key = $1 RETURNING id")
            .bind(&site_key)
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let chunks = sqlx::query("DELETE FROM chunks WHERE document_id IN (SELECT id FROM documents WHERE site_key = $1)")
            .bind(&site_key)
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let sites = sqlx::query("DELETE FROM sites WHERE site_key = $1")
            .bind(&site_key)
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        (docs.len() as i64, chunks.rows_affected() as i64, sites.rows_affected() as i64)
    } else {
        // Полная очистка (в правильном порядке из-за FK)
        sqlx::query("TRUNCATE embedding_cache, chunks, documents, sites RESTART IDENTITY CASCADE")
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Получаем статистику (после TRUNCATE COUNT = 0, поэтому логируем до)
        (0, 0, 0) // Можно улучшить, считая до удаления
    };

    tx.commit().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::warn!("🗑️  Database cleared: {} docs, {} chunks, {} sites",
        deleted_docs, deleted_chunks, deleted_sites);

    Ok(Json(ClearResponse {
        message: "Database cleared successfully".to_string(),
        deleted_documents: deleted_docs,
        deleted_chunks,
        deleted_sites,
    }))
}

pub async fn list_all_sites(
    State(state): State<ApiState>,
) -> Result<Json<Vec<SiteInfo>>, StatusCode> {
    let sites = sqlx::query_as!(
        SiteInfo,
        "SELECT site_key, site_name as name, site_url as url, created_at FROM sites ORDER BY created_at DESC"
    )
    .fetch_all(state.storage.pool())
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(sites))
}
