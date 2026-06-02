use llm_gateway::proxy::handler;

// ==================== extract_api_key ====================

#[test]
fn test_extract_api_key_bearer_prefix() {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("authorization", "Bearer sk-test-key-123".parse().unwrap());
    let result = handler::extract_api_key(&headers);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "sk-test-key-123");
}

#[test]
fn test_extract_api_key_no_bearer_prefix() {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("authorization", "sk-raw-key".parse().unwrap());
    let result = handler::extract_api_key(&headers);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "sk-raw-key");
}

#[test]
fn test_extract_api_key_missing_header() {
    let headers = axum::http::HeaderMap::new();
    let result = handler::extract_api_key(&headers);
    assert!(result.is_none());
}

#[test]
fn test_extract_api_key_empty_bearer() {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("authorization", "Bearer ".parse().unwrap());
    let result = handler::extract_api_key(&headers);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "");
}

// === x-api-key header support (Claude Code / Anthropic SDK) ===

#[test]
fn test_extract_api_key_x_api_key() {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-api-key", "sk-test-key-123".parse().unwrap());
    let result = handler::extract_api_key(&headers);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "sk-test-key-123");
}

#[test]
fn test_extract_api_key_x_api_key_preferred() {
    // When both x-api-key and Authorization are present, x-api-key wins
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-api-key", "lgk-x-api-key-value".parse().unwrap());
    headers.insert("authorization", "Bearer lgk-bearer-value".parse().unwrap());
    let result = handler::extract_api_key(&headers);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "lgk-x-api-key-value");
}

#[test]
fn test_extract_api_key_x_api_key_empty_falls_back_to_bearer() {
    // Empty x-api-key should fall back to Authorization
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-api-key", "   ".parse().unwrap());
    headers.insert("authorization", "Bearer lgk-bearer-value".parse().unwrap());
    let result = handler::extract_api_key(&headers);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "lgk-bearer-value");
}

#[test]
fn test_extract_api_key_x_api_key_only_no_authorization() {
    // Only x-api-key, no Authorization header
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-api-key", "lgk-anthropic-style-key".parse().unwrap());
    let result = handler::extract_api_key(&headers);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "lgk-anthropic-style-key");
}

// ==================== is_rate_limit_error ====================

#[test]
fn test_is_rate_limit_error_quota_exceeded() {
    assert!(handler::is_rate_limit_error("quota exceeded for this model"));
}

#[test]
fn test_is_rate_limit_error_rate_limit() {
    assert!(handler::is_rate_limit_error("Rate limit exceeded"));
}

#[test]
fn test_is_rate_limit_error_insufficient_quota() {
    assert!(handler::is_rate_limit_error("You have insufficient quota"));
}

#[test]
fn test_is_rate_limit_error_429_code() {
    assert!(handler::is_rate_limit_error("Error code 429: Too many requests"));
}

#[test]
fn test_is_rate_limit_error_normal_error() {
    assert!(!handler::is_rate_limit_error("Internal server error"));
}

#[test]
fn test_is_rate_limit_error_empty() {
    assert!(!handler::is_rate_limit_error(""));
}

#[test]
fn test_is_rate_limit_error_throttling() {
    assert!(handler::is_rate_limit_error("Request was throttled"));
}

#[test]
fn test_is_rate_limit_error_tokens_per_minute() {
    assert!(handler::is_rate_limit_error("Rate limit reached for tokens-per-minute"));
}

// ==================== strip_thinking_tags ====================

#[test]
fn test_strip_thinking_tags_basic() {
    let input = "Hello <think>this is thinking</think> world";
    let result = handler::strip_thinking_tags(input);
    assert!(result.contains("Hello"));
    assert!(result.contains("world"));
    assert!(!result.contains("thinking"));
}

#[test]
fn test_strip_thinking_tags_multiple() {
    let input = "Start <think>first</think> middle <think>second</think> end";
    let result = handler::strip_thinking_tags(input);
    assert!(result.contains("Start"));
    assert!(result.contains("middle"));
    assert!(result.contains("end"));
    assert!(!result.contains("first"));
    assert!(!result.contains("second"));
}

#[test]
fn test_strip_thinking_tags_no_tags() {
    let input = "No thinking tags here";
    let result = handler::strip_thinking_tags(input);
    assert_eq!(result, "No thinking tags here");
}

#[test]
fn test_strip_thinking_tags_empty() {
    let result = handler::strip_thinking_tags("");
    assert_eq!(result, "");
}

