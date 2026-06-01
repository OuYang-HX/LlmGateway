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
    let json = handler::build_rate_limit_error_json("quota exceeded");
    assert_eq!(json["error"]["type"], "rate_limit_error");
    assert!(json["error"]["message"].as_str().unwrap().contains("quota exceeded"));
}

// ==================== build_server_error_json ====================

#[test]
fn test_build_server_error_json_structure() {
    let json = handler::build_server_error_json("internal error");
    assert_eq!(json["error"]["type"], "server_error");
    assert!(json["error"]["message"].as_str().unwrap().contains("internal error"));
}
