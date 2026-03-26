// src/crawler/fetcher.rs
use reqwest::Client;
use std::time::Duration;
use tracing::info;

use super::types::Result;

pub struct Fetcher {
    client: Client,
    user_agent: String,
    max_retries: u32,
}

pub struct FetchResult {
    pub html: String,
    pub final_url: String,
    pub status_code: u16,
    pub redirected: bool,
}

impl Fetcher {
    pub fn new(user_agent: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .gzip(true)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            user_agent: user_agent.unwrap_or_else(|| "AIGatewayBot/1.0 (+https://yourdomain.com/bot)".to_string()),
            max_retries: 3,
        }
    }

        pub async fn fetch(&self, url: &str) -> Result<FetchResult> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.try_fetch(url).await {
                Ok(result) => {
                    info!(
                        "✓ Fetched {} → {} (attempt {}, redirected={})",
                        url, result.final_url, attempt, result.redirected
                    );
                    return Ok(result);
                }
                Err(e) => {
                    tracing::warn!("Attempt {} failed for {}: {}", attempt, url, e);
                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                }
            }
        }
        Err(last_error.unwrap())
    }

    async fn try_fetch(&self, url: &str) -> Result<FetchResult> {
        let response: reqwest::Response = self.client
            .get(url)
            .header("User-Agent", &self.user_agent)
            .header("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8")
            .send()
            .await?;

        response.error_for_status_ref()?;

        // 🔑 Ключевой момент: получаем финальный URL после всех редиректов
        let final_url = response.url().to_string();
        let status_code = response.status().as_u16();
        let redirected = response.url().as_str() != url;

        let html = response.text().await?;

        Ok(FetchResult {
            html,
            final_url,
            status_code,
            redirected,
        })
    }
}
