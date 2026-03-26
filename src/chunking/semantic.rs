// Заглушка для будущей реализации
// В продакшене: используйте `candle-transformers` или внешний API для эмбеддингов

use crate::chunking::{Chunker, ChunkingConfig, ContentChunk};
use crate::crawler::types::ExtractedContent;

pub struct SemanticChunker {
    // model: EmbeddingModel, // например, BGE-small через candle
}

impl SemanticChunker {
    pub fn new() -> Self {
        Self {
            // model: load_model()...
        }
    }
}

impl Chunker for SemanticChunker {
    fn chunk(&self, content: &ExtractedContent, config: &ChunkingConfig) -> Vec<ContentChunk> {
        // 1. Разбиваем на предложения
        // 2. Получаем эмбеддинги каждого предложения
        // 3. Кластеризуем / ищем разрывы по косинусному расстоянию
        // 4. Группируем в чанки <= max_chunk_size

        // Временно возвращаем простую разбивку как fallback
        super::sentence::SentenceChunker.chunk(content, config)
    }
}
