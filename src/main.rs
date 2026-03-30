// src/main.rs
use ai_gateway::storage::PostgresStorage;
use ai_gateway::api::{state::ApiState, routes::create_router};
use ai_gateway::mcp_server::server::McpServer;  // ← Импортируем MCP-сервер
use axum::serve;
use tokio::net::TcpListener;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("ai_gateway=debug,info")
        .init();

    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let admin_token = std::env::var("ADMIN_TOKEN")
        .unwrap_or_else(|_| "change-me-in-prod".to_string());

    // Инициализация хранилища
    let storage = PostgresStorage::connect(&database_url).await?;

    // Shared state для API и MCP
    let api_state = ApiState::new(storage, admin_token);

    // Запуск режимов
    let mode = std::env::var("MODE").unwrap_or_else(|_| "api".to_string());

    match mode.as_str() {
        "api" => {
            // === REST API сервер ===
            let app = create_router(api_state);
            let listener = TcpListener::bind("0.0.0.0:3000").await?;

            tracing::info!("🚀 Starting REST API on http://0.0.0.0:3000");
            serve(listener, app).await?;
        }

        "mcp" => {
            // === MCP сервер (stdio) ===
            tracing::info!("🤖 Starting MCP server (stdio)");

            let mcp = McpServer::new(api_state);
            mcp.run().await?;
        }

        "both" => {
            // === Оба режима параллельно ===
            let api_state_clone = api_state.clone();

            // API в фоне
            let api_handle = tokio::spawn(async move {
                let app = create_router(api_state_clone);
                let listener = TcpListener::bind("0.0.0.0:3000").await?;
                serve(listener, app).await
            });

            // MCP в основном потоке
            let mcp = McpServer::new(api_state);
            mcp.run().await?;

            // Ждём завершения API (если оно завершится)
            api_handle.await??;
        }

        _ => {
            anyhow::bail!("Unknown mode: {}. Use 'api', 'mcp', or 'both'", mode);
        }
    }

    Ok(())
}
