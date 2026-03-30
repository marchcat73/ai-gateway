// src/llms_txt/mod.rs
pub mod generator;
pub use generator::LlmsGenerator;
pub mod sitemap;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Конфигурация генератора llms.txt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmsConfig {
    /// Базовый URL сайта
    pub site_url: String,
    /// Название сайта для заголовка
    pub site_name: String,
    /// Описание сайта (для секции "About")
    pub site_description: Option<String>,
    /// Язык контента по умолчанию
    pub default_language: String,
    /// Максимальное количество ссылок в llms.txt
    pub max_links: usize,
    /// Включать ли полные тексты чанков (или только ссылки)
    pub include_chunk_content: bool,
    /// Правила исключения контента (по паттернам URL)
    pub exclude_patterns: Vec<String>,
    /// Дата последней генерации (заполняется автоматически)
    #[serde(skip)]
    pub generated_at: Option<DateTime<Utc>>,
}

impl Default for LlmsConfig {
    fn default() -> Self {
        Self {
            site_url: "https://example.com".to_string(),
            site_name: "My Site".to_string(),
            site_description: None,
            default_language: "en".to_string(),
            max_links: 100,
            include_chunk_content: false, // По умолчанию только ссылки
            exclude_patterns: vec![
                r"/admin/.*".to_string(),
                r"/api/.*".to_string(),
                r"\?.*".to_string(), // URL с параметрами
            ],
            generated_at: None,
        }
    }
}

/// Представление элемента для llms.txt
#[derive(Debug, Clone, Serialize)]
pub struct LlmsEntry {
    /// Заголовок секции (H2/H3)
    pub title: String,
    /// URL страницы
    pub url: String,
    /// Краткое описание
    pub description: Option<String>,
    /// Язык контента
    pub language: Option<String>,
    /// Дата последнего обновления
    pub updated: Option<DateTime<Utc>>,
    /// Ссылки на чанки (якоря)
    pub chunks: Vec<ChunkReference>,
    /// Теги/категории
    pub tags: Vec<String>,
}

/// Ссылка на чанк внутри страницы
#[derive(Debug, Clone, Serialize)]
pub struct ChunkReference {
    /// Якорь для перехода (#chunk-uuid)
    pub anchor: String,
    pub anchor_link: Option<String>, // Полная ссылка с якорем (для генерации Markdown)
    /// Краткое превью контента
    pub preview: String,
    /// Позиция в документе (для сортировки)
    pub position: usize,
}

/// Результат генерации
#[derive(Debug, Clone)]
pub struct LlmsResult {
    /// Сгенерированный Markdown
    pub content: String,
    /// Количество включённых страниц
    pub pages_count: usize,
    /// Количество включённых чанков
    pub chunks_count: usize,
    /// Предупреждения при генерации
    pub warnings: Vec<String>,
}
