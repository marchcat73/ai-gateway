// src/llms_txt/sitemap.rs
use quick_xml::de::from_str;
use serde::Deserialize;
use crate::llms_txt::LlmsConfig;
use crate::crawler::Crawler;
use crate::storage::ContentStorage;
use tracing::{debug, error, info, warn};
use std::future::Future;
use std::pin::Pin;

/// Элемент sitemap.xml
#[derive(Debug, Deserialize, Clone)]
pub struct SitemapUrl {
    pub loc: String,
    #[serde(default)]
    pub lastmod: Option<String>,
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

/// Загрузчик sitemap с интеграцией краулера
pub struct SitemapCrawler {
    config: LlmsConfig,
    crawler: Crawler,
}

impl SitemapCrawler {
    pub fn new(config: LlmsConfig) -> Self {
        Self {
            config,
            crawler: Crawler::new(),
        }
    }

    // src/llms_txt/sitemap.rs - добавьте этот метод в impl SitemapCrawler

    /// Загрузка sitemap и краулинг с привязкой к конкретному сайту
    pub async fn crawl_sitemap_with_site<S: ContentStorage>(
        &self,
        sitemap_url: &str,
        storage: &S,
        site_key: &str,  // ← Новый параметр
        max_pages: usize,
    ) -> Result<usize, SitemapError> {
        info!("🗺️  Loading sitemap for site {}: {}", site_key, sitemap_url);

        let urls = self.load(sitemap_url).await?;
        let filtered = self.filter_urls(urls);

        info!("📑 Found {} URLs after filtering", filtered.len());

        let mut crawled_count = 0;
        for url_entry in filtered.iter().take(max_pages) {
            // Проверяем, есть ли уже в БД
            if storage.exists_by_url(&url_entry.loc).await.unwrap_or(false) {
                info!("⏭️  Skipping (exists): {}", url_entry.loc);
                continue;
            }

            // Краулим
            match self.crawler.crawl(&url_entry.loc).await {
                Ok(mut content) => {
                    // ← КЛЮЧЕВОЕ: привязываем контент к сайту перед сохранением
                    content.site_key = Some(site_key.to_string());

                    info!("✅ Crawled: {}", content.title);

                    // Используем save_with_site вместо save
                    if let Err(e) = storage.save_with_site(content, site_key).await {
                        error!("Failed to save {}: {}", url_entry.loc, e);
                    } else {
                        crawled_count += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to crawl {}: {}", url_entry.loc, e);
                }
            }
        }

        Ok(crawled_count)
    }

    /// Загрузка sitemap и краулинг всех URL
    pub async fn crawl_sitemap<S: ContentStorage>(
        &self,
        sitemap_url: &str,
        storage: &S,
        max_pages: usize,
    ) -> Result<usize, SitemapError> {
        info!("🗺️  Loading sitemap: {}", sitemap_url);

        let urls = self.load(sitemap_url).await?;
        let filtered = self.filter_urls(urls);

        info!("📑 Found {} URLs after filtering", filtered.len());

        let mut crawled_count = 0;
        for url_entry in filtered.iter().take(max_pages) {
            // Проверяем, есть ли уже в БД
            if storage.exists_by_url(&url_entry.loc).await.unwrap_or(false) {
                info!("⏭️  Skipping (exists): {}", url_entry.loc);
                continue;
            }

            // Краулим и сохраняем
            match self.crawler.crawl(&url_entry.loc).await {
                Ok(content) => {
                    info!("✅ Crawled: {}", content.title);
                    if let Err(e) = storage.save(content).await {
                        error!("Failed to save {}: {}", url_entry.loc, e);
                    } else {
                        crawled_count += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to crawl {}: {}", url_entry.loc, e);
                }
            }
        }

        Ok(crawled_count)
    }

    /// Загрузка и парсинг sitemap.xml
    /// ← ИСПРАВЛЕНО: Возвращаем Pin<Box<Future>> для рекурсии
    pub fn load<'a>(
        &'a self,
        sitemap_url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SitemapUrl>, SitemapError>> + Send + 'a>> {


        Box::pin(async move {
            info!("📥 Fetching: {}", sitemap_url);

            let client = reqwest::Client::new();
            let response = client
                .get(sitemap_url)
                .header("User-Agent", "AIGatewayBot/1.0")
                .send()
                .await
                .map_err(|e| SitemapError::Fetch(e.to_string()))?;

            let xml = response.text().await
                .map_err(|e| SitemapError::Fetch(e.to_string()))?;

            debug!("🔍 Fetched {} bytes from {}", xml.len(), sitemap_url);
            if xml.contains("<sitemapindex") {
                debug!("📑 Detected sitemapindex structure");
            } else if xml.contains("<urlset") {
                debug!("📄 Detected urlset structure");
            }

            // Пробуем распарсить как SitemapIndex
            if let Ok(index) = from_str::<SitemapIndex>(&xml) {
                if !index.sitemaps.is_empty() {
                    info!("📑 Found sitemap index with {} references", index.sitemaps.len());

                    let mut all_urls = Vec::new();
                    for sitemap_ref in index.sitemaps {
                        match self.load(&sitemap_ref.loc).await {
                            Ok(urls) => {
                                info!("✓ Loaded {} URLs from {}", urls.len(), sitemap_ref.loc);
                                all_urls.extend(urls);
                            }
                            Err(e) => warn!("Failed to load nested sitemap {}: {}", sitemap_ref.loc, e),
                        }
                    }
                    return Ok(all_urls);
                }
            }

            // Парсим как обычный Sitemap
            match from_str::<Sitemap>(&xml) {
                Ok(sitemap) => {
                    info!("✓ Parsed {} URLs from sitemap", sitemap.urls.len());
                    Ok(sitemap.urls)
                }
                Err(e) => {
                    warn!("Failed to parse {} as Sitemap: {}", sitemap_url, e);
                    // Возвращаем пустой список вместо ошибки, чтобы не ломать весь процесс
                    Ok(Vec::new())
                }
            }
        })
    }

    /// Фильтрация URL по конфигурации
    pub fn filter_urls(&self, urls: Vec<SitemapUrl>) -> Vec<SitemapUrl> {
        urls.into_iter()
            .filter(|url_entry| {
                !self.config.exclude_patterns.iter().any(|pattern| {
                    regex::Regex::new(pattern)
                        .map(|re| re.is_match(&url_entry.loc))
                        .unwrap_or(false)
                })
            })
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SitemapError {
    #[error("Fetch error: {0}")]
    Fetch(String),
    #[error("Parse error: {0}")]
    Parse(String),
}
