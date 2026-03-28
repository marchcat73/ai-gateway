-- Fix: изменить размерность векторов с 512 на 256 для совместимости с potion-multilingual-128M

-- Пересоздаём колонки с правильной размерностью
-- Сначала удаляем старые индексы
DROP INDEX IF EXISTS idx_documents_embedding;
DROP INDEX IF EXISTS idx_chunks_embedding;
DROP INDEX IF EXISTS idx_embedding_cache_hash;

-- Удаляем старые колонки и добавляем новые с vector(256)
ALTER TABLE documents
    DROP COLUMN IF EXISTS embedding,
    ADD COLUMN embedding vector(256);

ALTER TABLE chunks
    DROP COLUMN IF EXISTS embedding,
    ADD COLUMN embedding vector(256) NOT NULL;

ALTER TABLE embedding_cache
    DROP COLUMN IF EXISTS embedding,
    ADD COLUMN embedding vector(256) NOT NULL;

-- Пересоздаём индексы с правильной размерностью
CREATE INDEX idx_documents_embedding ON documents USING hnsw(embedding vector_cosine_ops);
CREATE INDEX idx_chunks_embedding ON chunks USING hnsw(embedding vector_cosine_ops);
CREATE INDEX idx_embedding_cache_hash ON embedding_cache(text_hash);

-- Очищаем кэш эмбеддингов (старые векторы несовместимы)
TRUNCATE embedding_cache;
