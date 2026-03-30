// src/api/handlers/search.rs
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::{api::state::ApiState};
use crate::storage::ContentStorage;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub site_key: Option<String>,  // Фильтр по сайту
    pub min_score: Option<f32>,
}

fn default_limit() -> usize { 10 }

#[derive(Serialize)]
pub struct SearchResult {
    pub query: String,
    pub results: Vec<ChunkResult>,
    pub total: usize,
}

#[derive(Serialize, Clone)]
pub struct ChunkResult {
    pub id: uuid::Uuid,
    pub source_url: String,
    pub title: Option<String>,
    pub content: String,
    pub score: Option<f32>,  // Косинусное сходство
    pub chunk_index: usize,
}

pub async fn semantic_search(
    State(state): State<ApiState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResult>, StatusCode> {
    let chunks = if let Some(site_key) = &query.site_key {
        // Поиск в рамках сайта (нужно добавить метод в storage)
        state.storage.search_semantic_by_site(&query.q, &site_key, query.limit).await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        state.storage.search_semantic(&query.q, query.limit).await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let results_vec: Vec<ChunkResult> = chunks.into_iter().map(|chunk| ChunkResult {
        id: chunk.id,
        source_url: chunk.source_url,
        title: chunk.title,
        content: chunk.content,
        score: None,  // pgvector не возвращает score напрямую, можно вычислить
        chunk_index: chunk.chunk_index,
    }).collect();

    Ok(Json(SearchResult {
        query: query.q,
        results: results_vec.clone(),
        total: results_vec.len(),
    }))
}
