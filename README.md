# ai-gateway

## Database

```bash
# 1. Запуск БД
docker-compose up -d
podman-compose up -d

# 2. Установка sqlx-cli для миграций
cargo install sqlx-cli

# 3. Применение миграций
sqlx migrate run --database-url postgres://ai_gateway:dev_password@localhost:5432/ai_gateway

# 4. Запуск приложения
sudo docker ps -a
# уесли контейнер не запущен
sudo docker start ai-gateway-postgres-1
podman start ai-gateway-postgres-1
cargo run

# Очистка
podman-compose down --volumes --rmi all

# если без docker
sudo -u postgres psql
CREATE USER ai_gateway WITH PASSWORD 'dev_password';
CREATE DATABASE ai_gateway OWNER ai_gateway;


TRUNCATE documents, chunks RESTART IDENTITY CASCADE;
```

## Errors

```bash
cargo check 2>&1 | tee build_error.log.txt
```

## Test

```bash
# Запустите тесты чанкера
cargo test chunking::sentence

# Запуск с логами
RUST_LOG=ai_gateway=debug cargo run
```

1. Установите huggingface-hub (если нет)

pip install huggingface-hub

2. Скачайте модель в локальную папку

```sh
cd models
git clone https://huggingface.co/minishlab/potion-multilingual-128M
```

3. Проверьте структуру папки:

```sh
ls -la ./models/potion-multilingual-128M/
```

Должны быть файлы:

- config.json

- model.safetensors

- tokenizer.json (или tokenizer_config.json)

- special_tokens_map.json

## Tests

```sh
# Создаём тестовый сайт
curl -X POST http://localhost:3000/api/sites \
 -H "Authorization: Bearer change-me-in-prod" \
 -H "Content-Type: application/json" \
 -d '{
   "site_key": "cryptonewsnft.com",
   "name": "Solana validator monitoring",
   "url": "https://cryptonewsnft.com"
 }' | jq .

# Ожидаемый ответ:
{
  "id": "76a38dbb-83d5-44d0-8496-92f860781721",
  "site_key": "cryptonewsnft.com",
  "name": "Solana validator monitoring",
  "url": "https://cryptonewsnft.com",
  "description": null,
  "language": "en",
  "sitemap_url": null,
  "crawl_enabled": true,
  "crawl_interval_hours": 24,
  "include_patterns": null,
  "exclude_patterns": null,
  "created_at": "2026-03-30T11:22:42.421360595Z",
  "updated_at": "2026-03-30T11:22:42.421364006Z"
}

```

```sh
# Запускаем краулинг для сайта
curl -X POST http://localhost:3000/api/sites/cryptonewsnft.com/crawl \
  -H "Authorization: Bearer change-me-in-prod" \
  -H "Content-Type: application/json" \
  -d '{
    "sitemap_url": "https://cryptonewsnft.com/sitemap.xml",
    "max_pages": 100
  }' | jq .

# Ожидаемый ответ:
{
  "message": "Crawled 43 pages for site cryptonewsnft.com",
  "crawled_count": 43
}

```

```sh
# Регенерация llms.txt для сайта

curl -X POST http://localhost:3000/api/sites/cryptonewsnft.com/regenerate \
 -H "Authorization: Bearer change-me-in-prod" \
 -H "Content-Type: application/json" \
 -d '{"include_chunks": true}' | jq .

# Получение сгенерированного llms.txt

curl -s http://localhost:3000/api/sites/cryptonewsnft.com/llms.txt | head -30

```

## Поиск с фильтрами

```sh
# Поиск по всему индексу
curl -s "http://localhost:3000/api/search?q=bitcoin&limit=5" | jq .

# Поиск с фильтром по сайту
curl -s "http://localhost:3000/api/search?q=bitcoin&site_key=marchcat.com&limit=5" | jq .

# Поиск с минимальным score (если реализовано)
curl -s "http://localhost:3000/api/search?q=defi&min_score=0.7&limit=3" | jq .
```

## Test MCP

```sh
# 1. Простой тест инициализации
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | \
  cargo run --quiet -- --mode mcp 2>/dev/null

# Должен вернуть что-то вроде:
# {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05",...}}

# 2. Тест поиска (если есть данные в БД)
echo '{"jsonrpc":"2.0","id":2,"method":"search_semantic","params":{"query":"bitcoin","limit":2}}' | \
  cargo run --quiet -- --mode mcp 2>/dev/null

# 3. Тест llms.txt
echo '{"jsonrpc":"2.0","id":3,"method":"get_llms_txt","params":{"site_key":"newscryptonft.com"}}' | \
  cargo run --quiet -- --mode mcp 2>/dev/null
```
