// src/mcp_server/server.rs
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use std::sync::Arc;
use crate::api::state::ApiState;
use crate::storage::ContentStorage;

/// MCP-сервер: JSON-RPC 2.0 over stdio
pub struct McpServer {
    state: Arc<ApiState>,
}

impl McpServer {
    pub fn new(state: ApiState) -> Self {
        Self {
            state: Arc::new(state),
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        tracing::debug!("🔧 MCP server started, waiting for stdin...");
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        let mut reader = BufReader::new(stdin);
        let mut writer = BufWriter::new(stdout);

        let mut line = String::new();
        while reader.read_line(&mut line).await? > 0 {
            let request: Value = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    self.send_error(&mut writer, None, format!("Parse error: {}", e)).await?;
                    line.clear();
                    continue;
                }
            };

            let response = self.handle_request(request).await;

            let response_str = serde_json::to_string(&response)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "JSON serialization failed"))?;
            writer.write_all(response_str.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;

            line.clear();

            tracing::debug!("📤 Sent response");
        }
        tracing::warn!("⚠️  MCP server: stdin closed, exiting");
        Ok(())
    }

    async fn handle_request(&self, request: Value) -> Value {
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            "search_semantic" => self.tool_search_semantic(request).await,
            "get_llms_txt" => self.tool_get_llms_txt(request).await,
            "clear_database" => self.tool_clear_database(request).await,
            "initialize" => self.mcp_initialize(request).await,
            "notifications/initialized" => json!({"jsonrpc": "2.0", "result": {}}),
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            }),

                // Временно заглушки для нереализованных:
            "get_document" | "get_chunk" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").cloned(),
                    "error": { "code": -32601, "message": "Not implemented yet" }
                })
            }

            _ => json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned(),
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            }),
        }
    }

    /// Tool: search_semantic
    async fn tool_search_semantic(&self, request: Value) -> Value {
        let id = request.get("id").cloned();
        let default_params = serde_json::Value::Object(serde_json::Map::new());
        let params = request.get("params").unwrap_or(&default_params);

        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let limit: usize = params.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize).unwrap_or(10);
        let site_key = params.get("site_key").and_then(|v| v.as_str());

        let chunks = match &site_key {
            Some(key) => self.state.storage.search_semantic_by_site(query, key, limit).await,
            None => self.state.storage.search_semantic(query, limit).await,
        };

        match chunks {
            Ok(results) => {
                // ✅ ИСПРАВЛЕНО: аннотация типа + .collect()
                let data: Vec<Value> = results.into_iter().map(|c| json!({
                    "id": c.id,
                    "url": c.source_url,
                    "title": c.title,
                    "content": c.content,
                    "chunk_index": c.chunk_index,
                })).collect();

                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "chunks": data }
                })
            }
            Err(e) => self.send_error_json(id, format!("Search failed: {}", e)),
        }
    }

    /// Tool: get_llms_txt
    async fn tool_get_llms_txt(&self, request: Value) -> Value {
        let id = request.get("id").cloned();
        let default_params = serde_json::Value::Object(serde_json::Map::new());
        let params = request.get("params").unwrap_or(&default_params);
        let site_key = match params.get("site_key").and_then(|v| v.as_str()) {
            Some(k) => k,
            None => return self.send_error_json(id, "Missing required parameter: site_key".to_string()),
        };

        match self.state.storage.generate_llms_for_site(site_key, "/tmp").await {
            Ok(result) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": result.content,
                    "pages": result.pages_count,
                    "chunks": result.chunks_count
                }
            }),
            Err(e) => self.send_error_json(id, format!("Failed to generate llms.txt: {}", e)),
        }
    }

    /// Tool: clear_database (protected)
/// Tool: clear_database (protected)
async fn tool_clear_database(&self, request: Value) -> Value {
    let id = request.get("id").cloned();
    let default_params = serde_json::Value::Object(serde_json::Map::new());
    let params = request.get("params").unwrap_or(&default_params);

    let token = params.get("admin_token").and_then(|v| v.as_str()).unwrap_or("");
    if token != self.state.admin_token {
        return self.send_error_json(id, "Unauthorized: invalid admin token".to_string());
    }

    // ✅ ИСПРАВЛЕНО: pool() уже возвращает &PgPool, не нужно &
    match sqlx::query("TRUNCATE embedding_cache, chunks, documents, sites RESTART IDENTITY CASCADE")
        .execute(self.state.storage.pool())  // ← Без лишнего &
        .await
    {
        Ok(_) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "message": "Database cleared" }
        }),
        Err(e) => self.send_error_json(id, format!("Clear failed: {}", e)),
    }
}

    /// MCP initialize
    async fn mcp_initialize(&self, request: Value) -> Value {
        let id = request.get("id").cloned();
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "list": [
                            { "name": "search_semantic", "description": "Semantic search across indexed content" },
                            { "name": "get_document", "description": "Get full document by URL" },
                            { "name": "get_chunk", "description": "Get specific content chunk by ID" },
                            { "name": "get_llms_txt", "description": "Get llms.txt for a site" }
                        ]
                    }
                },
                "serverInfo": { "name": "ai-gateway", "version": "0.1.0" }
            }
        })
    }

    async fn send_error(
        &self,
        writer: &mut BufWriter<tokio::io::Stdout>,
        id: Option<Value>,
        message: String
    ) -> std::io::Result<()> {
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32603,
                "message": message
            }
        });

        // Сериализуем сразу в байты
        let response_bytes = serde_json::to_vec(&response)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "JSON serialization failed"))?;

        writer.write_all(&response_bytes).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await
    }

    fn send_error_json(&self, id: Option<Value>, message: impl Into<String>) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32603, "message": message.into() }
        })
    }
}
