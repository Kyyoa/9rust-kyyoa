use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::config::SharedState;

pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok", "app": "9rust-kyyoa", "version": "0.1.0"})))
}

pub async fn models_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let cfg = state.config.read().await;
    let mut model_list = Vec::new();

    for (p_id, p_conf) in &cfg.providers {
        for m in &p_conf.models {
            model_list.push(json!({
                "id": format!("{}/{}", p_id, m),
                "object": "model",
                "created": 1700000000,
                "owned_by": p_id,
            }));
            model_list.push(json!({
                "id": m,
                "object": "model",
                "created": 1700000000,
                "owned_by": p_id,
            }));
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "object": "list",
            "data": model_list
        })),
    )
}

pub async fn chat_completions_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(mut payload): Json<Value>,
) -> Result<Response, (StatusCode, String)> {
    let cfg = state.config.read().await.clone();

    // 1. Auth check
    if !cfg.allowed_keys.is_empty() {
        let auth_hdr = headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let token = auth_hdr.strip_prefix("Bearer ").unwrap_or(auth_hdr).trim();
        if !cfg.allowed_keys.contains(&token.to_string()) {
            return Err((StatusCode::UNAUTHORIZED, "Invalid API Key for 9rust".to_string()));
        }
    }

    // 2. Parse model routing
    let model_req = payload
        .get("model")
        .and_then(|m| m.as_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing model field".to_string()))?
        .to_string();

    let (provider_id, target_model) = parse_routing(&model_req, &cfg.providers)?;
    let provider = cfg.providers.get(&provider_id).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!("Provider '{}' not found in 9rust config", provider_id),
        )
    })?.clone();

    // Update target model in payload
    if let Some(m_mut) = payload.get_mut("model") {
        *m_mut = Value::String(target_model.clone());
    }

    // 3. RTK Token Saver Hook
    if cfg.token_saver.enabled {
        let (b_before, b_after) = state.rtk.apply(
            &mut payload,
            cfg.token_saver.max_tool_result_chars,
            cfg.token_saver.compress_whitespace,
        );
        if b_before > b_after {
            info!(
                "[9rust RTK] Compressed payload: {}B -> {}B (saved {}B)",
                b_before,
                b_after,
                b_before - b_after
            );
        }
    }

    // 4. Build upstream request
    let is_stream = payload.get("stream").and_then(|s| s.as_bool()).unwrap_or(false);
    let upstream_url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));

    let mut req_builder = state
        .client
        .post(&upstream_url)
        .header("Authorization", format!("Bearer {}", provider.api_key))
        .header("Content-Type", "application/json");

    for (k, v) in &provider.custom_headers {
        req_builder = req_builder.header(k, v);
    }

    info!(
        "[9rust] Routing '{}' -> [{}] {} (stream={})",
        model_req, provider.name, target_model, is_stream
    );

    let res = req_builder
        .json(&payload)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Upstream connect error: {}", e)))?;

    let status = res.status();
    if !status.is_success() {
        let err_body = res.text().await.unwrap_or_default();
        warn!("[9rust] Upstream error {}: {}", status, err_body);
        return Err((
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            err_body,
        ));
    }

    if is_stream {
        let mut resp_builder = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive");

        let byte_stream = res.bytes_stream().map(|res_item| {
            res_item.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        });

        let body = Body::from_stream(byte_stream);
        resp_builder
            .body(body)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
    } else {
        let res_json: Value = res
            .json()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid upstream JSON: {}", e)))?;

        let mut resp_builder = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json");

        resp_builder
            .body(Body::from(res_json.to_string()))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
    }
}

fn parse_routing(
    requested: &str,
    providers: &HashMap<String, crate::config::ProviderConfig>,
) -> Result<(String, String), (StatusCode, String)> {
    if let Some((prefix, sub_model)) = requested.split_once('/') {
        if providers.contains_key(prefix) {
            return Ok((prefix.to_string(), sub_model.to_string()));
        }
    }

    for (p_id, p_conf) in providers {
        if p_conf.models.iter().any(|m| m == requested) {
            return Ok((p_id.clone(), requested.to_string()));
        }
    }

    if let Some((first_id, _)) = providers.iter().next() {
        Ok((first_id.clone(), requested.to_string()))
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "No providers configured in 9rust".to_string(),
        ))
    }
}