#[test]
fn test_strip_thinking_tags_only_thinking() {
    let input = "<think>all thinking no content</think>";
    let result = handler::strip_thinking_tags(input);
    assert!(result.is_empty() || result.trim().is_empty());
}

#[test]
fn test_strip_thinking_tags_multiline() {
    let input = "Hello\n<think>\nline1\nline2\n</think>\nWorld";
    let result = handler::strip_thinking_tags(input);
    assert!(result.contains("Hello"));
    assert!(result.contains("World"));
    assert!(!result.contains("line1"));
}

// ==================== generate_mock_content ====================

#[test]
fn test_generate_mock_content_length() {
    let content = handler::generate_mock_content(100);
    assert!(content.len() > 0, "Mock content should not be empty");
    assert!(content.len() <= 100 + 50, "Mock content should be roughly within target length: got {}", content.len());
}

#[test]
fn test_generate_mock_content_zero() {
    let content = handler::generate_mock_content(0);
    assert!(content.is_empty(), "Zero-length mock should be empty");
}

#[test]
fn test_generate_mock_content_large() {
    let content = handler::generate_mock_content(500);
    assert!(content.len() > 0);
    assert!(content.len() <= 600, "Large mock content should be bounded");
}

#[test]
fn test_generate_mock_content_realistic_words() {
    let content = handler::generate_mock_content(200);
    // Should contain some realistic words, not just random chars
    assert!(content.contains("mock") || content.contains("Mock") || content.contains("response") || content.contains("testing"));
}

// ==================== build_rate_limit_error_json ====================

#[test]
fn test_build_rate_limit_error_json_structure() {
    // Note: build_rate_limit_error_json intentionally sanitizes "quota" / "quota exceeded"
    // upstream messages (see handler.rs) to avoid pi-coding-agent's
    // _isNonRetryableProviderLimitError regex from blocking retry. The test asserts the
    // post-sanitize behavior — i.e. "rate limit" remains in the message but "quota" is
    // rewritten, and the response is still detectable as a rate-limit via the "type" field
    // and the literal "rate_limit" substring.
    let json = handler::build_rate_limit_error_json("quota exceeded for model X");
    assert_eq!(json["error"]["type"], "rate_limit_error");
    let msg = json["error"]["message"].as_str().unwrap();
    assert!(msg.contains("rate_limit"),
        "message must contain 'rate_limit' for pi-coding-agent retry detection: got '{}'", msg);
    assert!(!msg.to_lowercase().contains("quota"),
        "message must NOT contain 'quota' (sanitized to avoid pi-coding-agent non-retryable detection): got '{}'", msg);
}

// ==================== build_server_error_json ====================

#[test]
fn test_build_server_error_json_structure() {
    let json = handler::build_server_error_json("internal error");
    assert_eq!(json["error"]["type"], "server_error");
    assert!(json["error"]["message"].as_str().unwrap().contains("internal error"));
}

// ==================== strip_thinking_from_openai_response_body ====================

fn parse_body(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).expect("body should be valid JSON")
}

#[test]
fn test_strip_thinking_from_openai_response_body_basic() {
    // The MiniMax-M3 pattern: <think>...</think> then actual response
    let body = br#"{"choices":[{"message":{"role":"assistant","content":"<think>\nThe user wants a 3-sentence explanation.\n</think>\nRust is a systems language."}}]}"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    let content = v["choices"][0]["message"]["content"].as_str().unwrap();
    assert_eq!(content, "Rust is a systems language.",
        "thinking block must be removed, leaving only actual response: got {:?}", content);
}

#[test]
fn test_strip_thinking_from_openai_response_body_multiple_choices() {
    let body = br#"{"choices":[
        {"message":{"role":"assistant","content":"<think>\nthink1\n</think>\nAnswer 1"}},
        {"message":{"role":"assistant","content":"No thinking here, just answer"}},
        {"message":{"role":"assistant","content":"<think>\nthink3\n</think>\nAnswer 3"}}
    ]}"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    let contents: Vec<&str> = v["choices"].as_array().unwrap()
        .iter()
        .map(|c| c["message"]["content"].as_str().unwrap())
        .collect();
    assert_eq!(contents[0], "Answer 1");
    assert_eq!(contents[1], "No thinking here, just answer");
    assert_eq!(contents[2], "Answer 3");
}

