use readabilityrs::{Readability, ReadabilityOptions};
use crate::crawler::types::{ExtractedContent, CrawlerError, Result};
use uuid::Uuid;
use chrono::Utc;

pub struct Parser {
    options: ReadabilityOptions,
}

impl Parser {
    pub fn new() -> Self {
        let options = ReadabilityOptions::builder()
            .char_threshold(500)
            .nb_top_candidates(3)
            .keep_classes(false)
            .debug(false)
            .build();
        Self { options }
    }

    pub fn parse(&self, html: &str, base_url: &str) -> Result<ExtractedContent> {
        // Инициализация Readability
        let readability = Readability::new(html, Some(base_url), Some(self.options.clone()))
            .map_err(|e| CrawlerError::Parse(format!("Readability init failed: {}", e)))?;

        // Парсинг статьи
        let article = readability.parse()
            .ok_or_else(|| CrawlerError::Parse("No content extracted".to_string()))?;

        // 1. Сначала вычисляем метаданные (пока значения ещё не перемещены)
        let excerpt_len = article.excerpt.as_ref().map(String::len);
        let content_len = article.content.as_ref().map(String::len);

        // 2. Конвертация HTML → plain text для эмбеддингов
        let content_text = article.content
            .as_ref()
            .map(|html| html2text::from_read(html.as_bytes(), 80))
            .unwrap_or_default();

        // 3. Парсинг даты публикации (если есть)
        let published_date = article.published_time
            .as_ref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t)
                .ok()
                .map(|dt| dt.with_timezone(&Utc)));

        // 4. Теперь перемещаем значения в структуру
        Ok(ExtractedContent {
            id: Uuid::new_v4(),
            source_url: base_url.to_string(),
            final_url: base_url.to_string(),
            title: article.title.unwrap_or_default(),
            content_html: article.content.unwrap_or_default(), // ← move
            content_text: content_text.clone(),
            author: article.byline,
            published_date,
            excerpt: article.excerpt, // ← move
            image: article.image,
            language: article.dir.or(article.lang),
            word_count: content_text.split_whitespace().count(),
            crawled_at: Utc::now(),
            meta: serde_json::json!({
                "excerpt_len": excerpt_len,
                "content_len": content_len,
            }),
        })
    }
}
