// src/api/routes.rs
use axum::{
    routing::{get, post, delete},
    Router,
};
use crate::api::{state::ApiState, handlers};

pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // === Публичные эндпоинты ===
        .route("/api/search", get(handlers::search::semantic_search))
        .route("/api/sites", get(handlers::sites::list_sites))
        .route("/api/sites/{site_key}", get(handlers::sites::get_site))
        .route("/api/sites/{site_key}/llms.txt", get(handlers::llms::get_llms_txt))

        // === Защищённые эндпоинты (требуют Bearer token) ===
        .route("/api/sites", post(handlers::sites::create_site))
        .route("/api/sites/{site_key}", delete(handlers::sites::delete_site))
        .route("/api/sites/{site_key}/crawl", post(handlers::crawl::trigger_crawl))
        .route("/api/sites/{site_key}/regenerate", post(handlers::llms::regenerate_llms))

        // === Админ-эндпоинты (требуют admin token) ===
        .route("/api/admin/clear", delete(handlers::admin::clear_database))
        .route("/api/admin/sites", get(handlers::admin::list_all_sites))

        // === Состояние ===
        .with_state(state)
}
