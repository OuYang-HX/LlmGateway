use axum::{
    body::Body,
    extract::{State, Request},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use regex::Regex;
use std::sync::LazyLock;
use crate::{AppState, db::ModelMappingRow};

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
pub fn build_rate_limit_error_json(original_message: &str) -> serde_json::Value {
    // Sanitize: remove "quota exceeded" from upstream message to avoid
    // pi-coding-agent's _isNonRetryableProviderLimitError from blocking retry.
    // The HTTP 429 + "rate_limit" keyword in this response is sufficient for retry detection.
    let sanitized = original_message
        .replace("quota exceeded", "rate limit triggered")
        .replace("quota exceed", "rate limit triggered")
        .replace("quota", "rate limit");
    serde_json::json!({
        "error": {
            "message": format!("rate_limit error from upstream provider: {}", sanitized),
            "type": "rate_limit_error",
            "code": "rate_limit_exceeded"
        }
    })
}

/// Build a standard OpenAI-compatible error response JSON for server errors.
pub fn build_server_error_json(original_message: &str) -> serde_json::Value {
    // Sanitize: remove "quota exceeded" to avoid pi-coding-agent blocking the retry.
    let sanitized = original_message
        .replace("quota exceeded", "rate limit triggered")
        .replace("quota exceed", "rate limit triggered")
        .replace("quota", "rate limit");
    serde_json::json!({
        "error": {
            "message": format!("server_error from upstream provider: {}", sanitized),
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

/// Strip `<think>...</think>` blocks from each choice's `message.content` in an
/// OpenAI-protocol chat completion response body.
///
/// This is a pure function: takes raw response bytes, returns modified bytes.
/// Non-JSON bodies and bodies without `choices[*].message.content` strings are
/// returned unchanged (silent skip — caller may log if needed).
///
/// Used to fix non-OpenAI-compliant upstreams (e.g. MiniMax-M3) that mix
/// thinking content into the `content` field using `<think>` markdown tags
/// instead of the standard `reasoning_content` field.
pub fn strip_thinking_from_openai_response_body(body: &[u8]) -> Vec<u8> {
    let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(body) else {
        return body.to_vec();
    };
    let Some(choices) = v.get_mut("choices").and_then(|c| c.as_array_mut()) else {
        return body.to_vec();
    };
    for choice in choices {
        let Some(message) = choice.get_mut("message") else { continue };
        let Some(content_value) = message.get_mut("content") else { continue };
        let Some(content_str) = content_value.as_str() else { continue };
        let stripped = strip_thinking_tags(content_str);
        if stripped != content_str {
            *content_value = serde_json::Value::String(stripped);
        }
    }
    serde_json::to_vec(&v).unwrap_or_else(|_| body.to_vec())
}

/// Tracks state for stripping thinking tags from SSE streaming responses.
/// Thinking tags (like `<think>...</think>`) can span multiple SSE chunks,
/// so we need a state machine that buffers content while inside a thinking block.
pub struct SseThinkingStripper {
    /// Whether we're currently inside a thinking block
    inside_thinking: bool,
    /// Buffer for content that might be part of a thinking tag
    buffer: String,
}

impl SseThinkingStripper {
    pub fn new() -> Self {
        Self {
            inside_thinking: false,
            buffer: String::new(),
        }
    }

    /// Process a content delta string and return the content that should be
    /// sent to the client. Returns None if the entire content is inside a
    /// thinking block and should be suppressed.
    pub fn strip_delta(&mut self, content: &str) -> Option<String> {
        if content.is_empty() {
            if self.inside_thinking {
                return None;
            }
            return Some(String::new());
        }

        if self.inside_thinking {
            // We're inside a thinking block, look for the closing tag
            self.buffer.push_str(content);
            if let Some(pos) = self.buffer.find("</think>") {
                // Found closing tag - everything after it is real content
                self.inside_thinking = false;
                let after = self.buffer[pos + 8..].to_string();
                self.buffer.clear();
                if after.is_empty() {
                    return None;
                }
                // The content after </think> might contain another <think>
                return self.strip_delta(&after);
            }
            // Still inside thinking block, suppress this content
            return None;
        }

        // Not inside thinking block, look for opening tag
        if let Some(pos) = content.find("<think>") {
            // Found opening tag
            let before = content[..pos].to_string();
            let after = content[pos + 7..].to_string();
            self.inside_thinking = true;
            self.buffer.clear();
            self.buffer.push_str(&after);

            // Look for closing tag in the remaining content
            if let Some(cpos) = self.buffer.find("</think>") {
                self.inside_thinking = false;
                let after_close = self.buffer[cpos + 8..].to_string();
                self.buffer.clear();
                // Recursively process the content after the closing tag
                let mut result = before;
                if let Some(more) = self.strip_delta(&after_close) {
                    result.push_str(&more);
                }
                if result.is_empty() {
                    return None;
                }
                return Some(result);
            }

            if before.is_empty() {
                return None;
            }
            return Some(before);
        }

        // No thinking tags in this content, pass through
        Some(content.to_string())
    }

    /// Called at the end of the stream to flush any remaining buffered content.
    /// If we're still inside a thinking block when the stream ends, we discard
    /// the thinking content (it was never properly closed).
    pub fn flush(&mut self) -> Option<String> {
        if self.inside_thinking {
            // Stream ended inside a thinking block - discard it
            // But check if the buffer contains content after an unclosed </think>
            self.inside_thinking = false;
            let remaining = std::mem::take(&mut self.buffer);
            // Apply strip_thinking_tags as a final cleanup
            let cleaned = strip_thinking_tags(&remaining);
            if cleaned.is_empty() {
                return None;
            }
            return Some(cleaned);
        }
        None
    }
}

/// Process an SSE chunk through the thinking stripper state machine.
/// Returns cleaned bytes (potentially empty if entire chunk was inside thinking).
fn strip_thinking_from_sse_chunk_with_state(chunk_str: &str, stripper: &mut SseThinkingStripper) -> axum::body::Bytes {
    let mut output_lines: Vec<String> = Vec::new();
    
    for line in chunk_str.split('\n') {
        let trimmed = line.trim();
        if !trimmed.starts_with("data:") {
            output_lines.push(line.to_string());
            continue;
        }
        
        let json_str = trimmed.strip_prefix("data:").unwrap_or(trimmed).trim_start();
        if json_str.is_empty() || json_str == "[DONE]" {
            output_lines.push(line.to_string());
            continue;
        }
        
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(choices) = v.get_mut("choices").and_then(|c| c.as_array_mut()) {
                for choice in choices {
                    if let Some(delta) = choice.get_mut("delta") {
                        // Handle reasoning_content for MiniMax M3 - move to content if content is empty
                        let has_reasoning = delta.get("reasoning_content").and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false);
                        if has_reasoning {
                            // MiniMax M3 uses reasoning_content field instead of 
                            let reasoning_value = delta.get("reasoning_content").unwrap().as_str().unwrap_or("");
                            if !reasoning_value.is_empty() {
                                let existing = delta.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let new_content = if existing.is_empty() {
                                    reasoning_value.to_string()
                                } else {
                                    format!("{}\n{}", existing, reasoning_value)
                                };
                                if let Some(v) = delta.get_mut("content") {
                                    *v = serde_json::Value::String(new_content);
                                }
                            }
                            if let Some(v) = delta.get_mut("reasoning_content") {
                                *v = serde_json::Value::String(String::new());
                            }
                        } else {
                            if let Some(content_value) = delta.get_mut("content") {
                                if let Some(content_str) = content_value.as_str() {
                                    match stripper.strip_delta(content_str) {
                                        Some(cleaned) => {
                                            *content_value = serde_json::Value::String(cleaned);
                                        }
                                        None => {
                                            *content_value = serde_json::Value::String(String::new());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let cleaned_json = serde_json::to_string(&v).unwrap_or_else(|_| json_str.to_string());
            output_lines.push(format!("data: {}", cleaned_json));
        } else {
            output_lines.push(line.to_string());
        }
    }
    let result = output_lines.join("\n");
    axum::body::Bytes::from(result)
}
// transform_reasoning_content_to_content: Extract actual reply from MiniMax M3 content.
// MiniMax M3 streaming format (without reasoning_split):
//   - content field contains: 思考内容...</think>\n实际回复
//   - We extract everything after </think>\n as the actual reply
// MiniMax M3 streaming format (with reasoning_split):
//   - content field = actual reply (already clean)
//   - reasoning_content field = thinking (not needed)
// OMP clients only understand content field, so we:
//   1. Extract actual reply from content (strip thinking tags)
//   2. Clear reasoning_content and reasoning_details
fn transform_reasoning_content_to_content(chunk_str: &str) -> axum::body::Bytes {
    let mut output_lines: Vec<String> = Vec::new();
    for line in chunk_str.split('\n') {
        let trimmed = line.trim();
        if !trimmed.starts_with("data:") {
            output_lines.push(line.to_string());
            continue;
        }
        let json_str = trimmed.strip_prefix("data:").unwrap_or(trimmed).trim_start();
        if json_str.is_empty() || json_str == "[DONE]" {
            output_lines.push(line.to_string());
            continue;
        }
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(json_str) {
            if let Some(choices) = v.get_mut("choices").and_then(|c| c.as_array_mut()) {
                for choice in choices {
                    if let Some(delta) = choice.get_mut("delta") {
                        // Get content
                        let content_value = delta.get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        // Extract actual reply: take content after </think>\n (MiniMax format)
                        let actual_reply = if content_value.contains("</think>") {
                            // Split by </think>\n and take the last part (actual reply)
                            let parts: Vec<&str> = content_value.split("</think>").collect();
                            let after_think = parts.last().unwrap_or(&"").trim_start_matches('\n');
                            after_think.to_string()
                        } else {
                            content_value
                        };
                        // Update content with actual reply
                        if let Some(v) = delta.get_mut("content") {
                            *v = serde_json::Value::String(actual_reply);
                        }
                        // Clear reasoning fields - OMP doesn't understand them
                        if let Some(v) = delta.get_mut("reasoning_content") {
                            *v = serde_json::Value::String(String::new());
                        }
                        if let Some(v) = delta.get_mut("reasoning_details") {
                            *v = serde_json::Value::Array(Vec::new());
                        }
                    }
                }
            }
            let transformed = serde_json::to_string(&v).unwrap_or_else(|_| json_str.to_string());
            output_lines.push(format!("data: {}", transformed));
        } else {
            output_lines.push(line.to_string());
        }
    }
    let result = output_lines.join("\n");
    axum::body::Bytes::from(result)
}
/// Standard OpenAI-compatible /v1/models endpoint.
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

    // Select a mapping — prefer provider whose api_type matches the request path
    // /v1/messages → prefer anthropic, /v1/chat/completions → prefer openai
    // This enables the same unified model to route to different providers
    // based on the API format the client is using
    let preferred_api_type = if path.contains("/messages") {
        "anthropic"
    } else {
        "openai"
    };
    
    // Load provider info for all valid mappings at once
    let mut preferred_mapping: Option<&ModelMappingRow> = None;
    let mut fallback_mapping: Option<&ModelMappingRow> = None;
    
    for m in &valid_mappings {
        if let Ok(Some(p)) = state.db.get_provider(&m.provider_id).await {
            if !p.is_active { continue; }
            if p.api_type == preferred_api_type && preferred_mapping.is_none() {
                preferred_mapping = Some(m);
            } else if fallback_mapping.is_none() {
                fallback_mapping = Some(m);
            }
        }
    }
    
    let selected_mapping = preferred_mapping.or(fallback_mapping)
        .unwrap_or(&valid_mappings[0]);

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

    // Parse streaming flag before mock check (needed by both paths)
    let is_streaming = serde_json::from_slice::<serde_json::Value>(&body_bytes)
        .ok()
        .and_then(|v| v.get("stream")?.as_bool())
        .unwrap_or(false);

    // === Mock mode: intercept request and return fake response without calling upstream ===
    if provider.mock_mode {
        return handle_mock_request(
            &provider,
            &request_model_id,
            &selected_mapping.provider_model_id,
            is_streaming,
            &body_bytes,
            &api_key_row.id,
            state.stats_collector.clone(),
            state.db.clone(),
        ).await.into_response();
    }

    // Replace model name in body, inject stream_options, and set default max_completion_tokens

    let final_body = if let Ok(mut body_json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
        if let Some(obj) = body_json.as_object_mut() {
            obj.insert("model".to_string(), serde_json::Value::String(selected_mapping.provider_model_id.clone()));
            // For streaming, inject stream_options to get usage stats in final SSE chunk
            if is_streaming {
                obj.insert("stream_options".to_string(), serde_json::json!({"include_usage": true}));
            }
            // If neither max_completion_tokens nor max_tokens is set, inject a safe default.
            // Some providers (e.g. MiniMax M3) use a very low default when this is omitted,
            // causing thinking to consume all tokens and leaving nothing for the actual response.
            if !obj.contains_key("max_completion_tokens") && !obj.contains_key("max_tokens") {
                obj.insert("max_completion_tokens".to_string(), serde_json::Value::Number(serde_json::Number::from(8192)));
            }
            // Inject reasoning_split for providers that support it (e.g. MiniMax M3).
            // This separates thinking into reasoning_content field instead of mixing
            // thinking tags into content, keeping content always clean.
            if !obj.contains_key("reasoning_split") {
                obj.insert("reasoning_split".to_string(), serde_json::Value::Bool(true));
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
                let provider_id = {
                    let effective = provider.group_id.as_ref().unwrap_or(&provider.id);
                    effective.clone()
                };
                let provider_api_type = provider.api_type.clone();
                let log_path = path.clone();
                let log_model = model.id.clone();
                let request_body_str = if !body_bytes.is_empty() { Some(String::from_utf8_lossy(&body_bytes).to_string()) } else { None };

                let mut sse_stripper = if provider.strip_thinking_tags_in_response && provider.api_type == "openai" { Some(SseThinkingStripper::new()) } else { None };

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
                                    // Skip event: lines (Anthropic SSE format)
                                    if trimmed.starts_with("event:") { continue; }
                                    if !trimmed.starts_with("data:") { continue; }
                                    let json_str = trimmed.strip_prefix("data:").unwrap_or(trimmed).trim_start();
                                    if json_str.is_empty() { continue; }
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
                                        if provider_api_type == "anthropic" {
                                            // Anthropic SSE: content_block_delta with delta.text
                                            if let Some(content) = crate::usage::extract_anthropic_sse_content(&format!("data: {}", json_str)) {
                                                all_delta_content.push_str(&content);
                                            }
                                            // Anthropic SSE usage: message_delta for output tokens
                                            let (ap, ac, at) = crate::usage::extract_anthropic_streaming_usage(&format!("data: {}", json_str));
                                            if ap > 0 { prompt_tokens = ap; }
                                            if ac > 0 { completion_tokens = ac; }
                                            if at > 0 { total_tokens = at; }
                                        } else {
                                            // OpenAI SSE format
                                            if let Some(choices) = v.get("choices").and_then(|v| v.as_array()) {
                                                for choice in choices {
                                                    if let Some(delta) = choice.get("delta") {
                                                        // Extract content from both content and reasoning_content fields (MiniMax M3)
                                                        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                                                            all_delta_content.push_str(content);
                                                        }
                                                        if let Some(reasoning) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                                                            all_delta_content.push_str(reasoning);
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
                                }
                                // Strip thinking tags from SSE content deltas when enabled
                                // For MiniMax M3 which uses reasoning_content field, we need special handling
                                let has_reasoning_content = chunk_str.contains("reasoning_content");
                                let send_bytes = if sse_stripper.is_some() && !has_reasoning_content {
                                    strip_thinking_from_sse_chunk_with_state(&chunk_str, sse_stripper.as_mut().unwrap())
                                } else if has_reasoning_content {
                                    // MiniMax M3: move reasoning_content to content for OpenAI compatibility
                                    transform_reasoning_content_to_content(&chunk_str)
                                } else {
                                    bytes
                                };
                                if send_bytes.is_empty() {
                                    // Entire chunk was inside a thinking block, skip it
                                    continue;
                                }
                                if tx.send(Ok::<_, std::convert::Infallible>(send_bytes)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Stream error: {}", e);
                                break;
                            }
                        }
                    }

                    // Flush the SSE thinking stripper state machine
                    // This handles the case where thinking consumed all tokens and the
                    // closing tag was never sent (truncated thinking block)
                    if let Some(ref mut stripper) = sse_stripper {
                        if let Some(flushed) = stripper.flush() {
                            // Stream ended mid-thinking but there's content after an unclosed tag
                            if !flushed.trim().is_empty() {
                                let flush_chunk = format!(
                                    "data: {{\"id\":\"{}\",\"model\":\"{}\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{}\"}},\"finish_reason\":null}}]}}\n\n",
                                    response_id.as_deref().unwrap_or("chatcmpl-flush"),
                                    response_model.as_deref().unwrap_or("unknown"),
                                    flushed.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n")
                                );
                                let _ = tx.send(Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(flush_chunk))).await;
                            }
                        }
                    }

                    // If strip_thinking was enabled and the entire response was thinking content,
                    // send a notice to the client so they don't get an empty response
                    if sse_stripper.is_some() && !all_delta_content.is_empty() {
                        let clean = strip_thinking_tags(&all_delta_content);
                        if clean.trim().is_empty() {
                            // All content was thinking - send a notice delta
                            let notice = "[thinking content stripped - model used all tokens for reasoning]";
                            let notice_chunk = format!(
                                "data: {{\"id\":\"{}\",\"model\":\"{}\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{}\"}},\"finish_reason\":null}}]}}\n\n",
                                response_id.as_deref().unwrap_or("chatcmpl-stripped"),
                                response_model.as_deref().unwrap_or("unknown"),
                                notice
                            );
                            let _ = tx.send(Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(notice_chunk))).await;
                        }
                    }

                    // Stream finished - build reconstructed non-streaming response
                    let duration_ms = start.elapsed().as_millis() as i64;
                    let final_model = response_model.clone().or(Some(log_model));
                    // Strip thinking tags from accumulated delta content
                    let clean_content = strip_thinking_tags(&all_delta_content);

                    // If API returned 0 for both prompt and completion tokens, estimate from content
                    // This handles cases where the upstream provider doesn't report accurate usage
                    if prompt_tokens == 0 && completion_tokens == 0 {
                        if !all_delta_content.is_empty() {
                            completion_tokens = crate::usage::estimate_completion_tokens(&clean_content);
                        }
                        if let Some(ref req_body) = request_body_str {
                            prompt_tokens = crate::usage::estimate_prompt_tokens(req_body);
                        }
                        total_tokens = prompt_tokens + completion_tokens;
                    }

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
            // Opt-in: strip <think>...</think> tags from OpenAI-protocol response body.
            // Useful for non-OpenAI-compliant upstreams (e.g. MiniMax-M3) that mix
            // thinking content into `message.content` using markdown tags instead of
            // the standard `reasoning_content` field. The dashboard log still records
            // the ORIGINAL body (with thinking), only the response to the client is
            // cleaned. See docs/SPEC.md "strip_thinking_tags_in_response".
            let stripped_body = if provider.api_type == "openai" && provider.strip_thinking_tags_in_response {
                strip_thinking_from_openai_response_body(&proxy_response.body)
            } else {
                proxy_response.body.to_vec()
            };

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
                (status, stripped_body)
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

/// Handle mock request: generate fake LLM response without calling upstream
async fn handle_mock_request(
    provider: &crate::db::ProviderRow,
    model_id: &str,
    provider_model_id: &str,
    is_streaming: bool,
    body_bytes: &[u8],
    api_key_id: &str,
    stats_collector: std::sync::Arc<crate::stats::StatsCollector>,
    db: std::sync::Arc<crate::db::Database>,
) -> impl axum::response::IntoResponse {
    use axum::{response::Response, body::Body, http::HeaderMap};
    use std::time::Instant;

    let start = Instant::now();
    let duration_ms = start.elapsed().as_millis() as i64;

    // Parse request body to extract max_tokens
    let max_tokens = serde_json::from_slice::<serde_json::Value>(body_bytes)
        .ok()
        .and_then(|v| v.get("max_tokens").and_then(|m| m.as_i64()))
        .unwrap_or(1024)
        .min(4096) as i64;

    // Generate realistic mock content
    let mock_content = generate_mock_content(max_tokens as usize);
    let completion_tokens = crate::usage::estimate_completion_tokens(&mock_content);
    let prompt_tokens = crate::usage::estimate_prompt_tokens(&String::from_utf8_lossy(body_bytes).as_ref());
    let total_tokens = prompt_tokens + completion_tokens;

    // Log the mock request
    let request_body_str = String::from_utf8_lossy(body_bytes).to_string();
    let _ = db.insert_request_log(
        api_key_id,
        &provider.id,
        Some(model_id),
        "/v1/chat/completions",
        "POST",
        None,
        Some(&request_body_str),
        Some(200),
        None,
        None,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        Some(duration_ms),
        is_streaming,
        false,
        None,
    ).await;

    // Record usage for token rate tracking
    let _ = stats_collector.record_usage(&provider.id, prompt_tokens, completion_tokens).await;

    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());

    if is_streaming {
        // Streaming: yield multiple SSE chunks with a short delay between each
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<axum::body::Bytes, std::convert::Infallible>>(32);

        let mock_content_clone = mock_content.clone();
        let response_id_clone = response_id.clone();
        let model_id_clone = model_id.to_string();
        let api_key_id_clone = api_key_id.to_string();
        let provider_id = provider.id.clone();
        let db_clone = db.clone();
        let stats_clone = stats_collector.clone();
        let duration_ms_copy = duration_ms;

        tokio::spawn(async move {
            // Split content into chunks to simulate streaming
            let chunks: Vec<String> = mock_content_clone.chars()
                .collect::<Vec<_>>()
                .chunks(8)
                .map(|c| c.iter().collect::<String>())
                .collect();

            if chunks.is_empty() {
                // Empty response - send done
                let _ = tx.send(Ok(axum::body::Bytes::from("data: [DONE]\n\n"))).await;
                return;
            }

            // Send each chunk with a small delay
            for (i, chunk) in chunks.iter().enumerate() {
                let is_last = i == chunks.len() - 1;
                let choice_json = if is_last {
                    serde_json::json!({
                        "index": 0,
                        "delta": {},
                        "finish_reason": "stop"
                    })
                } else {
                    serde_json::json!({
                        "index": 0,
                        "delta": { "content": chunk }
                    })
                };

                let sse_line = format!("data: {}\n\n", choice_json);
                let _ = tx.send(Ok(axum::body::Bytes::from(sse_line))).await;
                tokio::time::sleep(std::time::Duration::from_millis(15)).await;
            }

            // Send final usage chunk (OpenAI SSE format for stream_options)
            let usage_chunk = format!(
                "data: {}\n\ndata: [DONE]\n\n",
                serde_json::json!({
                    "id": response_id_clone,
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": model_id_clone,
                    "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                    "usage": {
                        "prompt_tokens": prompt_tokens,
                        "completion_tokens": completion_tokens,
                        "total_tokens": total_tokens
                    }
                })
            );
            let _ = tx.send(Ok(axum::body::Bytes::from(usage_chunk))).await;

            // Log final streaming stats
            let _ = db_clone.insert_request_log(
                &api_key_id_clone,
                &provider_id,
                Some(&model_id_clone),
                "/v1/chat/completions",
                "POST",
                None,
                None,
                Some(200),
                None,
                None,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                Some(duration_ms_copy),
                true,
                false,
                None,
            ).await;
            let _ = stats_clone.record_usage(&provider_id, prompt_tokens, completion_tokens).await;
        });

        let body_stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .header("connection", "keep-alive")
            .body(Body::from_stream(body_stream))
            .unwrap()
            .into_response()
    } else {
        // Non-streaming: return complete mock response immediately
        let response = serde_json::json!({
            "id": response_id,
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": model_id,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": mock_content
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": total_tokens
            }
        });

        (StatusCode::OK, [("content-type", "application/json")], serde_json::to_string(&response).unwrap_or_default()).into_response()
    }
}

/// Generate random mock content for LLM responses
pub fn generate_mock_content(target_len: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Realistic-looking response sentences
    let sentences = [
        "This is a simulated response from the mock LLM provider.",
        "The request was processed successfully without calling the upstream LLM.",
        "Mock responses are useful for testing and development purposes.",
        "The system generated this content without consuming any API credits.",
        "You can enable mock mode on a provider to intercept requests.",
        "This helps avoid unnecessary token costs during development.",
        "The response content is randomly generated for demonstration.",
        "In production, replace mock mode with a real provider configuration.",
    ];

    let mut result = String::new();
    while result.len() < target_len {
        let sentence = sentences[rng.gen_range(0..sentences.len())];
        result.push_str(sentence);
        result.push(' ');
        // Add some variation
        if rng.gen_bool(0.3) {
            result.push_str("Here is some additional context. ");
        }
    }
    result.truncate(target_len);
    if !result.is_empty() {
        // Remove trailing partial word
        if let Some(last_space) = result.rfind(' ') {
            result.truncate(last_space);
        }
    }
    result
}

/// Extract API key from Authorization header
/// Also supports `x-api-key` (Anthropic SDK / Claude Code default) — preferred when present.
pub fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    // 1. Anthropic native header (Claude Code, Anthropic SDK)
    //    When ANTHROPIC_API_KEY is set, Claude Code sends `x-api-key: <key>`.
    //    When ANTHROPIC_AUTH_TOKEN is set, it sends `Authorization: Bearer <token>`.
    //    These are mutually exclusive in Claude Code, so x-api-key-first ordering is safe.
    if let Some(v) = headers.get("x-api-key").and_then(|h| h.to_str().ok()) {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    // 2. OpenAI-style `Authorization: Bearer <key>` (also ANTHROPIC_AUTH_TOKEN)
    let auth_header = headers.get("authorization")?.to_str().ok()?;
    if auth_header.starts_with("Bearer ") {
        Some(auth_header[7..].to_string())
    } else {
        Some(auth_header.to_string())
    }
}
