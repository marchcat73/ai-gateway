// src/chunking/sentence.rs
use crate::chunking::{Chunker, ChunkingConfig, ContentChunk};
use crate::crawler::types::ExtractedContent;
use uuid::Uuid;
use std::collections::VecDeque;

// Простой детектор границ предложений (можно заменить на `punctuate` или `stanza-rs`)
fn split_sentences(text: &str, _language: Option<&str>) -> Vec<(String, usize, usize)> {
    // Эвристика: разбиваем по [.!?] + пробел/конец строки
    // Для продакшена: используйте crate = "punctuate" или ML-модель
    let mut sentences = Vec::new();
    let mut start = 0;

    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?' | '。' | '！' | '？') {
            // Проверяем, что это не сокращение (упрощенно)
            let is_end_of_sentence = i + 1 >= text.len() || text[i+1..].chars().next().is_some_and(|ch| ch.is_whitespace());
            if is_end_of_sentence {
                let end = i + c.len_utf8();
                let sentence = text[start..end].trim().to_string();
                if !sentence.is_empty() {
                    sentences.push((sentence, start, end));
                }
                start = end;
            }
        }
    }

    // Добавляем остаток
    if start < text.len() {
        let remainder = text[start..].trim();
        if !remainder.is_empty() {
            sentences.push((remainder.to_string(), start, text.len()));
        }
    }

    sentences
}

pub struct SentenceChunker;

impl Chunker for SentenceChunker {
    fn chunk(&self, content: &ExtractedContent, config: &ChunkingConfig) -> Vec<ContentChunk> {
        let sentences = split_sentences(&content.content_text, content.language.as_deref());

        if sentences.is_empty() {
            return vec![];
        }

        let mut chunks = Vec::new();
        let mut buffer = VecDeque::new();
        let mut current_size = 0;
        let mut chunk_index = 0;

        for (sentence, _start_pos, _end_pos) in sentences {
            let words = sentence.split_whitespace().count();

            // Если добавление предложения превысит лимит — сохраняем текущий чанк
            if current_size + words > config.max_chunk_size && !buffer.is_empty() {
                let chunk = self.make_chunk(content, &mut buffer, &mut chunk_index, config);
                chunks.push(chunk);
                current_size = 0;

                // Добавляем overlap из предыдущего чанка
                if config.overlap > 0 && !chunks.is_empty() {
                    let last = chunks.last().unwrap();
                    let overlap_words: Vec<&str> = last.content.split_whitespace()
                        .collect::<Vec<_>>()
                        .iter()
                        .rev()
                        .take(config.overlap)
                        .copied()
                        .collect();
                    current_size = overlap_words.len();
                    for w in &overlap_words {
                        buffer.push_back((*w).to_string());
                    }
                }
            }

            buffer.push_back(sentence);
            current_size += words;
        }

        // Сохраняем последний чанк
        if !buffer.is_empty() {
            let chunk = self.make_chunk(content, &mut buffer, &mut chunk_index, config);
            chunks.push(chunk);
        }

        chunks
    }
}

impl SentenceChunker {
    fn make_chunk(
        &self,
        content: &ExtractedContent,
        buffer: &mut VecDeque<String>,
        chunk_index: &mut usize,
        config: &ChunkingConfig,
    ) -> ContentChunk {
        let sentences: Vec<_> = buffer.drain(..).collect();
        let text = sentences.join(" ");

        // Находим позиции в исходном тексте (упрощенно)
        let start_char = content.content_text.find(&text).unwrap_or(0);
        let end_char = start_char + text.len();

        let chunk = ContentChunk {
            id: Uuid::new_v4(),
            source_id: content.id,
            source_url: content.source_url.clone(),
            chunk_index: *chunk_index,
            title: None, // Можно извлечь из ближайшего <h*>
            content: text.clone(),
            content_html: if config.preserve_structure {
                // Здесь можно мапить текст обратно на HTML-ноды через scraper
                None
            } else {
                None
            },
            word_count: text.split_whitespace().count(),
            start_char,
            end_char,
            meta: serde_json::json!({
                "sentence_count": sentences.len(),
                "language": content.language,
            }),
        };

        *chunk_index += 1;
        chunk
    }
}
