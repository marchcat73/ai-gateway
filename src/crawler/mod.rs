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

        // 2. Fetch HTML
        let html = self.fetcher.fetch(url).await?;

        // 3. Parse & Extract
        let content = self.parser.parse(&html, url)?;

        Ok(content)
    }
}
