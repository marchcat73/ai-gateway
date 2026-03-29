-- ============================================================================
-- Миграция: Добавление таблицы sites и привязка документов к сайтам
-- ============================================================================

-- Таблица сайтов (источников контента)
CREATE TABLE IF NOT EXISTS sites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Уникальный идентификатор сайта
    site_key TEXT UNIQUE NOT NULL,
    site_name TEXT NOT NULL,
    site_url TEXT NOT NULL,

    -- Метаданные для llms.txt
    site_description TEXT,
    default_language TEXT DEFAULT 'en',

    -- Настройки краулинга
    sitemap_url TEXT,
    crawl_enabled BOOLEAN DEFAULT true,
    crawl_interval_hours INTEGER DEFAULT 24,

    -- Фильтры контента (массивы строк)
    include_patterns TEXT[],
    exclude_patterns TEXT[],

    -- Метаданные
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Уникальные ограничения
    CONSTRAINT site_url_unique UNIQUE (site_url)
);

-- Индексы для сайтов
CREATE INDEX IF NOT EXISTS idx_sites_site_key ON sites(site_key);
CREATE INDEX IF NOT EXISTS idx_sites_enabled ON sites(crawl_enabled) WHERE crawl_enabled = true;

-- ============================================================================
-- Обновление таблицы documents: добавляем привязку к сайту
-- ============================================================================

-- Добавляем site_id (внешний ключ) и site_key (денормализация для быстрых запросов)
DO $$
BEGIN
    -- Добавляем site_id если не существует
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'documents' AND column_name = 'site_id'
    ) THEN
        ALTER TABLE documents ADD COLUMN site_id UUID REFERENCES sites(id) ON DELETE SET NULL;
    END IF;

    -- Добавляем site_key если не существует
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'documents' AND column_name = 'site_key'
    ) THEN
        ALTER TABLE documents ADD COLUMN site_key TEXT;
    END IF;
END $$;

-- Индексы для быстрых запросов по сайту
CREATE INDEX IF NOT EXISTS idx_documents_site_id ON documents(site_id);
CREATE INDEX IF NOT EXISTS idx_documents_site_key ON documents(site_key);

-- ============================================================================
-- Триггер для updated_at
-- ============================================================================

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Применяем триггер к таблице sites (если ещё не создан)
DROP TRIGGER IF EXISTS update_sites_updated_at ON sites;
CREATE TRIGGER update_sites_updated_at
    BEFORE UPDATE ON sites
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