#[test]
fn test_strip_thinking_from_openai_response_body_no_thinking() {
    // No <think> tags — content must pass through unchanged
    let original = br#"{"choices":[{"message":{"role":"assistant","content":"Just a normal response with no tags."}}]}"#;
    let out = handler::strip_thinking_from_openai_response_body(original);
    let v = parse_body(&out);
    assert_eq!(
        v["choices"][0]["message"]["content"].as_str().unwrap(),
        "Just a normal response with no tags."
    );
}

#[test]
fn test_strip_thinking_from_openai_response_body_multiple_thinking_blocks() {
    let body = br#"{"choices":[{"message":{"role":"assistant","content":"<think>\nfirst thought\n</think>\nMiddle text <think>\nsecond thought\n</think>\nEnd text"}}]}"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    let content = v["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(!content.contains("first thought"));
    assert!(!content.contains("second thought"));
    assert!(content.contains("Middle text"));
    assert!(content.contains("End text"));
}

#[test]
fn test_strip_thinking_from_openai_response_body_only_thinking() {
    // Content is ONLY a thinking block — result should be empty string
    let body = br#"{"choices":[{"message":{"role":"assistant","content":"<think>\njust thinking, no actual answer\n</think>"}}]}"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    let content = v["choices"][0]["message"]["content"].as_str().unwrap();
    assert_eq!(content, "", "Pure thinking should yield empty content");
}

#[test]
fn test_strip_thinking_from_openai_response_body_multiline_thinking() {
    let body = br#"{"choices":[{"message":{"role":"assistant","content":"<think>\nLine 1\nLine 2\nLine 3\nwith detail\n</think>\nThe actual answer is 42."}}]}"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    let content = v["choices"][0]["message"]["content"].as_str().unwrap();
    assert_eq!(content, "The actual answer is 42.");
    assert!(!content.contains("Line 1"));
    assert!(!content.contains("with detail"));
}

#[test]
fn test_strip_thinking_from_openai_response_body_malformed_json() {
    // Not JSON — must return body unchanged (silent skip, not error)
    let body = b"not json at all, just plain text";
    let out = handler::strip_thinking_from_openai_response_body(body);
    assert_eq!(out, body.to_vec());
}

#[test]
fn test_strip_thinking_from_openai_response_body_no_choices() {
    // Valid JSON but no `choices` key (e.g. error response) — pass through
    let body = br#"{"error":{"message":"rate limit exceeded"}}"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    assert_eq!(v["error"]["message"].as_str().unwrap(), "rate limit exceeded");
}

#[test]
fn test_strip_thinking_from_openai_response_body_no_message_field() {
    // Streaming delta shape — no `message` key, only `delta`
    let body = br#"{"choices":[{"delta":{"content":"hello"}}]}"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    // Must not crash, must not invent a message field
    assert!(v["choices"][0].get("message").is_none(),
        "must not add a message field if it didn't exist");
    assert_eq!(v["choices"][0]["delta"]["content"].as_str().unwrap(), "hello");
}

#[test]
fn test_strip_thinking_from_openai_response_body_content_not_string() {
    // Some OpenAI variants may have `content: null` or array of parts
    let body = br#"{"choices":[{"message":{"role":"assistant","content":null}}]}"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    assert!(v["choices"][0]["message"]["content"].is_null(),
        "null content must pass through unchanged");
}

#[test]
fn test_strip_thinking_from_openai_response_body_preserves_other_fields() {
    // Verify model/usage/finish_reason etc. are preserved untouched
    let body = br#"{
        "id":"chatcmpl-abc",
        "model":"MiniMax-M3",
        "choices":[{"message":{"role":"assistant","content":"<think>\nthink\n</think>\nAnswer"},"finish_reason":"stop","index":0}],
        "usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}
    }"#;
    let out = handler::strip_thinking_from_openai_response_body(body);
    let v = parse_body(&out);
    assert_eq!(v["id"].as_str().unwrap(), "chatcmpl-abc");
    assert_eq!(v["model"].as_str().unwrap(), "MiniMax-M3");
    assert_eq!(v["choices"][0]["finish_reason"].as_str().unwrap(), "stop");
    assert_eq!(v["choices"][0]["index"].as_i64().unwrap(), 0);
    assert_eq!(v["choices"][0]["message"]["content"].as_str().unwrap(), "Answer");
    assert_eq!(v["usage"]["total_tokens"].as_i64().unwrap(), 15);
}
