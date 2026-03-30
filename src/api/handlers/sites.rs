// src/api/handlers/sites.rs
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::api::state::ApiState;
use crate::storage::types::Site;
use crate::storage::ContentStorage;

#[derive(Deserialize)]
pub struct CreateSiteRequest {
    pub site_key: String,
    pub name: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct SiteResponse {
    pub id: uuid::Uuid,
    pub site_key: String,
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub sitemap_url: Option<String>,
    pub crawl_enabled: Option<bool>,
    pub crawl_interval_hours: Option<i32>,
    pub include_patterns: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<Site> for SiteResponse {
    fn from(site: Site) -> Self {
        Self {
            id: site.id,
            site_key: site.site_key,
            name: site.site_name,
            url: site.site_url,
            description: site.site_description,
            language: site.default_language,
            sitemap_url: site.sitemap_url,
            crawl_enabled: site.crawl_enabled,
            crawl_interval_hours: site.crawl_interval_hours,
            include_patterns: site.include_patterns,
            exclude_patterns: site.exclude_patterns,
            created_at: site.created_at,
            updated_at: site.updated_at,
        }
    }
}

pub async fn list_sites(
    State(state): State<ApiState>,
) -> Result<Json<Vec<SiteResponse>>, StatusCode> {
    let sites = state.storage.get_active_sites().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let responses = sites.into_iter().map(SiteResponse::from).collect();
    Ok(Json(responses))
}

pub async fn get_site(
    State(state): State<ApiState>,
    Path(site_key): Path<String>,
) -> Result<Json<SiteResponse>, StatusCode> {
    let site = state.storage.get_site_by_key(&site_key).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(site.into()))
}

pub async fn create_site(
    State(state): State<ApiState>,
    Json(req): Json<CreateSiteRequest>,
) -> Result<Json<SiteResponse>, StatusCode> {
    let site = state.storage
        .get_or_create_site(&req.site_key, &req.name, &req.url)
        .await
        .map_err(|e| match e {
            crate::storage::StorageError::Validation(_) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    Ok(Json(site.into()))
}

pub async fn delete_site(
    State(state): State<ApiState>,
    Path(site_key): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.storage.delete_site_by_key(&site_key).await
        .map_err(|e| match e {
            crate::storage::StorageError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        })?;

    Ok(StatusCode::NO_CONTENT)
}
