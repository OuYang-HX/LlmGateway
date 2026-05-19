use axum::{
    body::Body,
    extract::{State, Request},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use regex::Regex;
use std::sync::LazyLock;
use crate::AppState;

/// Pre-compiled regex for detecting rate-limit / quota-exhausted errors.
static RATE_LIMIT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)rate.?limit|insufficient.?quota|quota.?exceed|balance.?insufficient|credits?.deplet|NotEnough|capacity.?exceed|throttl|too.?many.?requests|usage.?limit|billing.?limit|plan.?limit|exceeds?.limit|limit.?exceed").unwrap()
});

/// Check if an error message indicates a rate-limit / quota-exhausted / throttling error
/// that should be converted to HTTP 429 so that clients (like pi-coding-agent) auto-retry.
pub fn is_rate_limit_error(message: &str) -> bool {
    // Common patterns from various providers:
    // - Xunfei: NotEnoughCvError, code: 11210
    // - OpenAI: rate_limit_exceeded, insufficient_quota
    // - Generic: quota exceeded, balance insufficient, credits depleted, etc.
    RATE_LIMIT_REGEX.is_match(message)
}

/// Build a standard OpenAI-compatible error response JSON for rate limit errors.
/// The message includes "rate_limit" keyword so that pi-coding-agent's _isRetryableError regex matches.
fn build_rate_limit_error_json(original_message: &str) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "message": format!("rate_limit error from upstream provider: {}", original_message),
            "type": "rate_limit_error",
            "code": "rate_limit_exceeded"
        }
    })
}

/// Build a standard OpenAI-compatible error response JSON for server errors.
fn build_server_error_json(original_message: &str) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "message": format!("server_error from upstream provider: {}", original_message),
            "type": "server_error",
            "code": "upstream_error"
        }
    })
}

/// Strip thinking/reasoning tags from LLM response content (e.g. <think>...</think>, <tool_call>...</think>)
pub fn strip_thinking_tags(content: &str) -> String {
    // Strip <think>...</think> tags (may appear multiple times)
    let re_think = Regex::new(r"<think>[\s\S]*?<\/think>").unwrap();
    // Strip <tool_call>...</think> tags
    let re_tool = Regex::new(r"<tool_call>[\s\S]*?<\/think>").unwrap();
    let mut result = re_think.replace_all(content, "").to_string();
    result = re_tool.replace_all(&result, "").to_string();
    // For thinking content that spans across accumulated chunks without proper closing:
    // MiniMax pattern: <tool_call>...\n\nActualResponse
    // If content starts with thinking marker, strip everything up to first \n\n
    let re_thought_start = Regex::new(r"^(<tool_call>[^\n]*\n)[\s\S]*?\n\n").unwrap();
    result = re_thought_start.replace(&result, "").to_string();
    // Compress multiple newlines to a single one
    let re_newlines = Regex::new(r"\n{2,}").unwrap();
    result = re_newlines.replace_all(&result, "\n").to_string();
    result.trim().to_string()
}

