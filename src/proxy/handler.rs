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

    // Select a provider
    let provider = state.proxy.select_provider(allowed_providers.as_deref()).await;
    let provider = match provider {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to select provider: {}", e);
            return (StatusCode::SERVICE_UNAVAILABLE, "No provider available").into_response();
        }
    };

    // Read body bytes
    let body_bytes = axum::body::to_bytes(body, 10 * 1024 * 1024) // 10MB max
        .await
        .unwrap_or_default();

    // Extract model from request body
    let request_model = serde_json::from_slice::<serde_json::Value>(&body_bytes)
        .ok()
        .and_then(|v| v.get("model")?.as_str().map(|s| s.to_string()));

    // Try unified model routing: if model has mappings, replace model name and select mapped provider
    let mut final_body = body_bytes.clone();
    let mut final_provider = provider;
    
    if let Some(ref model_id) = request_model {
        // Check if this model has active mappings in model_mappings table
        // Use a timeout to avoid blocking
        let mappings_result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            state.db.list_active_model_mappings(model_id)
        ).await;
        
        if let Ok(Ok(mappings)) = mappings_result {
            if !mappings.is_empty() {
                // Filter by allowed providers
                let valid_mappings: Vec<_> = mappings.into_iter()
                    .filter(|m| {
                        let provider_allowed = allowed_providers.as_ref()
                            .map(|ap| ap.is_empty() || ap.contains(&m.provider_id))
                            .unwrap_or(true);
                        m.is_active && provider_allowed
                    })
                    .collect();

                if !valid_mappings.is_empty() {
                    // Use the first valid mapping (simple selection)
                    let selected = &valid_mappings[0];
                    
                    // Get the mapped provider
                    let provider_result = tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        state.db.get_provider(&selected.provider_id)
                    ).await;
                    
                    if let Ok(Ok(Some(mapped_provider))) = provider_result {
                        if mapped_provider.is_active {
                            // Replace model name in body
                            if let Ok(mut body_json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                                if let Some(obj) = body_json.as_object_mut() {
                                    obj.insert("model".to_string(), serde_json::Value::String(selected.provider_model_id.clone()));
                                    let new_body_str = serde_json::to_string(&body_json).unwrap_or_default();
                                    final_body = axum::body::Bytes::from(new_body_str);
                                }
                            }
                            final_provider = mapped_provider;
                        }
                    }
                }
            }
        }
    }

    // Check if this is a streaming request
    let is_streaming = serde_json::from_slice::<serde_json::Value>(&body_bytes)
        .ok()
        .and_then(|v| v.get("stream")?.as_bool())
        .unwrap_or(false);

    if is_streaming {
        // Streaming path: forward and stream the response
        let response = state.proxy.forward_streaming(
            &api_key_row.id,
            &final_provider,
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
        &final_provider,
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