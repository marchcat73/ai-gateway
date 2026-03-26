// src/chunking/mod.rs
pub mod sentence;

use crate::crawler::types::ExtractedContent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Представление одного чанка для индексации
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunk {
    pub id: Uuid,
    pub source_id: Uuid, // Ссылка на ExtractedContent
    pub source_url: String,
    pub chunk_index: usize, // Порядок в документе
    pub title: Option<String>, // Заголовок секции (если есть)
    pub content: String, // Текст чанка (очищенный)
    pub content_html: Option<String>, // HTML-версия для отображения
    pub word_count: usize,
    pub start_char: usize, // Позиция в исходном тексте (для цитирования)
    pub end_char: usize,
    pub meta: serde_json::Value, // Доп. данные: теги, важность и т.д.
}

/// Стратегии чанкинга
#[derive(Debug, Clone, Copy, Default)]
pub enum ChunkingStrategy {
    #[default]
    BySentence,      // Простая разбивка по предложениям
    ByParagraph,     // По абзацам (<p>, <li>)
    Semantic,        // ML-based (опционально, через BPE/токены)
    Hierarchical,    // Заголовки H1-H3 как границы
}

#[derive(Debug, Clone)]
pub struct ChunkingConfig {
    pub strategy: ChunkingStrategy,
    pub max_chunk_size: usize, // слов
    pub min_chunk_size: usize,
    pub overlap: usize, // слов перекрытия между чанками (для контекста)
    pub preserve_structure: bool, // Сохранять ли HTML-теги внутри чанка
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            strategy: ChunkingStrategy::default(),
            max_chunk_size: 256,
            min_chunk_size: 32,
            overlap: 20,
            preserve_structure: false,
        }
    }
}

/// Основной трейт для чанкеров
pub trait Chunker: Send + Sync {
    fn chunk(&self, content: &ExtractedContent, config: &ChunkingConfig) -> Vec<ContentChunk>;
}
