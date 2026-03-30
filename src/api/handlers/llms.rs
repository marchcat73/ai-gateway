// src/api/handlers/llms.rs
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    body::Body,
};
use crate::api::state::ApiState;

pub async fn get_llms_txt(
    State(state): State<ApiState>,
    Path(site_key): Path<String>,
) -> Result<Response<Body>, StatusCode> {
    // Проверяем, существует ли сайт
    let _site = state.storage.get_site_by_key(&site_key).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Читаем файл
    let file_path = format!("public/{}/llms.txt", site_key);
    let content = tokio::fs::read_to_string(&file_path).await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Body::from(content))
        .unwrap();

    Ok(response)
}

pub async fn regenerate_llms(
    State(state): State<ApiState>,
    Path(site_key): Path<String>,
) -> Result<StatusCode, StatusCode> {
    // Проверяем, существует ли сайт
    let _site = state.storage.get_site_by_key(&site_key).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Регенерируем
    state.storage.generate_llms_for_site(&site_key, &format!("public/{}", site_key)).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}
