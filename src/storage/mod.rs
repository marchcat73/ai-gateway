pub mod db;
pub mod embeddings;

use async_trait::async_trait;

use crate::crawler::types::ExtractedContent;
use crate::chunking::ContentChunk;

pub use db::PostgresStorage;
pub use embeddings::EmbeddingModel;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Embedding error: {0}")]
    Embedding(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Трейт для абстракции хранилища
#[async_trait]
pub trait ContentStorage: Send + Sync {
    /// Сохранение документа с чанками
    async fn save(&self, content: ExtractedContent) -> Result<()>;

    /// Получение документа по URL
    async fn get_by_url(&self, url: &str) -> Result<Option<ExtractedContent>>;

    /// Семантический поиск по чанкам
    async fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<ContentChunk>>;

    /// Проверка существования документа по URL
    async fn exists_by_url(&self, url: &str) -> Result<bool> {
        Ok(self.get_by_url(url).await?.is_some())
    }
}
