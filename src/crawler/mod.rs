pub mod types;
pub mod fetcher;
pub mod parser;

use types::{ExtractedContent, Result};
use fetcher::Fetcher;
use parser::Parser;

pub struct Crawler {
    fetcher: Fetcher,
    parser: Parser,
}

impl Default for Crawler {
    fn default() -> Self {
        Self::new()
    }
}

impl Crawler {
    pub fn new() -> Self {
        Self {
            fetcher: Fetcher::new(None),
            parser: Parser::new(),
        }
    }

    pub async fn crawl(&self, url: &str) -> Result<ExtractedContent> {
        // 1. Валидация URL
        let _ = url::Url::parse(url)
            .map_err(|e| types::CrawlerError::InvalidUrl(e.to_string()))?;

        // 2. Fetch HTML + получаем финальный URL
        let fetch_result = self.fetcher.fetch(url).await?;

        // 3. Parse & Extract (передаём final_url вместо исходного url)
        let content = self.parser.parse(&fetch_result.html, &fetch_result.final_url)?;

        Ok(content)
    }
}
