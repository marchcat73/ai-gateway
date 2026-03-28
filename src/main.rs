use ai_gateway::storage::PostgresStorage;
use ai_gateway::llms_txt::{LlmsGenerator, LlmsConfig, sitemap::SitemapCrawler};

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

    // 2. Опционально: Краулинг sitemap
    let do_crawl = std::env::var("CRAWL_SITEMAP").unwrap_or_default() == "true";
    if do_crawl {
        let sitemap_url = "https://marchcat.com/sitemap.xml"; // Пример
        let config = LlmsConfig::default();
        let crawler = SitemapCrawler::new(config);

        tracing::info!("🕷️  Starting sitemap crawl...");
        let count = crawler.crawl_sitemap(sitemap_url, &storage, 5).await?;
        tracing::info!("✅ Crawled {} new pages", count);
    }

    // 3. Получаем данные из БД
    tracing::info!("📚 Loading documents for llms.txt...");
    let docs = storage.get_all_documents(100).await?;
    let chunks = storage.get_all_chunks(1000).await?;

    // 4. Генерация llms.txt
    let llms_config = LlmsConfig {
        site_url: "https://marchcat.com".to_string(),
        site_name: "AI Gateway Demo".to_string(),
        site_description: Some("Structured content index for AI agents".to_string()),
        include_chunk_content: false,
        max_links: 100,
        ..Default::default()
    };

    let generator = LlmsGenerator::new(llms_config);
    let result = generator.generate(&docs, &chunks);

    // 5. Сохранение
    generator.save_to_file(&result, "public/llms.txt")?;

    tracing::info!("🎉 Pipeline completed: {} pages, {} chunks",
        result.pages_count, result.chunks_count);

    Ok(())
}
