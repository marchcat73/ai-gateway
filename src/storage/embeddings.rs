// src/storage/embeddings.rs
use pgvector::Vector;  // ← Для работы с pgvector
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::{info, warn};
use std::sync::Arc;

use super::{StorageError, Result};

/// Обёртка над model2vec для генерации эмбеддингов
pub struct EmbeddingModel {
    // ← Реальная модель вместо заглушки
    model: Option<Arc<model2vec::Model2Vec>>,
    cache_pool: Option<PgPool>,
    dimension: usize,
}

impl EmbeddingModel {
    /// Инициализация модели (загружает model2vec)
    pub async fn new() -> Result<Self> {
        info!("🧠 Loading model2vec embedding model...");

        // Попытка загрузить модель (может занять время при первом запуске)
        let model_path = std::env::var("MODEL2VEC_PATH").unwrap_or_else(|_| "model2vec".to_string());
        let model = match model2vec::Model2Vec::from_pretrained(&model_path, None, None) {
            Ok(m) => {
                let dim = m.embedding_dim();
                info!("✓ Model loaded from {}: dimension={}", model_path, dim);
                Some(Arc::new(m))
            }
            Err(e) => {
                warn!("⚠️  Failed to load model2vec from {}: {}. Using fallback hasher.", model_path, e);
                None
            }
        };

        let dimension = model.as_ref()
            .map(|m| m.embedding_dim())
            .unwrap_or(512); // Fallback dimension

        Ok(Self {
            model,
            cache_pool: None,
            dimension,
        })
    }

    /// Подключение кэша эмбеддингов из PostgreSQL
    pub fn with_cache(mut self, pool: PgPool) -> Self {
        self.cache_pool = Some(pool);
        self
    }

    /// Генерация эмбеддинга для текста
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 1. Проверяем кэш (если подключен)
        if let Some(pool) = &self.cache_pool {
            if let Some(cached) = self.get_from_cache(pool, text).await? {
                return Ok(cached);
            }
        }

        // 2. Генерируем эмбеддинг
        let embedding_vec = if let Some(model) = &self.model {
            // ← Используем реальную модель
            match model.encode([text]) {
                Ok(embedding) => {
                    let (vec, _offset) = embedding.into_raw_vec_and_offset();
                    vec
                }
                Err(e) => {
                    warn!("⚠️  model2vec encode failed: {}. Using fallback.", e);
                    self.generate_fallback_embedding(text)
                }
            }
        } else {
            // Fallback если модель не загрузилась
            self.generate_fallback_embedding(text)
        };

        // 3. Сохраняем в кэш (если подключен)
        if let Some(pool) = &self.cache_pool {
            if let Err(e) = self.save_to_cache(pool, text, &embedding_vec).await {
                warn!("Failed to cache embedding: {}", e);
            }
        }

        Ok(embedding_vec)
    }

    /// Генерация эмбеддингов для батча текстов
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            embeddings.push(self.embed(text).await?);
        }
        Ok(embeddings)
    }

    /// ← ИСПРАВЛЕНО: Читаем как Vector, конвертируем в Vec<f32>
    async fn get_from_cache(&self, pool: &PgPool, text: &str) -> Result<Option<Vec<f32>>> {
        let text_hash = self.hash_text(text);

        let record: Option<Vector> = sqlx::query_scalar(
            r#"SELECT embedding FROM embedding_cache WHERE text_hash = $1"#
        )
        .bind(text_hash)
        .fetch_optional(pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(record.map(|v| v.to_vec())) // Vector → Vec<f32>
    }

    /// ← ИСПРАВЛЕНО: Конвертируем Vec<f32> в Vector перед записью
    async fn save_to_cache(&self, pool: &PgPool, text: &str, embedding: &[f32]) -> Result<()> {
        let text_hash = self.hash_text(text);
        let embedding_vector = Vector::from(embedding.to_vec()); // Vec<f32> → Vector

        sqlx::query(
            r#"
            INSERT INTO embedding_cache (text_hash, embedding, model_version)
            VALUES ($1, $2, $3)
            ON CONFLICT (text_hash) DO NOTHING
            "#
        )
        .bind(text_hash)
        .bind(embedding_vector) // ← Передаём Vector
        .bind("model2vec-v1")
        .execute(pool)
        .await
        .map_err(|e| StorageError::Database(e.to_string()))?;

        Ok(())
    }

    /// Хэширование текста для кэша
    fn hash_text(&self, text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// ← Fallback-генерация (если model2vec не загрузился)
    fn generate_fallback_embedding(&self, text: &str) -> Vec<f32> {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let hash_bytes = hasher.finalize();

        let mut embedding = vec![0.0f32; self.dimension];
        for (i, byte) in hash_bytes.iter().enumerate() {
            embedding[i % self.dimension] = (*byte as f32) / 128.0 - 1.0;
        }

        // Нормализация
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in embedding.iter_mut() {
                *val /= norm;
            }
        }
        embedding
    }

    /// Размерность вектора
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Косинусное сходство
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}
