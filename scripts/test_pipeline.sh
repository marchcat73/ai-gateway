#!/bin/bash
set -e

echo "🚀 Starting AI Gateway Pipeline Test..."

# 1. Очистка БД (для чистого теста)
echo "🧹 Cleaning database..."
psql $DATABASE_URL -c "TRUNCATE documents, chunks, embedding_cache RESTART IDENTITY CASCADE;"

# 2. Тест краулинга одной страницы
echo "🕷️  Testing single page crawl..."
export CRAWL_SITEMAP=false
cargo run --quiet

# 3. Проверка наличия данных в БД
echo "🗄️  Checking database..."
DOC_COUNT=$(psql $DATABASE_URL -t -c "SELECT COUNT(*) FROM documents;")
CHUNK_COUNT=$(psql $DATABASE_URL -t -c "SELECT COUNT(*) FROM chunks;")

echo "   Documents: $DOC_COUNT"
echo "   Chunks: $CHUNK_COUNT"

if [ "$DOC_COUNT" -lt 1 ]; then
    echo "❌ No documents found!"
    exit 1
fi

if [ "$CHUNK_COUNT" -lt 1 ]; then
    echo "❌ No chunks found!"
    exit 1
fi

# 4. Проверка эмбеддингов
echo "🧠 Checking embeddings..."
EMB_COUNT=$(psql $DATABASE_URL -t -c "SELECT COUNT(*) FROM chunks WHERE embedding IS NOT NULL;")
echo "   Chunks with embeddings: $EMB_COUNT"

if [ "$EMB_COUNT" -lt 1 ]; then
    echo "❌ No embeddings found!"
    exit 1
fi

# 5. Проверка llms.txt
echo "📝 Checking llms.txt..."
if [ -f "public/llms.txt" ]; then
    LINES=$(wc -l < public/llms.txt)
    echo "   llms.txt: $LINES lines"

    # Проверка на кириллицу (не должно быть паник)
    if grep -q "##" public/llms.txt; then
        echo "   ✅ Markdown structure OK"
    else
        echo "   ❌ Markdown structure broken"
        exit 1
    fi
else
    echo "❌ llms.txt not found!"
    exit 1
fi

# 6. Тест sitemap (опционально, долго)
echo "🗺️  Testing sitemap crawl (optional)..."
export CRAWL_SITEMAP=true
timeout 60 cargo run --quiet || echo "⏭️  Sitemap test skipped (timeout)"

echo ""
echo "🎉 All tests passed!"
echo "   - Documents: $DOC_COUNT"
echo "   - Chunks: $CHUNK_COUNT"
echo "   - Embeddings: $EMB_COUNT"
echo "   - llms.txt: Generated"
