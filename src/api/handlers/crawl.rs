// src/api/handlers/crawl.rs
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use crate::{api::state::ApiState, llms_txt::LlmsConfig};
use crate::llms_txt::sitemap::SitemapCrawler;

#[derive(Serialize)]
pub struct CrawlResponse {
    pub message: String,
    pub crawled_count: usize,
}

pub async fn trigger_crawl(
    State(state): State<ApiState>,
    Path(site_key): Path<String>,
) -> Result<Json<CrawlResponse>, StatusCode> {
    // Получаем сайт
    let site = state.storage.get_site_by_key(&site_key).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Определяем URL sitemap
    let sitemap_url = if let Some(sitemap) = &site.sitemap_url {
        sitemap.clone()
    } else {
        format!("{}/sitemap.xml", site.site_url.trim_end_matches('/'))
    };

    let config = LlmsConfig::default();
    let crawler = SitemapCrawler::new(config);
    let crawled_count = crawler.crawl_sitemap_with_site(&sitemap_url, &*state.storage, &site_key, std::env::var("MAX_PAGES").unwrap_or_else(|_| "7000".into()).parse().unwrap_or(7000)).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CrawlResponse {
        message: format!("Crawled {} pages for site {}", crawled_count, site_key),
        crawled_count,
    }))
}
