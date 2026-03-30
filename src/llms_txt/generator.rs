// src/llms_txt/generator.rs
use crate::crawler::types::ExtractedContent;
use crate::chunking::ContentChunk;
use crate::llms_txt::{LlmsConfig, LlmsEntry, LlmsResult, ChunkReference};
use chrono::Utc;
use url::Url;
use std::collections::BTreeMap; // Для сортировки по URL
use tracing::{info, debug};

// ============================================================================
// Утилита для экранирования Markdown-символов
// ============================================================================

/// Экранирует специальные символы Markdown в тексте
/// Предотвращает поломку форматирования при вставке пользовательского контента
fn escape_markdown(text: &str) -> String {
    text.replace('\\', "\\\\")  // Сначала экранируем обратные слеши
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('`', "\\`")
        .replace('#', "\\#")    // Опционально: экранировать заголовки
        .replace('|', "\\|")    // Для таблиц
        .replace('>', "\\>")    // Для цитат
}

/// Основной генератор llms.txt
pub struct LlmsGenerator {
    config: LlmsConfig,
}

impl LlmsGenerator {
    pub fn new(config: LlmsConfig) -> Self {
        Self { config }
    }

    /// Генерация llms.txt из коллекции документов
    pub fn generate(&self, documents: &[ExtractedContent], chunks: &[ContentChunk]) -> LlmsResult {
        info!("📝 Generating llms.txt for {} documents", documents.len());

        let mut warnings = Vec::new();
        let mut entries: BTreeMap<String, LlmsEntry> = BTreeMap::new(); // Сортировка по URL

        // 1. Группируем чанки по document_id для быстрого доступа
        let chunks_by_doc: std::collections::HashMap<_, Vec<_>> =
            chunks.iter().fold(std::collections::HashMap::new(), |mut acc, chunk| {
                acc.entry(chunk.source_id).or_insert_with(Vec::new).push(chunk);
                acc
            });

        // 2. Обрабатываем каждый документ
        for doc in documents {
            // Пропускаем исключённые паттерны
            if self.is_excluded(&doc.source_url) {
                debug!("⏭️  Excluded: {}", doc.source_url);
                continue;
            }

            // Создаём запись
            let chunk_refs = chunks_by_doc.get(&doc.id)
                .map(|chunks| self.make_chunk_references(chunks))
                .unwrap_or_default();

            let entry = LlmsEntry {
                title: doc.title.clone(),
                url: doc.final_url.clone(),
                description: doc.excerpt.clone(),
                language: doc.language.clone().or(Some(self.config.default_language.clone())),
                updated: Some(doc.crawled_at),
                chunks: chunk_refs,
                tags: vec![], // Можно извлечь из meta keywords
            };

            entries.insert(doc.source_url.clone(), entry);

            // Ограничение по количеству ссылок
            if entries.len() >= self.config.max_links {
                warnings.push(format!(
                    "Reached max_links ({}), skipping remaining documents",
                    self.config.max_links
                ));
                break;
            }
        }

        // 3. Генерируем Markdown
        let content = self.render_markdown(&entries);
        let total_chunks: usize = entries.values().map(|e| e.chunks.len()).sum();

        info!("✓ Generated llms.txt: {} pages, {} chunks", entries.len(), total_chunks);

        LlmsResult {
            content,
            pages_count: entries.len(),
            chunks_count: total_chunks,
            warnings,
        }
    }

    /// Проверка: должен ли URL быть исключён
    fn is_excluded(&self, url: &str) -> bool {
        self.config.exclude_patterns.iter().any(|pattern| {
            // Простая проверка через regex (можно заменить на glob)
            match regex::Regex::new(pattern) {
                Ok(re) => re.is_match(url),
                Err(_) => false, // Игнорируем невалидные паттерны
            }
        })
    }

    /// Создание ссылок на чанки
    fn make_chunk_references(&self, chunks: &[&ContentChunk]) -> Vec<ChunkReference> {
        let mut refs = Vec::with_capacity(chunks.len());

        for (_idx, chunk) in chunks.iter().enumerate() {

            let anchor_id = format!("chunk-{}", &chunk.id.to_string()[..8]);  // без # для id
            let anchor_link = format!("#{}", anchor_id);

            // ← ИСПРАВЛЕНО: Экранируем Markdown в превью
            let preview_raw = if chunk.content.len() > 100 {
                let truncated: String = chunk.content.chars().take(100).collect();
                format!("{}...", truncated)
            } else {
                chunk.content.clone()
            };

            // ← Экранируем специальные символы
            let preview = escape_markdown(&preview_raw);

            refs.push(ChunkReference {
                anchor: anchor_id,
                anchor_link: Some(anchor_link),
                preview,
                position: chunk.chunk_index,
            });
        }

        // Сортируем по позиции в документе
        refs.sort_by_key(|r| r.position);
        refs
    }

