// src/main.rs
use ai_gateway::storage::PostgresStorage;
use ai_gateway::llms_txt::sitemap::SitemapCrawler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("ai_gateway=debug,info")
        .init();

    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let storage = PostgresStorage::connect(&database_url).await?;

    // === 1. Краулинг для каждого сайта ===
    let sites = vec![
        // Ваш пример:
        ("newscryptonft.com", "News Crypto NFT", "https://newscryptonft.com/sitemap.xml"),
        // Добавьте другие сайты:
        // ("habr.com", "Habr", "https://habr.com/sitemap.xml"),
    ];

    for (site_key, site_name, sitemap_url) in sites {
        if std::env::var("CRAWL_SITEMAP").unwrap_or_default() == "true" {
            tracing::info!("🕷️  Crawling sitemap for {}...", site_key);

            let crawler = SitemapCrawler::new(Default::default());
            let count = crawler.crawl_sitemap_with_site(
                sitemap_url,
                &storage,
                site_key,  // ← Передаём site_key
                7000         // max_pages per site
            ).await?;

            tracing::info!("✅ Crawled {} pages for {}", count, site_key);
        }
    }

    // === 2. Генерация llms.txt для каждого сайта ===
    let active_sites = storage.get_active_sites().await?;

    for site in active_sites {
        tracing::info!("📝 Generating llms.txt for {}...", site.site_key);

        let result = storage.generate_llms_for_site(
            &site.site_key,
            &format!("public/{}", site.site_key)  // public/newscryptonft.com/llms.txt
        ).await?;

        tracing::info!("✓ Generated: {} pages, {} chunks for {}",
            result.pages_count, result.chunks_count, site.site_key);
    }

    tracing::info!("🎉 Pipeline completed!");
    Ok(())
}
