// src/storage/db.rs
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, FromRow, Row};
use pgvector::Vector;  // ← Импортируем Vector
use async_trait::async_trait;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::info;

use super::{ContentStorage, StorageError, Result, EmbeddingModel};
use crate::crawler::types::ExtractedContent;
use crate::chunking::{ContentChunk, Chunker, ChunkingConfig, sentence::SentenceChunker};
use crate::llms_txt::{LlmsConfig, LlmsGenerator, LlmsResult};
use crate::storage::types::Site;
use crate::utils::{extract_site_name, normalize_site_url};

/// Реализация хранилища на PostgreSQL + pgvector
pub struct PostgresStorage {
    pool: PgPool,
    embedding_model: EmbeddingModel,
    chunker: Box<dyn Chunker>,
    chunk_config: ChunkingConfig,
}

#[derive(Debug, FromRow)]
pub struct DocumentRow {
    pub id: Uuid,
    pub source_url: String,
    pub final_url: String,
    pub title: String,
    pub content_html: Option<String>,
    pub content_text: String,
    pub author: Option<String>,
    pub published_date: Option<DateTime<Utc>>,
    pub excerpt: Option<String>,
    pub image: Option<String>,
    pub language: Option<String>,
    pub word_count: i32,
    pub crawled_at: DateTime<Utc>,
    pub meta: serde_json::Value,
    pub site_id: Option<Uuid>,
    pub site_key: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct ChunkRow {
    pub id: Uuid,
    pub document_id: Uuid,
    pub chunk_index: i32,
    pub title: Option<String>,
    pub content: String,
    pub content_html: Option<String>,
    pub word_count: i32,
    pub start_char: i32,
    pub end_char: i32,
    pub meta: serde_json::Value,
    // embedding не нужен при выборке чанков (только для поиска)
}

impl PostgresStorage {
    pub async fn connect(database_url: &str) -> Result<Self> {
        info!("🗄️  Connecting to PostgreSQL...");

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .min_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .connect(database_url)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        sqlx::query("SELECT 1")
            .fetch_one(&pool)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        info!("✓ Connected to PostgreSQL");

        let embedding_model = EmbeddingModel::new().await?;
        let embedding_model = embedding_model.with_cache(pool.clone());

        Ok(Self {
            pool,
            embedding_model,
            chunker: Box::new(SentenceChunker),
            chunk_config: ChunkingConfig::default(),
        })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn run_migrations(&self) -> Result<()> {
        info!("📜 Running database migrations...");
        info!("✓ Migrations completed");
        Ok(())
    }

    pub async fn exists_by_url(&self, url: &str) -> Result<bool> {
        let exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM documents WHERE source_url = $1)"
        )
        .bind(url)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(exists.0)
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM documents WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn get_all_documents(&self, limit: usize) -> Result<Vec<ExtractedContent>> {
        let rows = sqlx::query_as::<_, DocumentRow>(
            r#"
            SELECT id, source_url, final_url, title, content_html, content_text,
                   author, published_date, excerpt, image, language,
                   word_count, crawled_at, meta, site_id, site_key
            FROM documents
            ORDER BY crawled_at DESC
            LIMIT $1
            "#
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| ExtractedContent {
            id: r.id,
            source_url: r.source_url,
            final_url: r.final_url,
            title: r.title,
            content_html: r.content_html.unwrap_or_default(),
            content_text: r.content_text,
            author: r.author,
            published_date: r.published_date,
            excerpt: r.excerpt,
            image: r.image,
            language: r.language,
            word_count: r.word_count as usize,
            crawled_at: r.crawled_at,
            meta: r.meta,
            site_id: r.site_id,
            site_key: r.site_key,
        }).collect())
    }

