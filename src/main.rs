use ai_gateway::crawler::Crawler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Инициализация логирования
    tracing_subscriber::fmt::init();

    let crawler = Crawler::new();

    // Тестовая статья (например, с Хабра или блога)
    let test_url = "https://habr.com/ru/articles/752346/";

    println!("🕷️  Crawling: {}", test_url);

    match crawler.crawl(test_url).await {
        Ok(content) => {
            // Сохраняем в файл для проверки
            let json = serde_json::to_string_pretty(&content)?;
            std::fs::write("extracted_content.json", json)?;
            println!("\n💾 Saved to extracted_content.json");

            println!("\n✅ Success!");
            println!("📰 Title: {}", content.title);
            println!("📝 Excerpt: {}", content.excerpt.unwrap_or_default());
            println!("🔤 Words: {}", content.word_count);
            println!("🔗 URL: {}", content.final_url);

        }
        Err(e) => eprintln!("\n❌ Error: {}", e),
    }

    Ok(())
}
