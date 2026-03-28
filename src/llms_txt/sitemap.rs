// src/llms_txt/sitemap.rs
use quick_xml::de::from_str;
use serde::Deserialize;
use url::Url;
use crate::llms_txt::LlmsConfig;
use tracing::{info, warn};

/// Элемент sitemap.xml
#[derive(Debug, Deserialize, Clone)]
pub struct SitemapUrl {
    pub loc: String,
    #[serde(default)]
    pub lastmod: Option<String>,
    #[serde(default)]
    pub changefreq: Option<String>,
    #[serde(default)]
    pub priority: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct Sitemap {
    #[serde(rename = "url", default)]
    pub urls: Vec<SitemapUrl>,
}

#[derive(Debug, Deserialize)]
struct SitemapIndex {
    #[serde(rename = "sitemap", default)]
    pub sitemaps: Vec<SitemapRef>,
}

#[derive(Debug, Deserialize)]
struct SitemapRef {
    pub loc: String,
}

/// Загрузчик sitemap для построения структуры сайта
pub struct SitemapLoader {
    config: LlmsConfig,
}

impl SitemapLoader {
    pub fn new(config: LlmsConfig) -> Self {
        Self { config }
    }

    /// Загрузка и парсинг sitemap.xml
    pub async fn load(&self, sitemap_url: &str) -> Result<Vec<SitemapUrl>, SitemapError> {
        info!("🗺️  Loading sitemap: {}", sitemap_url);

        let client = reqwest::Client::new();
        let response = client
            .get(sitemap_url)
            .header("User-Agent", "AIGatewayBot/1.0")
            .send()
            .await
            .map_err(|e| SitemapError::Fetch(e.to_string()))?;

        let xml = response.text().await
            .map_err(|e| SitemapError::Fetch(e.to_string()))?;

        // Пробуем распарсить как SitemapIndex (если есть вложенные sitemap)
        if let Ok(index) = from_str::<SitemapIndex>(&xml) {
            info!("📑 Found sitemap index with {} references", index.sitemaps.len());

            let mut all_urls = Vec::new();
            for sitemap_ref in index.sitemaps {
                match self.load(&sitemap_ref.loc).await {
                    Ok(urls) => all_urls.extend(urls),
                    Err(e) => warn!("Failed to load nested sitemap {}: {}", sitemap_ref.loc, e),
                }
            }
            return Ok(all_urls);
        }

        // Парсим как обычный Sitemap
        let sitemap: Sitemap = from_str(&xml)
            .map_err(|e| SitemapError::Parse(e.to_string()))?;

        info!("✓ Parsed {} URLs from sitemap", sitemap.urls.len());
        Ok(sitemap.urls)
    }

    /// Фильтрация URL по конфигурации
    pub fn filter_urls(&self, urls: Vec<SitemapUrl>) -> Vec<SitemapUrl> {
        urls.into_iter()
            .filter(|url_entry| {
                // Проверяем исключённые паттерны
                !self.config.exclude_patterns.iter().any(|pattern| {
                    regex::Regex::new(pattern)
                        .map(|re| re.is_match(&url_entry.loc))
                        .unwrap_or(false)
                })
            })
            .collect()
    }

    /// Преобразование SitemapUrl в LlmsEntry (заглушка)
    pub fn to_llms_entry(&self, sitemap_url: &SitemapUrl) -> Option<crate::llms_txt::LlmsEntry> {
        // Для полноценной реализации нужен краулер для получения контента
        // Здесь возвращаем базовую структуру
        Some(crate::llms_txt::LlmsEntry {
            title: extract_title_from_url(&sitemap_url.loc),
            url: sitemap_url.loc.clone(),
            description: None,
            language: Some(self.config.default_language.clone()),
            updated: sitemap_url.lastmod
                .as_ref()
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            chunks: vec![],
            tags: vec![],
        })
    }
}

fn extract_title_from_url(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.path_segments())
        .and_then(|mut segs| segs.last().map(str::to_string))
        .map(|s| s.replace('-', " ").replace('_', " "))
        .map(|s| {
            // Capitalize first letter
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .unwrap_or_else(|| "Untitled".to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum SitemapError {
    #[error("Fetch error: {0}")]
    Fetch(String),
    #[error("Parse error: {0}")]
    Parse(String),
}
