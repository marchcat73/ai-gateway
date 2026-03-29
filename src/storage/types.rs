// src/storage/types.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Site {
    pub id: Uuid,
    pub site_key: String,      // "newscryptonft.com"
    pub site_name: String,
    pub site_url: String,      // "https://newscryptonft.com"
    pub site_description: Option<String>,
    pub default_language: Option<String>,
    pub sitemap_url: Option<String>,
    pub crawl_enabled: Option<bool>,
    pub crawl_interval_hours: Option<i32>,
    pub include_patterns: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Site {
    /// Проверка: должен ли URL быть обработан для этого сайта
    pub fn should_include_url(&self, url: &str) -> bool {
        // Если есть include_patterns — проверяем их
        if let Some(patterns) = &self.include_patterns {
            if !patterns.is_empty() {
                return patterns.iter().any(|p| self.matches_pattern(url, p));
            }
        }

        // Если есть exclude_patterns — исключаем совпадения
        if let Some(patterns) = &self.exclude_patterns {
            if patterns.iter().any(|p| self.matches_pattern(url, p)) {
                return false;
            }
        }

        true
    }

    fn matches_pattern(&self, url: &str, pattern: &str) -> bool {
        // Простая glob-проверка (можно заменить на regex)
        let pattern = pattern.replace('*', ".*");
        match regex::Regex::new(&format!("^{}$", pattern)) {
            Ok(re) => re.is_match(url),
            Err(_) => false,
        }
    }
}
