// src/chunking/sentence.rs
use unicode_segmentation::UnicodeSegmentation;
use whatlang::{detect, Lang as WhatLang};
use crate::chunking::{Chunker, ChunkingConfig, ContentChunk};
use crate::crawler::types::ExtractedContent;
use uuid::Uuid;
use tracing::debug;

/// Чанкер по предложениям с использованием unicode-segmentation
pub struct SentenceChunker;

impl Default for SentenceChunker {
    fn default() -> Self {
        Self::new()
    }
}

impl SentenceChunker {
    pub fn new() -> Self {
        Self
    }

    /// Определение языка для улучшения детекции (опционально)
    fn detect_language(text: &str, hint: Option<&str>) -> Option<WhatLang> {
        // 1. Если есть явная подсказка
        if let Some(lang_code) = hint {
            return Self::map_lang_code(lang_code);
        }

        // 2. Автодетекция через whatlang
        detect(text).filter(|info| info.is_reliable()).map(|info| info.lang())
    }

    /// Маппинг кодов языков из ExtractedContent
    fn map_lang_code(code: &str) -> Option<WhatLang> {
        match code.to_lowercase().as_str() {
            "ru" | "rus" => Some(WhatLang::Rus),
            "en" | "eng" => Some(WhatLang::Eng),
            "de" | "ger" => Some(WhatLang::Deu),
            "fr" | "fre" => Some(WhatLang::Fra),
            "es" | "spa" => Some(WhatLang::Spa),
            "it" | "ita" => Some(WhatLang::Ita),
            "pt" | "por" => Some(WhatLang::Por),
            "zh" | "chi" => Some(WhatLang::Cmn),
            "ja" | "jpn" => Some(WhatLang::Jpn),
            _ => None,
        }
    }

    /// Разбивка текста на предложения с позициями (байтовые индексы)
    fn split_with_positions(text: &str) -> Vec<(String, usize, usize)> {
        let mut sentences = Vec::new();

        // UnicodeSegmentation::sentences() возвращает итератор по предложениям
        let mut start = 0;
        for sentence in text.unicode_sentences() {
            let trimmed = sentence.trim();
            if !trimmed.is_empty() {
                // Находим позицию в исходном тексте
                if let Some(pos) = text[start..].find(trimmed) {
                    let abs_start = start + pos;
                    let abs_end = abs_start + trimmed.len();
                    sentences.push((trimmed.to_string(), abs_start, abs_end));
                    start = abs_end;
                }
            } else {
                // Для пустых предложений просто сдвигаем позицию
                start += sentence.len();
            }
        }

        sentences
    }
}

impl Chunker for SentenceChunker {
    fn chunk(&self, content: &ExtractedContent, config: &ChunkingConfig) -> Vec<ContentChunk> {
        if content.content_text.is_empty() {
            return vec![];
        }

        // Определяем язык (для логов и метаданных)
        let detected_lang = Self::detect_language(
            &content.content_text,
            content.language.as_deref()
        );

        debug!(
            "🔤 Chunking: {} chars, detected_lang: {:?}",
            content.content_text.len(),
            detected_lang
        );

        // Разбиваем на предложения с позициями
        let sentences = Self::split_with_positions(&content.content_text);

        if sentences.is_empty() {
            return vec![];
        }

        let sentence_count = sentences.len();
        let mut chunks = Vec::new();
        let mut buffer: Vec<(String, usize, usize)> = Vec::new();
        let mut current_word_count = 0;
        let mut chunk_index = 0;

        for (sentence, start_pos, end_pos) in &sentences {
            let words = sentence.split_whitespace().count();

            // Если добавление предложения превысит лимит — сохраняем текущий чанк
            if current_word_count + words > config.max_chunk_size && !buffer.is_empty() {
                let chunk = self.make_chunk(content, &mut buffer, &mut chunk_index, config);
                chunks.push(chunk);
                current_word_count = 0;

                // Добавляем overlap из предыдущего чанка для контекста
                if config.overlap > 0 && !chunks.is_empty() {
                    let last_chunk = &chunks[chunks.len() - 1];
                    let words: Vec<&str> = last_chunk.content.split_whitespace().collect();
                    let overlap_words: Vec<&str> = words.into_iter().rev().take(config.overlap).rev().collect();
                    current_word_count = overlap_words.len();
                }
            }

            buffer.push((sentence.clone(), *start_pos, *end_pos));
            current_word_count += words;
        }

        // Сохраняем последний чанк
        if !buffer.is_empty() {
            let chunk = self.make_chunk(content, &mut buffer, &mut chunk_index, config);
            chunks.push(chunk);
        }

        // Фильтруем слишком короткие чанки
        chunks.retain(|c| c.word_count >= config.min_chunk_size);

        debug!("✅ Generated {} chunks from {} sentences", chunks.len(), sentence_count);
        chunks
    }
}

impl SentenceChunker {
    fn make_chunk(
        &self,
        content: &ExtractedContent,
        buffer: &mut Vec<(String, usize, usize)>,
        chunk_index: &mut usize,
        _config: &ChunkingConfig,
    ) -> ContentChunk {
        let sentences: Vec<_> = buffer.drain(..).collect();

        // Собираем текст чанка
        let text: String = sentences
            .iter()
            .map(|(s, _, _)| s.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        // Позиции в исходном тексте
        let start_char = sentences.first().map(|(_, s, _)| *s).unwrap_or(0);
        let end_char = sentences.last().map(|(_, _, e)| *e).unwrap_or(start_char + text.len());

        // Извлекаем возможный заголовок
        let title = if sentences.len() == 1 && sentences[0].0.len() < 100 {
            Some(sentences[0].0.clone())
        } else {
            None
        };

        ContentChunk {
            id: Uuid::new_v4(),
            source_id: content.id,
            source_url: content.source_url.clone(),
            chunk_index: *chunk_index,
            title,
            content: text.clone(),
            content_html: None,
            word_count: text.split_whitespace().count(),
            start_char,
            end_char,
            meta: serde_json::json!({
                "sentence_count": sentences.len(),
                "language": content.language,
                "char_range": [start_char, end_char],
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::ChunkingConfig;
    use crate::crawler::types::ExtractedContent;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_russian_sentence_splitting() {
        let chunker = SentenceChunker::new();
        let content = ExtractedContent {
            id: Uuid::new_v4(),
            source_url: "https://example.com/test".to_string(),
            final_url: "https://example.com/test".to_string(),
            title: "Test".to_string(),
            content_html: String::new(),
            content_text: "Это первое предложение. Это второе! А вот третье? Да, именно так.".to_string(),
            author: None,
            published_date: None,
            excerpt: None,
            image: None,
            language: Some("ru".to_string()),
            word_count: 15,
            crawled_at: Utc::now(),
            meta: serde_json::Value::Null,
            site_id: None,
            site_key: None,
        };

        let config = ChunkingConfig {
            max_chunk_size: 10,
            min_chunk_size: 2,
            overlap: 0,
            ..Default::default()
        };

        let chunks = chunker.chunk(&content, &config);

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.word_count >= config.min_chunk_size));

        println!("Generated {} chunks:", chunks.len());
        for (i, chunk) in chunks.iter().enumerate() {
            println!("  {}. [{}w] {}", i+1, chunk.word_count, chunk.content);
        }
    }

    #[test]
    fn test_unicode_boundaries() {
        // Тест с разными типами окончаний предложений
        let text = "Hello! How are you? I'm fine... Great.";
        let sentences: Vec<_> = text.split_sentence_bounds()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        assert!(sentences.len() >= 3); // Минимум 3 предложения
    }
}