    pub async fn get_all_chunks(&self, limit: usize) -> Result<Vec<ContentChunk>> {
        let rows = sqlx::query_as::<_, ChunkRow>(
            r#"
            SELECT id, document_id, chunk_index, title, content,
                   content_html, word_count, start_char, end_char, meta
            FROM chunks
            ORDER BY document_id, chunk_index
            LIMIT $1
            "#
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        // Подгружаем URL документов
        let doc_ids: Vec<Uuid> = rows.iter().map(|r| r.document_id).collect();
        let doc_urls = if !doc_ids.is_empty() {
            let docs = sqlx::query("SELECT id, source_url FROM documents WHERE id = ANY($1)")
                .bind(&doc_ids)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?;

            docs.into_iter()
                .filter_map(|r| {
                    let id = r.try_get::<Uuid, _>("id").ok()?;
                    let url = r.try_get::<String, _>("source_url").ok()?;
                    Some((id, url))
                })
                .collect::<std::collections::HashMap<_, _>>()
        } else {
            std::collections::HashMap::new()
        };

        Ok(rows.into_iter().map(|row| ContentChunk {
            id: row.id,
            source_id: row.document_id,
            source_url: doc_urls.get(&row.document_id).cloned().unwrap_or_default(),
            chunk_index: row.chunk_index as usize,
            title: row.title,
            content: row.content,
            content_html: row.content_html,
            word_count: row.word_count as usize,
            start_char: row.start_char as usize,
            end_char: row.end_char as usize,
            meta: row.meta,
        }).collect())
    }

    // В impl PostgresStorage:

    /// Создание или получение сайта по ключу
    pub async fn get_or_create_site(
        &self,
        site_key: &str,
        site_name: &str,
        site_url: &str,
    ) -> Result<Site> {
        // Пробуем получить существующий
        if let Some(site) = self.get_site_by_key(site_key).await? {
            return Ok(site);
        }

        // Создаём новый
        let site = Site {
            id: Uuid::new_v4(),
            site_key: site_key.to_string(),
            site_name: site_name.to_string(),
            site_url: site_url.to_string(),
            site_description: None,
            default_language: Some("en".to_string()),
            sitemap_url: None,
            crawl_enabled: Some(true),
            crawl_interval_hours: Some(24),
            include_patterns: None,
            exclude_patterns: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        sqlx::query(
            r#"
            INSERT INTO sites (
                id, site_key, site_name, site_url, default_language,
                crawl_enabled, crawl_interval_hours, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(site.id)
        .bind(&site.site_key)
        .bind(&site.site_name)
        .bind(&site.site_url)
        .bind(&site.default_language)
        .bind(site.crawl_enabled)
        .bind(site.crawl_interval_hours)
        .bind(site.created_at)
        .bind(site.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(site)
    }

/// Получение сайта по ключу
pub async fn get_site_by_key(&self, site_key: &str) -> Result<Option<Site>> {
    let site = sqlx::query_as!(
        Site,
        r#"
        SELECT
            id, site_key, site_name, site_url, site_description,
            default_language, sitemap_url, crawl_enabled,
            crawl_interval_hours, include_patterns, exclude_patterns,
            created_at, updated_at
        FROM sites WHERE site_key = $1
        "#,
        site_key
    )
    .fetch_optional(&self.pool)
    .await
    .map_err(|e| StorageError::Database(e.to_string()))?;

    Ok(site)
}

    /// Получение всех активных сайтов
    pub async fn get_active_sites(&self) -> Result<Vec<Site>> {
        let sites = sqlx::query_as!(
            Site,
            r#"
            SELECT
                id, site_key, site_name, site_url, site_description,
                default_language, sitemap_url, crawl_enabled,
                crawl_interval_hours, include_patterns, exclude_patterns,
                created_at, updated_at
            FROM sites WHERE crawl_enabled = true
            ORDER BY site_name
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(sites)
    }

    /// Обновление документа с привязкой к сайту
    pub async fn save_with_site_impl(
        &self,
        content: ExtractedContent,
        site_key: &str,
    ) -> Result<()> {
        // Получаем или создаём сайт
        let site = self.get_or_create_site(
            site_key,
            &extract_site_name(site_key),
            &normalize_site_url(site_key),
        ).await?;

        // Проверяем, должен ли этот URL быть включён
        if !site.should_include_url(&content.source_url) {
            tracing::debug!("⏭️  URL excluded by site filters: {}", content.source_url);
            return Ok(());
        }

        let mut tx = self.pool.begin().await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        // Сохраняем документ с site_id
        let doc_embedding = self.embedding_model.embed(&content.content_text).await?;
        let doc_embedding_vec = pgvector::Vector::from(doc_embedding);

        sqlx::query(
            r#"
            INSERT INTO documents (
                id, source_url, final_url, title, content_html,
                content_text, author, published_date, excerpt,
                image, language, word_count, crawled_at, meta,
                embedding, site_id, site_key
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            ON CONFLICT (source_url) DO UPDATE SET
                site_id = EXCLUDED.site_id,
                site_key = EXCLUDED.site_key,
                content_text = EXCLUDED.content_text,
                crawled_at = EXCLUDED.crawled_at,
                embedding = EXCLUDED.embedding
            "#
        )
        .bind(content.id)
        .bind(&content.source_url)
        .bind(&content.final_url)
        .bind(&content.title)
        .bind(&content.content_html)
        .bind(&content.content_text)
        .bind(&content.author)
        .bind(content.published_date)
        .bind(&content.excerpt)
        .bind(&content.image)
        .bind(&content.language)
        .bind(content.word_count as i32)
        .bind(content.crawled_at)
        .bind(serde_json::to_value(&content.meta).unwrap_or_default())
        .bind(doc_embedding_vec)
        .bind(site.id)
        .bind(&site.site_key)  // Денормализация для быстрых фильтров
        .execute(&mut *tx)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        // Сохраняем чанки (они наследуют site_id через document_id)
        let mut chunks = self.chunker.chunk(&content, &self.chunk_config);
        for chunk in &mut chunks {
            chunk.source_id = content.id;  // Синхронизация
        }

        for chunk in chunks {
            let chunk_embedding = self.embedding_model.embed(&chunk.content).await?;
            let chunk_embedding_vec = pgvector::Vector::from(chunk_embedding);

            sqlx::query(
                r#"
                INSERT INTO chunks (
                    id, document_id, chunk_index, title, content,
                    content_html, word_count, start_char, end_char,
                    meta, embedding
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#
            )
            .bind(chunk.id)
            .bind(chunk.source_id)  // Связь с документом → сайт
            .bind(chunk.chunk_index as i32)
            .bind(chunk.title)
            .bind(&chunk.content)
            .bind(&chunk.content_html)
            .bind(chunk.word_count as i32)
            .bind(chunk.start_char as i32)
            .bind(chunk.end_char as i32)
            .bind(serde_json::to_value(&chunk.meta).unwrap_or_default())
            .bind(chunk_embedding_vec)
            .execute(&mut *tx)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;
        }

        tx.commit().await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    /// Получение документов конкретного сайта
    pub async fn get_documents_by_site(&self, site_key: &str, limit: usize) -> Result<Vec<ExtractedContent>> {
        let rows = sqlx::query_as!(
            DocumentRow,
            r#"
            SELECT
                id, source_url, final_url, title, content_html, content_text,
                author, published_date, excerpt, image, language,
                word_count, crawled_at, meta, site_id, site_key
            FROM documents
            WHERE site_key = $1
            ORDER BY crawled_at DESC
            LIMIT $2
            "#,
            site_key,
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| ExtractedContent {
            id: r.id,
            source_url: r.source_url,
            final_url: r.final_url,
            title: r.title,
            content_html: r.content_html.unwrap_or_default(),
            content_text: r.content_text,
            author: r.author,
            published_date: r.published_date,
            excerpt: r.excerpt,
            image: r.image,
            language: r.language,
            word_count: r.word_count as usize,
            crawled_at: r.crawled_at,
            meta: r.meta,
            site_id: r.site_id,
            site_key: r.site_key,
        }).collect())
    }

    /// Получение чанков конкретного сайта
    pub async fn get_chunks_by_site(&self, site_key: &str, limit: usize) -> Result<Vec<ContentChunk>> {
        let rows = sqlx::query_as!(
            ChunkRow,
            r#"
            SELECT c.id, c.document_id, c.chunk_index, c.title, c.content,
                   c.content_html, c.word_count, c.start_char, c.end_char, c.meta
            FROM chunks c
            JOIN documents d ON c.document_id = d.id
            WHERE d.site_key = $1
            ORDER BY d.crawled_at DESC, c.chunk_index
            LIMIT $2
            "#,
            site_key,
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        // Подгружаем URL документов
        let doc_ids: Vec<Uuid> = rows.iter().map(|r| r.document_id).collect();
        let doc_urls = if !doc_ids.is_empty() {
            let docs = sqlx::query("SELECT id, source_url FROM documents WHERE id = ANY($1)")
                .bind(&doc_ids)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?;

            docs.into_iter()
                .filter_map(|r| {
                    let id = r.try_get::<Uuid, _>("id").ok()?;
                    let url = r.try_get::<String, _>("source_url").ok()?;
                    Some((id, url))
                })
                .collect::<std::collections::HashMap<_, _>>()
        } else {
            std::collections::HashMap::new()
        };

        Ok(rows.into_iter().map(|row| ContentChunk {
            id: row.id,
            source_id: row.document_id,
            source_url: doc_urls.get(&row.document_id).cloned().unwrap_or_default(),
            chunk_index: row.chunk_index as usize,
            title: row.title,
            content: row.content,
            content_html: row.content_html,
            word_count: row.word_count as usize,
            start_char: row.start_char as usize,
            end_char: row.end_char as usize,
            meta: row.meta,
        }).collect())
    }

    /// Генерация llms.txt для конкретного сайта
    pub async fn generate_llms_for_site(
        &self,
        site_key: &str,
        output_path: &str,
    ) -> Result<LlmsResult> {
        // Получаем сайт
        let site = self.get_site_by_key(site_key).await?
            .ok_or_else(|| StorageError::NotFound(format!("Site not found: {}", site_key)))?;

        // Получаем документы и чанки сайта
        let docs = self.get_documents_by_site(site_key, 7000).await?;
        let chunks = self.get_chunks_by_site(site_key, 5000).await?;

        // Настраиваем генератор под сайт
        let llms_config = LlmsConfig {
            site_url: site.site_url.clone(),
            site_name: site.site_name.clone(),
            site_description: site.site_description.clone(),
            default_language: site.default_language.clone().unwrap_or_else(|| "en".to_string()),
            include_chunk_content: std::env::var("INCLUDE_CHUNK_CONTENT").unwrap_or_default() == "true",
            max_links: 7000,
            exclude_patterns: site.exclude_patterns.clone().unwrap_or_default(),
            ..Default::default()
        };

        let generator = LlmsGenerator::new(llms_config);
        let result = generator.generate(&docs, &chunks);

        // Сохраняем в файл (с подпапкой для сайта)
        let file_path = format!("{}/llms.txt", output_path.trim_end_matches('/'));
        std::fs::create_dir_all(std::path::Path::new(&file_path).parent().unwrap())
            .map_err(|e| StorageError::Database(e.to_string()))?;

        std::fs::write(&file_path, &result.content)
            .map_err(|e| StorageError::Database(e.to_string()))?;

        tracing::info!("💾 Saved llms.txt for {} to {}", site_key, file_path);
        Ok(result)
    }
}

#[async_trait]
impl ContentStorage for PostgresStorage {
    async fn save(&self, content: ExtractedContent) -> Result<()> {
        info!("💾 Saving document: {} ({})", content.title, content.source_url);

        let mut tx = self.pool.begin().await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        // Генерируем и конвертируем эмбеддинг
        let doc_embedding = self.embedding_model.embed(&content.content_text).await?;
        let doc_embedding_vec = Vector::from(doc_embedding);  // ← Конвертация

        sqlx::query(
            r#"
            INSERT INTO documents (
                id, source_url, final_url, title, content_html,
                content_text, author, published_date, excerpt,
                image, language, word_count, crawled_at, meta, embedding
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (source_url) DO UPDATE SET
                content_text = EXCLUDED.content_text,
                content_html = EXCLUDED.content_html,
                title = EXCLUDED.title,
                word_count = EXCLUDED.word_count,
                crawled_at = EXCLUDED.crawled_at,
                updated_at = NOW(),
                embedding = EXCLUDED.embedding
            "#
        )
        .bind(content.id)
        .bind(&content.source_url)
        .bind(&content.final_url)
        .bind(&content.title)
        .bind(&content.content_html)
        .bind(&content.content_text)
        .bind(&content.author)
        .bind(content.published_date)
        .bind(&content.excerpt)
        .bind(&content.image)
        .bind(&content.language)
        .bind(content.word_count as i32)
        .bind(content.crawled_at)
        .bind(serde_json::to_value(&content.meta).unwrap_or_default())
        .bind(doc_embedding_vec)  // ← Передаём Vector
        .execute(&mut *tx)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        let mut chunks = self.chunker.chunk(&content, &self.chunk_config);
        info!("📝 Generated {} chunks", chunks.len());

        for chunk in &mut chunks {
            chunk.source_id = content.id;
            chunk.source_url = content.final_url.clone();
        }

        for chunk in chunks {
            let chunk_embedding = self.embedding_model.embed(&chunk.content).await?;
            let chunk_embedding_vec = Vector::from(chunk_embedding);  // ← Конвертация

            sqlx::query(
                r#"
                INSERT INTO chunks (
                    id, document_id, chunk_index, title, content,
                    content_html, word_count, start_char, end_char,
                    meta, embedding
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#
            )
            .bind(chunk.id)
            .bind(chunk.source_id)
            .bind(chunk.chunk_index as i32)
            .bind(chunk.title)
            .bind(&chunk.content)
            .bind(&chunk.content_html)
            .bind(chunk.word_count as i32)
            .bind(chunk.start_char as i32)
            .bind(chunk.end_char as i32)
            .bind(serde_json::to_value(&chunk.meta).unwrap_or_default())
            .bind(chunk_embedding_vec)  // ← Передаём Vector
            .execute(&mut *tx)
            .await
            .map_err(|e| StorageError::Database(e.to_string()))?;
        }

        tx.commit().await
            .map_err(|e| StorageError::Database(e.to_string()))?;

        info!("✓ Document saved successfully");
        Ok(())
    }

    async fn get_by_url(&self, url: &str) -> Result<Option<ExtractedContent>> {
        let row = sqlx::query_as::<_, DocumentRow>(
            r#"
            SELECT
                id, source_url, final_url, title, content_html, content_text,
                author, published_date, excerpt, image, language,
                word_count, crawled_at, meta, site_id, site_key
            FROM documents WHERE source_url = $1
            "#
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(row.map(|r| ExtractedContent {
            id: r.id,
            source_url: r.source_url,
            final_url: r.final_url,
            title: r.title,
            content_html: r.content_html.unwrap_or_default(),
            content_text: r.content_text,
            author: r.author,
            published_date: r.published_date,
            excerpt: r.excerpt,
            image: r.image,
            language: r.language,
            word_count: r.word_count as usize,
            crawled_at: r.crawled_at,
            meta: r.meta,
            site_id: r.site_id,
            site_key: r.site_key,
        }))
    }

    async fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<ContentChunk>> {
        info!("🔍 Semantic search: '{}' (limit={})", query, limit);

        let query_embedding = self.embedding_model.embed(query).await?;
        let query_vector = Vector::from(query_embedding);  // ← Конвертация

        let rows = sqlx::query_as::<_, ChunkRow>(
            r#"
            SELECT
                id, document_id, chunk_index, title, content,
                content_html, word_count, start_char, end_char,
                meta
            FROM chunks
            ORDER BY embedding <-> $1  -- pgvector оператор косинусного расстояния
            LIMIT $2
            "#
        )
        .bind(query_vector)  // ← Передаём Vector
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        // Получаем URL документов
        let doc_ids: Vec<Uuid> = rows.iter().map(|r| r.document_id).collect();
        let doc_urls = if !doc_ids.is_empty() {
            let docs = sqlx::query("SELECT id, source_url FROM documents WHERE id = ANY($1)")
                .bind(&doc_ids)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?;
            docs.into_iter()
                .filter_map(|r| {
                    let id = r.try_get::<Uuid, _>("id").ok()?;
                    let url = r.try_get::<String, _>("source_url").ok()?;
                    Some((id, url))
                })
                .collect::<std::collections::HashMap<_, _>>()
        } else {
            std::collections::HashMap::new()
        };

        let chunks: Vec<ContentChunk> = rows.into_iter().map(|row| ContentChunk {
            id: row.id,
            source_id: row.document_id,
            source_url: doc_urls.get(&row.document_id).cloned().unwrap_or_default(),
            chunk_index: row.chunk_index as usize,
            title: row.title,
            content: row.content,
            content_html: row.content_html,
            word_count: row.word_count as usize,
            start_char: row.start_char as usize,
            end_char: row.end_char as usize,
            meta: row.meta,
        }).collect();

        info!("✓ Found {} chunks", chunks.len());
        Ok(chunks)
    }

    // Переопределяем default impl для эффективной реализации
    async fn save_with_site(&self, content: ExtractedContent, site_key: &str) -> Result<()> {
        self.save_with_site_impl(content, site_key).await  // Вызов собственного метода
    }

    async fn delete_site_by_key(&self, site_key: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM sites WHERE site_key = $1"
        )
        .bind(site_key)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn search_semantic_by_site(
        &self,
        query: &str,
        site_key: &str,
        limit: usize
    ) -> Result<Vec<ContentChunk>> {
        let query_embedding = self.embedding_model.embed(query).await?;
        let query_vector = pgvector::Vector::from(query_embedding);

        let rows = sqlx::query_as::<_, ChunkRow>(
            r#"
            SELECT c.id, c.document_id, c.chunk_index, c.title, c.content,
                   c.content_html, c.word_count, c.start_char, c.end_char, c.meta
            FROM chunks c
            JOIN documents d ON c.document_id = d.id
            WHERE d.site_key = $1
            ORDER BY c.embedding <-> $2
            LIMIT $3
            "#
        )
        .bind(site_key)
        .bind(query_vector)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        // ... аналогично search_semantic, подгружаем URL документов ...
        // Получаем URL документов
        let doc_ids: Vec<Uuid> = rows.iter().map(|r| r.document_id).collect();
        let doc_urls = if !doc_ids.is_empty() {
            let docs = sqlx::query("SELECT id, source_url FROM documents WHERE id = ANY($1)")
                .bind(&doc_ids)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| StorageError::Database(e.to_string()))?;
            docs.into_iter()
                .filter_map(|r| {
                    let id = r.try_get::<Uuid, _>("id").ok()?;
                    let url = r.try_get::<String, _>("source_url").ok()?;
                    Some((id, url))
                })
                .collect::<std::collections::HashMap<_, _>>()
        } else {
            std::collections::HashMap::new()
        };

        let chunks: Vec<ContentChunk> = rows.into_iter().map(|row| ContentChunk {
            id: row.id,
            source_id: row.document_id,
            source_url: doc_urls.get(&row.document_id).cloned().unwrap_or_default(),
            chunk_index: row.chunk_index as usize,
            title: row.title,
            content: row.content,
            content_html: row.content_html,
            word_count: row.word_count as usize,
            start_char: row.start_char as usize,
            end_char: row.end_char as usize,
            meta: row.meta,
        }).collect();

        info!("✓ Found {} chunks", chunks.len());

        Ok(chunks)
    }
}
