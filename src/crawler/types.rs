// src/crawler/types.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedContent {
    pub id: Uuid,
    pub source_url: String,
    pub final_url: String, // После редиректов
    pub title: String,
    pub content_html: String, // Очищенный HTML
    pub content_text: String, // Plain text для эмбеддингов/поиска
    pub author: Option<String>,
    pub published_date: Option<DateTime<Utc>>,
    pub excerpt: Option<String>,
    pub image: Option<String>,
    pub language: Option<String>,
    pub word_count: usize,
    pub crawled_at: DateTime<Utc>,
    pub meta: serde_json::Value, // Дополнительные meta-теги
}

#[derive(Debug, thiserror::Error)]
pub enum CrawlerError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

pub type Result<T> = std::result::Result<T, CrawlerError>;