    /// Рендеринг в Markdown
    fn render_markdown(&self, entries: &BTreeMap<String, LlmsEntry>) -> String {
        let mut md = String::new();

        // === Header ===
        md.push_str(&format!(
            r#"# {site_name}

> AI-Optimized Content Index
> Generated: {timestamp}
> Language: {lang}

"#,
            site_name = self.config.site_name,
            timestamp = Utc::now().format("%Y-%m-%d %H:%M UTC"),
            lang = self.config.default_language,
        ));

        // === About Section ===
        if let Some(desc) = &self.config.site_description {
            md.push_str(&format!(
                r#"## About

{desc}

This index is optimized for AI agents and LLMs.
Use the links below to access structured content chunks.

"#,
                desc = desc
            ));
        }

        // === Navigation Hints ===
        md.push_str(r#"## 🤖 For AI Agents

- Each page link may contain `#chunk-XXXX` anchors for precise content access
- Content is pre-chunked for efficient retrieval (RAG-friendly)
- Use semantic search via the API for contextual queries

"#);

        // === Content Index ===
        md.push_str("## 📚 Content Index\n\n");

        for entry in entries.values() {
            // Заголовок страницы
            md.push_str(&format!("### [{}]({})\n\n", entry.title, entry.url));

            // Метаданные
            if let Some(desc) = &entry.description {
                md.push_str(&format!("> {}\n\n", desc));
            }

            md.push_str(&format!(
                "- **Language**: {}\n",
                entry.language.as_deref().unwrap_or("unknown")
            ));
            if let Some(updated) = entry.updated {
                md.push_str(&format!("- **Updated**: {}\n", updated.format("%Y-%m-%d")));
            }
            md.push('\n');

            // Чанки (если включено)
            if !entry.chunks.is_empty() && self.config.include_chunk_content {
                md.push_str("#### Content Chunks\n\n");
                for chunk in &entry.chunks {
                    let anchor_id = &chunk.anchor;  // "chunk-3bbc941e" (без #)
                    let anchor_href = format!("#{}", anchor_id);  // "#chunk-3bbc941e" (с #)

                    // ← Экранируем превью ещё раз на всякий случай (защита в глубину)
                    let preview_escaped = escape_markdown(&chunk.preview);

                    md.push_str(&format!(
                        "- <a id=\"{anchor_id}\"></a>[`{anchor_href}`]({url}{anchor_href}): {preview}\n",
                        anchor_id = anchor_id,
                        anchor_href = anchor_href,
                        url = entry.url.split('#').next().unwrap_or(&entry.url),
                        preview = preview_escaped
                    ));
                }
                md.push('\n');
            } else if !entry.chunks.is_empty() {
                // Только количество чанков без контента
                md.push_str(&format!(
                    "_{} content chunks available (use anchors for precise access)_\n\n",
                    entry.chunks.len()
                ));
            }

            md.push_str("---\n\n");
        }

        // === Footer ===
        md.push_str(&format!(
            r#"
## ℹ️ Metadata

- **Total Pages**: {pages}
- **Total Chunks**: {chunks}
- **Index Format**: llms.txt v1.0
- **Generator**: AI Gateway

> For questions or API access, contact: [support@{domain}](mailto:support@{domain})
"#,
            pages = entries.len(),
            chunks = entries.values().map(|e| e.chunks.len()).sum::<usize>(),
            domain = Url::parse(&self.config.site_url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
                .unwrap_or_else(|| "example.com".to_string()),
        ));

        md
    }

    /// Сохранение результата в файл
    pub fn save_to_file(&self, result: &LlmsResult, path: &str) -> std::io::Result<()> {
        std::fs::write(path, &result.content)?;
        info!("💾 Saved llms.txt to {}", path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_cyrillic_preview() {
        let content = "Всем привет! Это тестовая статья на русском языке. ".repeat(10);

        // Должно обрезать по символам, а не байтам
        let preview = if content.chars().count() > 50 {
            let truncated: String = content.chars().take(50).collect();
            format!("{}...", truncated)
        } else {
            content.clone()
        };

        assert!(preview.ends_with("..."));
        assert_eq!(preview.chars().count(), 53); // 50 + 3 точки
        println!("Preview: {}", preview);
    }
}
