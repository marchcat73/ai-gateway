// src/crawler/fetcher.rs
use reqwest::{Client, Response};
use std::time::Duration;
use tracing::info;

use super::types::{CrawlerError, Result};

pub struct Fetcher {
    client: Client,
    user_agent: String,
    max_retries: u32,
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

    pub async fn fetch(&self, url: &str) -> Result<String> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.try_fetch(url).await {
                Ok(html) => {
                    info!("✓ Fetched {} (attempt {})", url, attempt);
                    return Ok(html);
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

    async fn try_fetch(&self, url: &str) -> Result<String> {
        let response: Response = self.client
            .get(url)
            .header("User-Agent", &self.user_agent)
            .header("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8")
            .send()
            .await?;

        response.error_for_status_ref()?;
        let html = response.text().await?;
        Ok(html)
    }
}
