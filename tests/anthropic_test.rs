use llm_gateway::usage;

// ==================== Anthropic usage extraction ====================

#[test]
fn test_extract_anthropic_usage_standard() {
    let response = serde_json::json!({
        "usage": {
            "input_tokens": 25,
            "output_tokens": 50
        }
    });
    let (prompt, completion, total) = usage::extract_anthropic_usage(&response);
    assert_eq!(prompt, 25);
    assert_eq!(completion, 50);
    assert_eq!(total, 75);
}

#[test]
fn test_extract_anthropic_usage_no_usage() {
    let response = serde_json::json!({"type": "message", "content": []});
    let (p, c, t) = usage::extract_anthropic_usage(&response);
    assert_eq!(p, 0);
    assert_eq!(c, 0);
    assert_eq!(t, 0);
}

#[test]
fn test_extract_anthropic_usage_partial() {
    // Only input_tokens, no output_tokens
    let response = serde_json::json!({
        "usage": {
            "input_tokens": 100
        }
    });
    let (p, c, t) = usage::extract_anthropic_usage(&response);
    assert_eq!(p, 100);
    assert_eq!(c, 0);
    assert_eq!(t, 100);
}

#[test]
fn test_extract_anthropic_usage_cache_metrics() {
    // Anthropic can include cache_read_input_tokens, cache_creation_input_tokens
    let response = serde_json::json!({
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_read_input_tokens": 80,
            "cache_creation_input_tokens": 20
        }
    });
    let (p, c, t) = usage::extract_anthropic_usage(&response);
    assert_eq!(p, 100);
    assert_eq!(c, 50);
    assert_eq!(t, 150);
}

#[test]
fn test_extract_anthropic_usage_empty_object() {
    let response = serde_json::json!({"usage": {}});
    let (p, c, t) = usage::extract_anthropic_usage(&response);
    assert_eq!(p, 0);
    assert_eq!(c, 0);
}

// ==================== Anthropic streaming usage extraction ====================

#[test]
fn test_extract_anthropic_streaming_usage_message_delta() {
    // Anthropic sends usage in the message_delta event
    let chunk = r#"event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":15}}"#;
    let (p, c, t) = usage::extract_anthropic_streaming_usage(chunk);
    assert_eq!(p, 0);  // input tokens sent in message_start, not here
    assert_eq!(c, 15);
    assert_eq!(t, 15);
}

#[test]
fn test_extract_anthropic_streaming_usage_message_start() {
    let chunk = r#"event: message_start
data: {"type":"message_start","message":{"usage":{"input_tokens":25,"output_tokens":0}}}"#;
    let (p, c, t) = usage::extract_anthropic_streaming_usage(chunk);
    assert_eq!(p, 25);
    assert_eq!(c, 0);
}

#[test]
fn test_extract_anthropic_streaming_usage_content_block_delta() {
    // Content deltas don't carry usage info
    let chunk = r#"event: content_block_delta
data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}"#;
    let (p, c, t) = usage::extract_anthropic_streaming_usage(chunk);
    assert_eq!(p, 0);
    assert_eq!(c, 0);
}

#[test]
fn test_extract_anthropic_streaming_usage_non_anthropic() {
    // OpenAI format chunk should return 0
    let chunk = r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#;
    let (p, c, t) = usage::extract_anthropic_streaming_usage(chunk);
    assert_eq!(p, 0);
    assert_eq!(c, 0);
}

#[test]
fn test_extract_anthropic_streaming_usage_empty() {
    let (p, c, t) = usage::extract_anthropic_streaming_usage("");
    assert_eq!(p, 0);
    assert_eq!(c, 0);
}

#[test]
fn test_extract_anthropic_streaming_usage_no_event_prefix() {
    // Data-only line without event prefix
    let chunk = r#"data: {"type":"message_delta","usage":{"output_tokens":10}}"#;
    let (p, c, t) = usage::extract_anthropic_streaming_usage(chunk);
    assert_eq!(c, 10);
}

// ==================== Anthropic SSE content extraction ====================

#[test]
fn test_extract_anthropic_sse_content() {
    let chunk = r#"event: content_block_delta
data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello world"}}"#;
    let content = usage::extract_anthropic_sse_content(chunk);
    assert_eq!(content, Some("Hello world".to_string()));
}

#[test]
fn test_extract_anthropic_sse_content_thinking() {
    let chunk = r#"event: content_block_delta
data: {"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"Let me think..."}}"#;
    let content = usage::extract_anthropic_sse_content(chunk);
    // Thinking content should not be returned as main content
    assert!(content.is_none() || content.unwrap().is_empty());
}

#[test]
fn test_extract_anthropic_sse_content_non_delta() {
    let chunk = r#"event: message_start
data: {"type":"message_start","message":{}}"#;
    let content = usage::extract_anthropic_sse_content(chunk);
    assert!(content.is_none());
}

#[test]
fn test_extract_anthropic_sse_content_openai_format() {
    let chunk = r#"data: {"choices":[{"delta":{"content":"Hi"}}"#;
    let content = usage::extract_anthropic_sse_content(chunk);
    assert!(content.is_none());
}

#[test]
fn test_extract_anthropic_sse_content_empty() {
    let content = usage::extract_anthropic_sse_content("");
    assert!(content.is_none());
}

// ==================== Anthropic error detection ====================

#[test]
fn test_is_anthropic_error() {
    let response = serde_json::json!({
        "type": "error",
        "error": {
            "type": "overloaded_error",
            "message": "Overloaded"
        }
    });
    assert!(usage::is_anthropic_error(&response));
}

#[test]
fn test_is_anthropic_error_not_error() {
    let response = serde_json::json!({
        "type": "message",
        "content": []
    });
    assert!(!usage::is_anthropic_error(&response));
}

#[test]
fn test_is_anthropic_error_rate_limit() {
    let response = serde_json::json!({
        "type": "error",
        "error": {
            "type": "rate_limit_error",
            "message": "Too many requests"
        }
    });
    assert!(usage::is_anthropic_error(&response));
    assert!(usage::is_anthropic_rate_limit_error(&response));
}

#[test]
fn test_is_anthropic_rate_limit_not_rate_limit() {
    let response = serde_json::json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": "Bad request"
        }
    });
    assert!(!usage::is_anthropic_rate_limit_error(&response));
}
