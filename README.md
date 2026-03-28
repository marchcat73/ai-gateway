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

# 1. Установите huggingface-hub (если нет)

pip install huggingface-hub

# 2. Скачайте модель в локальную папку

```sh
cd models
git clone https://huggingface.co/minishlab/potion-multilingual-128M
```

# 3. Проверьте структуру папки:

```sh
ls -la ./models/potion-multilingual-128M/
```

# Должны быть файлы:

# - config.json

# - model.safetensors

# - tokenizer.json (или tokenizer_config.json)

# - special_tokens_map.json
