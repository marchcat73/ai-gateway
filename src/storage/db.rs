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
    pub embedding: Option<Vector>,  // ← Vector для pgvector
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

    pub async fn run_migrations(&self) -> Result<()> {
        info!("📜 Running database migrations...");
        // sqlx::migrate!("./migrations").run(&self.pool).await?;
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

        // Сохраняем чанки
        let chunks = self.chunker.chunk(&content, &self.chunk_config);
        info!("📝 Generated {} chunks", chunks.len());

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
                word_count, crawled_at, meta, embedding
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
}
