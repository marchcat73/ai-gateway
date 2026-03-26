-- migrations/20240101000001_init.sql

-- Включаем расширение pgvector
CREATE EXTENSION IF NOT EXISTS vector;

-- Таблица документов (источники)
CREATE TABLE IF NOT EXISTS documents (
    id UUID PRIMARY KEY,
    source_url TEXT UNIQUE NOT NULL,
    final_url TEXT NOT NULL,
    title TEXT NOT NULL,
    content_html TEXT,
    content_text TEXT NOT NULL,
    author TEXT,
    published_date TIMESTAMPTZ,
    excerpt TEXT,
    image TEXT,
    language TEXT,
    word_count INTEGER NOT NULL DEFAULT 0,
    crawled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    meta JSONB DEFAULT '{}'::jsonb,

    -- Вектор всего документа (опционально, для быстрого поиска по документу)
    embedding vector(512)
);

-- Индексы для документов
CREATE INDEX IF NOT EXISTS idx_documents_url ON documents(source_url);
CREATE INDEX IF NOT EXISTS idx_documents_crawled ON documents(crawled_at DESC);
CREATE INDEX IF NOT EXISTS idx_documents_embedding ON documents USING hnsw(embedding vector_cosine_ops);
CREATE INDEX IF NOT EXISTS idx_documents_fts ON documents USING GIN(to_tsvector('simple', content_text));

-- Таблица чанков (основная единица для RAG)
CREATE TABLE IF NOT EXISTS chunks (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    title TEXT,
    content TEXT NOT NULL,
    content_html TEXT,
    word_count INTEGER NOT NULL DEFAULT 0,
    start_char INTEGER NOT NULL DEFAULT 0,
    end_char INTEGER NOT NULL DEFAULT 0,
    metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Векторное представление чанка (обязательно для семантического поиска)
    embedding vector(512) NOT NULL
);

-- Индексы для чанков
CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_chunks_embedding ON chunks USING hnsw(embedding vector_cosine_ops);
CREATE INDEX IF NOT EXISTS idx_chunks_content ON chunks USING GIN(to_tsvector('simple', content));
CREATE INDEX IF NOT EXISTS idx_chunks_index ON chunks(document_id, chunk_index);

-- Таблица для кэша эмбеддингов (чтобы не генерировать дважды одинаковый текст)
CREATE TABLE IF NOT EXISTS embedding_cache (
    text_hash TEXT PRIMARY KEY, -- MD5 или SHA256 от текста
    embedding vector(512) NOT NULL,
    model_version TEXT NOT NULL DEFAULT 'model2vec-base-v1',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_embedding_cache_hash ON embedding_cache(text_hash);

-- Триггер для updated_at
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_documents_updated_at
    BEFORE UPDATE ON documents
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