/// Standard OpenAI-compatible /v1/models endpoint.
/// Lists all active unified models that are accessible by the given API key.
async fn list_models_for_api_key(
    state: &AppState,
    _api_key_row: &crate::db::ApiKeyRow,
    allowed_providers: &Option<Vec<String>>,
) -> impl IntoResponse {
    let models = match state.db.list_models_with_mappings().await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to list models for /v1/models: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let now = chrono::Utc::now().timestamp();
    let data: Vec<serde_json::Value> = models
        .into_iter()
        .filter(|model| model.model.is_active)
        .filter_map(|model| {
            let has_valid_mapping = model.mappings.iter().any(|m| {
                let provider_ok = allowed_providers.as_ref()
                    .map(|ap| ap.is_empty() || ap.contains(&m.mapping.provider_id))
                    .unwrap_or(true);
                m.mapping.is_active && provider_ok
            });
            if !has_valid_mapping {
                return None;
            }
            Some(serde_json::json!({
                "id": model.model.id,
                "object": "model",
                "created": now,
                "owned_by": model.model.name,
                "permission": [],
                "root": model.model.id,
            }))
        })
        .collect();

    (StatusCode::OK, Json(serde_json::json!({
        "object": "list",
        "data": data,
    }))).into_response()
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

    // Determine allowed providers for this API key
    let allowed_providers: Option<Vec<String>> = api_key_row.allowed_providers
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok());

    // === Standard /v1/models API (OpenAI-compatible) ===
    if path == "/v1/models" && method == "GET" {
        return list_models_for_api_key(&state, &api_key_row, &allowed_providers).await.into_response();
    }

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
                let upstream_status = upstream_response.status(); // reqwest::StatusCode

                // === Handle non-OK upstream responses ===
                // When upstream returns an error HTTP status, we read the body,
                // check for rate-limit/quota errors, and convert to 429 or 503
                // so that clients (like pi-coding-agent) will auto-retry.
                if !upstream_status.is_success() {
                    let status_code = upstream_status.as_u16() as i32;
                    let error_body = upstream_response.bytes().await.unwrap_or_default();
                    let error_text = String::from_utf8_lossy(&error_body).to_string();

                    // Check if this is a rate-limit / quota error that should become 429
                    let (final_status, final_body) = if is_rate_limit_error(&error_text) {
                        tracing::warn!(
                            "Upstream rate-limit/quota error (status {}), converting to 429: {}",
                            status_code,
                            &error_text[..error_text.len().min(200)]
                        );
                        let error_json = build_rate_limit_error_json(&error_text);
                        (StatusCode::TOO_MANY_REQUESTS, serde_json::to_string(&error_json).unwrap_or_default())
                    } else if status_code >= 500 {
                        // Server errors: convert to 503 so clients retry
                        tracing::warn!(
                            "Upstream server error (status {}), converting to 503: {}",
                            status_code,
                            &error_text[..error_text.len().min(200)]
                        );
                        let error_json = build_server_error_json(&error_text);
                        (StatusCode::SERVICE_UNAVAILABLE, serde_json::to_string(&error_json).unwrap_or_default())
                    } else {
                        // Other client errors (400, 401, 403, 404, etc.): pass through as-is
                        (upstream_status, error_text)
                    };

                    return (
                        final_status,
                        [("content-type", "application/json")],
                        final_body,
                    ).into_response();
                }

                // === Upstream returned 200 OK — stream SSE to client ===
                let stream = upstream_response.bytes_stream();
                let (tx, rx) = tokio::sync::mpsc::channel(100);

                // Clone needed data for the collector task
                let db = state.db.clone();
                let stats_collector = state.stats_collector.clone();
                let api_key_id = api_key_row.id.clone();
                let provider_id = provider.id.clone();
                let log_path = path.clone();
                let log_model = model.id.clone();
                let request_body_str = if !body_bytes.is_empty() { Some(String::from_utf8_lossy(&body_bytes).to_string()) } else { None };

                // Spawn a task that collects all chunks for logging while forwarding to client
                tokio::spawn(async move {
                    use futures::StreamExt;
                    let mut stream = Box::pin(stream);
                    let mut all_delta_content = String::new();
                    let mut prompt_tokens: i64 = 0;
                    let mut completion_tokens: i64 = 0;
                    let mut total_tokens: i64 = 0;
                    let mut response_id: Option<String> = None;
                    let mut response_model: Option<String> = None;
                    let mut finish_reason: Option<String> = None;
                    let start = std::time::Instant::now();
                    let mut raw_sse = String::new();
                    // Track if we detected a rate-limit/quota error inside the SSE stream
                    let mut sse_rate_limit_error: Option<String> = None;

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                let chunk_str = String::from_utf8_lossy(&bytes).to_string();
                                raw_sse.push_str(&chunk_str);

                                // Parse SSE lines and accumulate content
                                for line in chunk_str.split('\n') {
                                    let trimmed = line.trim();
                                    if !trimmed.starts_with("data: ") { continue; }
                                    let json_str = &trimmed[6..];
                                    if json_str == "[DONE]" { continue; }

                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        // Check for error objects in SSE stream
                                        // Some providers return HTTP 200 but include errors in the stream
                                        if let Some(error_obj) = v.get("error") {
                                            let error_msg = error_obj.get("message")
                                                .and_then(|m| m.as_str())
                                                .unwrap_or("");
                                            let error_code = error_obj.get("code")
                                                .and_then(|c| c.as_str())
                                                .unwrap_or("");
                                            let error_str = format!("{} {}", error_code, error_msg);
                                            if is_rate_limit_error(&error_str) {
                                                sse_rate_limit_error = Some(error_msg.to_string());
                                            } else {
                                                // Non-rate-limit upstream error in SSE stream (e.g. Engine Busy)
                                                // Record it so we can log with 503 status and notify the client
                                                sse_rate_limit_error = Some(format!("upstream_error: {}", error_msg));
                                            }
                                        }

                                        // Extract usage from final chunk
                                        if let Some(u) = v.get("usage") {
                                            prompt_tokens = u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                                            completion_tokens = u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                                            total_tokens = u.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(prompt_tokens + completion_tokens);
                                        }
                                        // Extract ID and model from first chunk
                                        if response_id.is_none() {
                                            response_id = v.get("id").and_then(|v| v.as_str()).map(String::from);
                                        }
                                        if response_model.is_none() {
                                            response_model = v.get("model").and_then(|v| v.as_str()).map(String::from);
                                        }
                                        // Accumulate delta content
                                        if let Some(choices) = v.get("choices").and_then(|v| v.as_array()) {
                                            for choice in choices {
                                                if let Some(delta) = choice.get("delta") {
                                                    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                                                        all_delta_content.push_str(content);
                                                    }
                                                }
                                                // Extract finish_reason if present
                                                if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                                                    if finish_reason.is_none() {
                                                        finish_reason = Some(fr.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // If we detected an error in the SSE stream, stop forwarding
                                // the raw upstream error and instead send a standardized error event
                                // so the client can properly detect and handle it.
                                if sse_rate_limit_error.is_some() {
                                    // Don't forward the raw error chunk to client
                                    // Instead, send a proper error SSE event
                                    let is_rate_limit = !sse_rate_limit_error.as_deref().unwrap_or("").starts_with("upstream_error:");
                                    let error_event = if is_rate_limit {
                                        let err_json = build_rate_limit_error_json(sse_rate_limit_error.as_deref().unwrap_or(""));
                                        format!("data: {}\n\ndata: [DONE]\n\n", err_json)
                                    } else {
                                        let err_msg = sse_rate_limit_error.as_deref().unwrap_or("").strip_prefix("upstream_error: ").unwrap_or("");
                                        let err_json = build_server_error_json(err_msg);
                                        format!("data: {}\n\ndata: [DONE]\n\n", err_json)
                                    };
                                    let _ = tx.send(Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(error_event))).await;
                                    break;
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

                    // Stream finished - build reconstructed non-streaming response
                    let duration_ms = start.elapsed().as_millis() as i64;
                    let final_model = response_model.clone().or(Some(log_model));
                    // Strip thinking tags from accumulated delta content
                    let clean_content = strip_thinking_tags(&all_delta_content);

                    // If we detected a rate-limit error in the SSE stream and got no useful content,
                    // log it with a warning so the operator knows the upstream is throttling
                    if let Some(ref err) = sse_rate_limit_error {
                        tracing::warn!(
                            "Rate-limit error detected in SSE stream (no content produced): {}",
                            err
                        );
                    }

                    // Build a non-streaming style response body for QA
                    let reconstructed = serde_json::json!({
                        "id": response_id.unwrap_or_else(|| format!("chatcmpl-{}", uuid::Uuid::new_v4())),
                        "model": final_model.as_deref().unwrap_or("unknown"),
                        "choices": [{
                            "finish_reason": finish_reason.unwrap_or_else(|| "stop".to_string()),
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": clean_content
                            }
                        }],
                        "usage": {
                            "prompt_tokens": prompt_tokens,
                            "completion_tokens": completion_tokens,
                            "total_tokens": total_tokens
                        }
                    });

                    let log_status_code = if sse_rate_limit_error.is_some() {
                        if sse_rate_limit_error.as_deref().unwrap_or("").starts_with("upstream_error:") {
                            503i32  // Upstream server error (e.g. Engine Busy)
                        } else {
                            429i32  // Rate limit error
                        }
                    } else {
                        200i32
                    };
                    let _ = db.insert_request_log(
                        &api_key_id,
                        &provider_id,
                        final_model.as_deref(),
                        &log_path,
                        "POST",
                        None,
                        request_body_str.as_deref(),
                        Some(log_status_code),
                        None,
                        Some(&reconstructed.to_string()),
                        prompt_tokens,
                        completion_tokens,
                        total_tokens,
                        Some(duration_ms),
                        true,
                        sse_rate_limit_error.is_some(),
                        sse_rate_limit_error.as_deref(),
                    ).await;

                    // Record usage for token rate tracking
                    let _ = stats_collector.record_usage(&provider.id, prompt_tokens, completion_tokens).await;
                });

                let body_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
                let response = Response::builder()
                    .status(upstream_status)
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
            let body_text = String::from_utf8_lossy(&proxy_response.body).to_string();

            // Check if the response body contains a rate-limit / quota error
            // even if the HTTP status code is not 429.
            // Some providers return 200 or other codes with error details in the body.
            let is_rate_limit = is_rate_limit_error(&body_text);

            let (final_status, final_body) = if is_rate_limit && proxy_response.status_code != 429 {
                tracing::warn!(
                    "Non-streaming rate-limit/quota error detected in response body (status {}), converting to 429: {}",
                    proxy_response.status_code,
                    &body_text[..body_text.len().min(200)]
                );
                let error_json = build_rate_limit_error_json(&body_text);
                (StatusCode::TOO_MANY_REQUESTS, serde_json::to_string(&error_json).unwrap_or_default().into_bytes())
            } else if proxy_response.status_code >= 500 && proxy_response.status_code != 429 {
                // Server errors: convert to 503 so clients retry
                tracing::warn!(
                    "Non-streaming server error (status {}), converting to 503: {}",
                    proxy_response.status_code,
                    &body_text[..body_text.len().min(200)]
                );
                let error_json = build_server_error_json(&body_text);
                (StatusCode::SERVICE_UNAVAILABLE, serde_json::to_string(&error_json).unwrap_or_default().into_bytes())
            } else {
                let status = StatusCode::from_u16(proxy_response.status_code as u16)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                (status, proxy_response.body.to_vec())
            };

            let mut builder = Response::builder().status(final_status);
            // Set content-type to application/json for error responses we generated
            // (rate-limit conversions and server-error conversions)
            if (is_rate_limit && proxy_response.status_code != 429) || (proxy_response.status_code >= 500 && proxy_response.status_code != 429) {
                builder = builder.header("content-type", "application/json");
            } else if let Some(content_type) = proxy_response.content_type {
                builder = builder.header("content-type", content_type);
            }
            builder.body(Body::from(final_body)).unwrap().into_response()
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
