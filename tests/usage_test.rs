use llm_gateway::usage;

#[test]
fn test_usage_extraction_openai_full() {
    let response = serde_json::json!({
        "id": "chatcmpl-123",
        "model": "gpt-4",
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 200,
            "total_tokens": 300
        }
    });
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 100);
    assert_eq!(c, 200);
    assert_eq!(t, 300);
}

#[test]
fn test_usage_extraction_openai_no_total() {
    let response = serde_json::json!({
        "usage": {
            "prompt_tokens": 50,
            "completion_tokens": 75
        }
    });
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 50);
    assert_eq!(c, 75);
    assert_eq!(t, 125);
}

#[test]
fn test_usage_extraction_no_usage_field() {
    let response = serde_json::json!({"id": "chatcmpl-123"});
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!((p, c, t), (0, 0, 0));
}

#[test]
fn test_usage_extraction_empty_object() {
    let response = serde_json::json!({});
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!((p, c, t), (0, 0, 0));
}

#[test]
fn test_streaming_usage_extraction() {
    let chunk = r#"data: {"id":"chatcmpl-1","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":200,"total_tokens":300}}"#;
    let (p, c, t) = usage::extract_streaming_usage(chunk);
    assert_eq!(p, 100);
    assert_eq!(c, 200);
    assert_eq!(t, 300);
}

#[test]
fn test_streaming_usage_done_marker() {
    let (p, c, t) = usage::extract_streaming_usage("data: [DONE]");
    assert_eq!((p, c, t), (0, 0, 0));
}

#[test]
fn test_streaming_usage_no_prefix() {
    let chunk = r#"{"id":"chatcmpl-1","usage":{"prompt_tokens":50,"completion_tokens":100,"total_tokens":150}}"#;
    let (p, c, t) = usage::extract_streaming_usage(chunk);
    assert_eq!(p, 50);
    assert_eq!(c, 100);
    assert_eq!(t, 150);
}

#[test]
fn test_streaming_usage_invalid_json() {
    let (p, c, t) = usage::extract_streaming_usage("data: invalid json");
    assert_eq!((p, c, t), (0, 0, 0));
}

#[test]
fn test_extract_model_from_response() {
    let response = serde_json::json!({"model": "gpt-4-0613"});
    assert_eq!(usage::extract_model_from_response(&response), Some("gpt-4-0613".to_string()));
}

#[test]
fn test_extract_model_no_model() {
    let response = serde_json::json!({"id": "123"});
    assert_eq!(usage::extract_model_from_response(&response), None);
}

#[test]
fn test_is_rate_limited() {
    assert!(usage::is_rate_limited(429));
    assert!(!usage::is_rate_limited(200));
    assert!(!usage::is_rate_limited(500));
    assert!(!usage::is_rate_limited(400));
}

#[test]
fn test_usage_extraction_anthropic_format() {
    // Anthropic uses different field names but our extractor handles OpenAI format
    let response = serde_json::json!({
        "usage": {
            "prompt_tokens": 50,
            "completion_tokens": 100,
            "total_tokens": 150
        }
    });
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 50);
    assert_eq!(c, 100);
    assert_eq!(t, 150);
}

#[test]
fn test_usage_extraction_zero_tokens() {
    let response = serde_json::json!({
        "usage": {
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0
        }
    });
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!((p, c, t), (0, 0, 0));
}

#[test]
fn test_streaming_usage_delta_only() {
    // Typical streaming chunk with delta but no usage
    let chunk = r#"data: {"id":"chatcmpl-1","choices":[{"delta":{"content":"Hello"}}]}"#;
    let (p, c, t) = usage::extract_streaming_usage(chunk);
    assert_eq!((p, c, t), (0, 0, 0));
}

#[test]
fn test_streaming_usage_final_chunk_with_usage() {
    // Final streaming chunk that includes usage stats
    let chunk = r#"data: {"id":"chatcmpl-1","choices":[{"finish_reason":"stop"}],"usage":{"prompt_tokens":25,"completion_tokens":50,"total_tokens":75}}"#;
    let (p, c, t) = usage::extract_streaming_usage(chunk);
    assert_eq!(p, 25);
    assert_eq!(c, 50);
    assert_eq!(t, 75);
}