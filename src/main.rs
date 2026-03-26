use ai_gateway::{crawler::Crawler, storage::PostgresStorage, storage::ContentStorage};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("ai_gateway=debug,info")
        .init();

    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let storage = PostgresStorage::connect(
        &database_url
    ).await?;

    let crawler = Crawler::new();

    // 2. Краулим и сохраняем
    let url = "https://habr.com/ru/articles/752346/";
    tracing::info!("🕷️  Crawling: {}", url);

    match crawler.crawl(url).await {
        Ok(content) => {
            tracing::info!("✅ Extracted: {} ({} words)", content.title, content.word_count);

            // 3. Сохраняем в БД (включая чанкинг и эмбеддинги)
            storage.save(content).await?;
            tracing::info!("💾 Saved to PostgreSQL with chunks");

            // 4. Тест семантического поиска
            let query = "о чем эта статья?";
            let results = storage.search_semantic(query, 3).await?;
            tracing::info!("🔍 Search results for '{}':", query);
            for (i, chunk) in results.iter().enumerate() {
                tracing::info!("  {}. [{}] {}", i+1, chunk.word_count,
                    chunk.content.chars().take(100).collect::<String>());
            }
        }
        Err(e) => tracing::error!("❌ Crawl failed: {}", e),
    }

    Ok(())
}
