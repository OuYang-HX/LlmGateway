use axum::{
    body::Body,
    extract::{State, Request},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use crate::AppState;

/// Unified model routing info
pub struct UnifiedModelRoute {
    pub unified_model_id: String,
    pub provider_model_id: String,
}

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

    // Read body bytes first to check for unified model
    let body_bytes = axum::body::to_bytes(body, 10 * 1024 * 1024) // 10MB max
        .await
        .unwrap_or_default();

    // Parse body to check if model is a unified model ID
    let mut request_body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap_or(serde_json::Value::Null);
    let mut unified_route: Option<UnifiedModelRoute> = None;
    let mut provider_for_request = None;
    let mut needs_model_replace = None;

    if let Some(model_value) = request_body.get("model").and_then(|v| v.as_str()) {
        // Check if this model is a unified model ID
        if let Some(model_info) = state.db.get_model_with_mappings(model_value).await.ok().flatten() {
            if model_info.model.is_active && !model_info.mappings.is_empty() {
                // Get allowed providers
                let allowed_providers: Option<Vec<String>> = api_key_row.allowed_providers
                    .as_ref()
                    .and_then(|s| serde_json::from_str(s).ok());
                
                // Filter mappings by allowed providers and active status
                let valid_mappings: Vec<_> = model_info.mappings.iter()
                    .filter(|m| {
                        let provider_allowed = allowed_providers.as_ref()
                            .map(|ap| ap.is_empty() || ap.contains(&m.provider.id))
                            .unwrap_or(true);
                        m.mapping.is_active && m.provider.is_active && provider_allowed
                    })
                    .collect();

                if !valid_mappings.is_empty() {
                    // Weighted random selection
                    let total_weight: i64 = valid_mappings.iter().map(|m| m.mapping.weight).sum();
                    let random_val = (rand_simple() * total_weight as f64) as i64;
                    let mut acc = 0i64;
                    
                    for m in &valid_mappings {
                        acc += m.mapping.weight;
                        if acc >= random_val {
                            unified_route = Some(UnifiedModelRoute {
                                unified_model_id: model_value.to_string(),
                                provider_model_id: m.mapping.provider_model_id.clone(),
                            });
                            provider_for_request = Some(m.provider.clone());
                            needs_model_replace = Some(m.mapping.provider_model_id.clone());
                            tracing::info!("Routing unified model '{}' to provider '{}' with model '{}'", 
                                model_value, m.provider.id, m.mapping.provider_model_id);
                            break;
                        }
                    }
                }
            }
        }
    }
    
    // Apply model replacement if needed
    if let Some(new_model_id) = needs_model_replace {
        if let Some(obj) = request_body.as_object_mut() {
            obj.insert("model".to_string(), serde_json::Value::String(new_model_id));
        }
    }

    // If no unified model routing, use traditional provider selection
    if provider_for_request.is_none() {
        let allowed_providers: Option<Vec<String>> = api_key_row.allowed_providers
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        let provider = state.proxy.select_provider(allowed_providers.as_deref()).await;
        provider_for_request = match provider {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::error!("Failed to select provider: {}", e);
                return (StatusCode::SERVICE_UNAVAILABLE, "No provider available").into_response();
            }
        };
    }

    let provider = provider_for_request.unwrap();

    // Validate model against provider's allowed models (if not unified route)
    let request_model = request_body.get("model").and_then(|v| v.as_str());
    if unified_route.is_none() {
        if let Some(model) = request_model {
            match state.db.is_model_allowed_for_provider(&provider.id, model).await {
                Ok(false) => {
                    tracing::warn!("Model '{}' not allowed for provider '{}'", model, provider.id);
                    return (StatusCode::FORBIDDEN, format!("Model '{}' is not available on provider '{}'", model, provider.id)).into_response();
                }
                Err(e) => {
                    tracing::error!("Failed to check model permission: {}", e);
                    // Allow on DB error to avoid blocking requests
                }
                Ok(true) => {}
            }
        }
    }

    // Serialize modified body if unified model routing was applied
    let final_body: axum::body::Bytes = if request_body != serde_json::Value::Null {
        serde_json::to_vec(&request_body)
            .map(|v| v.into())
            .unwrap_or_else(|_| body_bytes.clone())
    } else {
        body_bytes.clone()
    };

    // Check if this is a streaming request
    let is_streaming = request_body.get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if is_streaming {
        // Streaming path: forward and stream the response
        let response = state.proxy.forward_streaming(
            &api_key_row.id,
            &provider,
            &path,
            &method,
            headers,
            final_body,
        ).await;

        match response {
            Ok(upstream_response) => {
                let status = upstream_response.status();
                let stream = upstream_response.bytes_stream();
                let (tx, rx) = tokio::sync::mpsc::channel(100);
                tokio::spawn(async move {
                    use futures::StreamExt;
                    let mut stream = Box::pin(stream);
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
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

/// Simple random number generator (0-1)
fn rand_simple() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos as f64) / (u32::MAX as f64)
}
