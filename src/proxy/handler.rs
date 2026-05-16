use axum::{
    body::Body,
    extract::{State, Request},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use crate::AppState;

/// Handler for proxying LLM requests
pub async fn proxy_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> impl IntoResponse {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path().to_string();
    let method = parts.method.as_str().to_string();

    // Extract API key from Authorization header
    let api_key = extract_api_key(&headers);
    if api_key.is_none() {
        return (StatusCode::UNAUTHORIZED, "Missing API key").into_response();
    }
    let api_key = api_key.unwrap();

    // Validate API key (plaintext lookup)
    let api_key_row = state.db.get_api_key_by_key(&api_key).await;

    let api_key_row = match api_key_row {
        Ok(Some(row)) => row,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response(),
        Err(e) => {
            tracing::error!("Failed to validate API key: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    // Determine allowed providers for this API key
    let allowed_providers: Option<Vec<String>> = api_key_row.allowed_providers
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok());

    // Read body bytes
    let body_bytes = axum::body::to_bytes(body, 10 * 1024 * 1024) // 10MB max
        .await
        .unwrap_or_default();

    // Extract model from request body
    let request_model = serde_json::from_slice::<serde_json::Value>(&body_bytes)
        .ok()
        .and_then(|v| v.get("model")?.as_str().map(|s| s.to_string()));

    // === Unified model routing ===
    // The model ID in the request MUST be a unified model ID (exists in models table).
    // Provider model IDs (like "MiniMax-M2.7-highspeed") are internal and cannot be used directly.
    // The gateway will look up the unified model's mappings to find the provider and internal model ID.
    
    let request_model_id = match request_model {
        Some(id) => id,
        None => {
            return (StatusCode::BAD_REQUEST, "Missing 'model' field in request body").into_response();
        }
    };

    // Look up the unified model
    let model_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state.db.get_model(&request_model_id)
    ).await;

    let model = match model_result {
        Ok(Ok(Some(m))) if m.is_active => m,
        Ok(Ok(Some(_))) => {
            return (StatusCode::FORBIDDEN, 
                format!("Model '{}' is inactive. Please use an active model.", request_model_id)
            ).into_response();
        }
        Ok(Ok(None)) => {
            return (StatusCode::NOT_FOUND, 
                format!("Model '{}' not found. You must use a unified model ID (对外模型ID), not a provider model ID.", request_model_id)
            ).into_response();
        }
        _ => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to look up model").into_response();
        }
    };

    // Get active mappings for this unified model
    let mappings_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state.db.list_active_model_mappings(&request_model_id)
    ).await;

    let mappings = match mappings_result {
        Ok(Ok(m)) => m,
        _ => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to look up model mappings").into_response();
        }
    };

    if mappings.is_empty() {
        return (StatusCode::NOT_FOUND, 
            format!("Model '{}' has no provider mappings configured. Please configure a provider mapping for this model.", request_model_id)
        ).into_response();
    }

    // Filter mappings by allowed providers
    let valid_mappings: Vec<_> = mappings.into_iter()
        .filter(|m| {
            let provider_allowed = allowed_providers.as_ref()
                .map(|ap| ap.is_empty() || ap.contains(&m.provider_id))
                .unwrap_or(true);
            m.is_active && provider_allowed
        })
        .collect();

    if valid_mappings.is_empty() {
        return (StatusCode::FORBIDDEN, 
            format!("Model '{}' has no available provider mappings for this API key.", request_model_id)
        ).into_response();
    }

    // Select a mapping (weighted random selection)
    let total_weight: i64 = valid_mappings.iter().map(|m| m.weight).sum();
    if total_weight <= 0 {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Model mappings have invalid weights").into_response();
    }
    
    // Simple selection: use first valid mapping for now
    // TODO: implement weighted random selection
    let selected_mapping = &valid_mappings[0];

    // Get the mapped provider
    let provider_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state.db.get_provider(&selected_mapping.provider_id)
    ).await;

    let provider = match provider_result {
        Ok(Ok(Some(p))) if p.is_active => p,
        Ok(Ok(Some(_))) => {
            return (StatusCode::SERVICE_UNAVAILABLE, 
                format!("Provider '{}' is inactive.", selected_mapping.provider_id)
            ).into_response();
        }
        _ => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to look up provider").into_response();
        }
    };

    // Replace model name in body and inject stream_options for streaming
    let is_streaming = serde_json::from_slice::<serde_json::Value>(&body_bytes)
        .ok()
        .and_then(|v| v.get("stream")?.as_bool())
        .unwrap_or(false);

    let final_body = if let Ok(mut body_json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
        if let Some(obj) = body_json.as_object_mut() {
            obj.insert("model".to_string(), serde_json::Value::String(selected_mapping.provider_model_id.clone()));
            // For streaming, inject stream_options to get usage stats in final SSE chunk
            if is_streaming {
                obj.insert("stream_options".to_string(), serde_json::json!({"include_usage": true}));
            }
        }
        axum::body::Bytes::from(serde_json::to_string(&body_json).unwrap_or_default())
    } else {
        body_bytes.clone()
    };

    if is_streaming {
        // Streaming path: forward, collect chunks, and stream to client
        let response = state.proxy.forward_streaming_no_log(
            &provider,
            &path,
            &method,
            headers,
            final_body,
        ).await;

        match response {
            Ok(upstream_response) => {
                let status = upstream_response.status();
                let status_code = status.as_u16() as i32;
                let is_throttled = status_code == 429;
                let stream = upstream_response.bytes_stream();
                let (tx, rx) = tokio::sync::mpsc::channel(100);

                // Clone needed data for the collector task
                let db = state.db.clone();
                let api_key_id = api_key_row.id.clone();
                let provider_id = provider.id.clone();
                let log_path = path.clone();
                let log_model = model.id.clone();
                let request_body_str = if !body_bytes.is_empty() { Some(String::from_utf8_lossy(&body_bytes).to_string()) } else { None };

                // Spawn a task that collects all chunks for logging while forwarding to client
                tokio::spawn(async move {
                    use futures::StreamExt;
                    let mut stream = Box::pin(stream);
                    let mut collected_bytes = Vec::new();
                    let mut prompt_tokens: i64 = 0;
                    let mut completion_tokens: i64 = 0;
                    let mut total_tokens: i64 = 0;
                    let mut response_model: Option<String> = None;
                    let start = std::time::Instant::now();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                collected_bytes.extend_from_slice(&bytes);
                                // Try to extract usage from SSE chunks
                                let chunk_str = String::from_utf8_lossy(&bytes);
                                for line in chunk_str.split('\n') {
                                    let (p, c, t) = crate::usage::extract_streaming_usage(line);
                                    if p > 0 || c > 0 || t > 0 {
                                        prompt_tokens = p;
                                        completion_tokens = c;
                                        total_tokens = t;
                                    }
                                    // Try to extract model from streaming chunks
                                    if response_model.is_none() {
                                        let json_str = line.trim().strip_prefix("data: ").unwrap_or(line.trim());
                                        if json_str != "[DONE]" {
                                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                                                response_model = crate::usage::extract_model_from_response(&v);
                                            }
                                        }
                                    }
                                }
                                if tx.send(Ok::<_, std::convert::Infallible>(bytes)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Stream error: {}", e);
                                break;
                            }
                        }
                    }

                    // Stream finished - log the complete request
                    let duration_ms = start.elapsed().as_millis() as i64;
                    let final_model = response_model.or(Some(log_model));
                    let response_body_str = if !collected_bytes.is_empty() { Some(String::from_utf8_lossy(&collected_bytes).to_string()) } else { None };

                    let _ = db.insert_request_log(
                        &api_key_id,
                        &provider_id,
                        final_model.as_deref(),
                        &log_path,
                        "POST",
                        None,
                        request_body_str.as_deref(),
                        Some(status_code),
                        None,
                        response_body_str.as_deref(),
                        prompt_tokens,
                        completion_tokens,
                        total_tokens,
                        Some(duration_ms),
                        true,
                        is_throttled,
                        None,
                    ).await;
                });

                let body_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
                let response = Response::builder()
                    .status(status)
                    .header("content-type", "text/event-stream")
                    .header("cache-control", "no-cache")
                    .header("connection", "keep-alive")
                    .body(Body::from_stream(body_stream))
                    .unwrap();

                return response.into_response();
            }
            Err(e) => {
                tracing::error!("Proxy error: {}", e);
                return (StatusCode::BAD_GATEWAY, format!("Upstream error: {}", e)).into_response();
            }
        }
    }

    // Non-streaming path: forward, extract usage, and return
    let response = state.proxy.forward_and_collect(
        &api_key_row.id,
        &provider,
        &path,
        &method,
        headers,
        final_body,
    ).await;

    match response {
        Ok(proxy_response) => {
            let mut builder = Response::builder().status(StatusCode::from_u16(proxy_response.status_code as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
            if let Some(content_type) = proxy_response.content_type {
                builder = builder.header("content-type", content_type);
            }
            builder.body(Body::from(proxy_response.body)).unwrap().into_response()
        }
        Err(e) => {
            tracing::error!("Proxy error: {}", e);
            (StatusCode::BAD_GATEWAY, format!("Upstream error: {}", e)).into_response()
        }
    }
}

/// Extract API key from Authorization header
fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    let auth_header = headers.get("authorization")?.to_str().ok()?;
    if auth_header.starts_with("Bearer ") {
        Some(auth_header[7..].to_string())
    } else {
        Some(auth_header.to_string())
    }
}
