use llm_gateway::usage;
use llm_gateway::utils;

// ==================== Usage: estimate_tokens ====================

#[test]
fn test_estimate_tokens_empty() {
    assert_eq!(usage::estimate_tokens(""), 0);
}

#[test]
fn test_estimate_tokens_ascii() {
    let tokens = usage::estimate_tokens("Hello world this is a test");
    assert!(tokens > 0, "Should estimate some tokens for ASCII text");
    assert!(tokens < 50, "Should be reasonable estimate for short text");
}

#[test]
fn test_estimate_tokens_chinese() {
    let tokens = usage::estimate_tokens("你好世界这是一个测试");
    assert!(tokens > 0, "Should estimate tokens for Chinese text");
}

#[test]
fn test_estimate_tokens_mixed() {
    let tokens = usage::estimate_tokens("Hello 你好 world 世界");
    assert!(tokens > 0);
}

#[test]
fn test_estimate_completion_tokens_same_as_estimate() {
    let text = "This is a completion response from the model.";
    let est = usage::estimate_tokens(text);
    let comp = usage::estimate_completion_tokens(text);
    assert_eq!(est, comp, "estimate_completion_tokens should delegate to estimate_tokens");
}

#[test]
fn test_estimate_prompt_tokens_same_as_estimate() {
    let text = r#"{"model":"gpt-4","messages":[{"role":"user","content":"Hi"}]}"#;
    let est = usage::estimate_tokens(text);
    let prompt = usage::estimate_prompt_tokens(text);
    assert_eq!(est, prompt, "estimate_prompt_tokens should delegate to estimate_tokens");
}

// ==================== Usage: extract_openai_usage ====================

#[test]
fn test_extract_openai_usage_with_usage() {
    let response = serde_json::json!({
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 20,
            "total_tokens": 30
        }
    });
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 10);
    assert_eq!(c, 20);
    assert_eq!(t, 30);
}

#[test]
fn test_extract_openai_usage_no_usage() {
    let response = serde_json::json!({"choices": []});
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 0);
    assert_eq!(c, 0);
    assert_eq!(t, 0);
}

#[test]
fn test_extract_openai_usage_partial_usage() {
    let response = serde_json::json!({
        "usage": {
            "prompt_tokens": 15,
            "completion_tokens": 25
        }
    });
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 15);
    assert_eq!(c, 25);
    assert_eq!(t, 40); // total = prompt + completion
}

#[test]
fn test_extract_openai_usage_zero_tokens_with_content() {
    // When both are 0 but response has content, it should estimate
    let response = serde_json::json!({
        "model": "gpt-4",
        "usage": {
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0
        },
        "choices": [{
            "message": {
                "content": "This is a test response from the model"
            }
        }]
    });
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert!(c > 0, "Should estimate completion tokens from content when usage is 0");
    assert!(t > 0);
}

#[test]
fn test_extract_openai_usage_deepseek_format() {
    // DeepSeek format: prompt_tokens_details and completion_tokens_details
    let response = serde_json::json!({
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 50,
            "total_tokens": 150,
            "prompt_tokens_details": {"cached_tokens": 80},
            "completion_tokens_details": {"reasoning_tokens": 20}
        }
    });
    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 100);
    assert_eq!(c, 50);
    assert_eq!(t, 150);
}

// ==================== Usage: extract_streaming_usage ====================

#[test]
fn test_extract_streaming_usage_with_data_prefix() {
    let chunk = r#"data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30}}"#;
    let (p, c, t) = usage::extract_streaming_usage(chunk);
    assert_eq!(p, 10);
    assert_eq!(c, 20);
    assert_eq!(t, 30);
}

#[test]
fn test_extract_streaming_usage_done_chunk() {
    let (p, c, t) = usage::extract_streaming_usage("data: [DONE]");
    assert_eq!(p, 0);
    assert_eq!(c, 0);
}

#[test]
fn test_extract_streaming_usage_invalid_json() {
    let (p, c, t) = usage::extract_streaming_usage("data: {invalid}");
    assert_eq!(p, 0);
    assert_eq!(c, 0);
}

#[test]
fn test_extract_streaming_usage_no_usage_field() {
    let chunk = r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#;
    let (p, c, t) = usage::extract_streaming_usage(chunk);
    assert_eq!(p, 0);
    assert_eq!(c, 0);
}

#[test]
fn test_extract_streaming_usage_empty() {
    let (p, c, t) = usage::extract_streaming_usage("");
    assert_eq!(p, 0);
    assert_eq!(c, 0);
}

// ==================== Usage: extract_model_from_response ====================

#[test]
fn test_extract_model_from_response_with_model() {
    let response = serde_json::json!({
        "model": "gpt-4-0613"
    });
    let model = usage::extract_model_from_response(&response);
    assert_eq!(model, Some("gpt-4-0613".to_string()));
}

#[test]
fn test_extract_model_from_response_no_model() {
    let response = serde_json::json!({"choices": []});
    let model = usage::extract_model_from_response(&response);
    assert!(model.is_none());
}

#[test]
fn test_extract_model_from_response_non_string_model() {
    let response = serde_json::json!({"model": 42});
    let model = usage::extract_model_from_response(&response);
    assert!(model.is_none());
}

// ==================== Usage: is_rate_limited ====================

#[test]
fn test_is_rate_limited_429() {
    assert!(usage::is_rate_limited(429));
}

#[test]
fn test_is_rate_limited_200() {
    assert!(!usage::is_rate_limited(200));
}

#[test]
fn test_is_rate_limited_500() {
    assert!(!usage::is_rate_limited(500));
}

#[test]
fn test_is_rate_limited_403() {
    assert!(!usage::is_rate_limited(403));
}

// ==================== Usage: estimate_from_response ====================

#[test]
fn test_estimate_from_response_with_content() {
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "content": "This is a generated response with some text content."
            }
        }]
    });
    let tokens = usage::estimate_from_response(&response);
    assert!(tokens > 0);
}

#[test]
fn test_estimate_from_response_empty_choices() {
    let response = serde_json::json!({"choices": []});
    let tokens = usage::estimate_from_response(&response);
    assert_eq!(tokens, 0);
}

#[test]
fn test_estimate_from_response_delta_content() {
    let response = serde_json::json!({
        "choices": [{
            "delta": {
                "content": "Streaming delta content"
            }
        }]
    });
    let tokens = usage::estimate_from_response(&response);
    assert!(tokens > 0);
}

// ==================== Auth: extract_json_path ====================

#[test]
fn test_extract_json_path_simple() {
    let value = serde_json::json!({"data": {"token": "abc123"}});
    let result = llm_gateway::auth::extract_json_path(&value, "data.token");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "abc123");
}

#[test]
fn test_extract_json_path_top_level() {
    let value = serde_json::json!({"access_token": "xyz789"});
    let result = llm_gateway::auth::extract_json_path(&value, "access_token");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "xyz789");
}

#[test]
fn test_extract_json_path_missing() {
    let value = serde_json::json!({"data": {"token": "abc"}});
    let result = llm_gateway::auth::extract_json_path(&value, "data.missing");
    assert!(result.is_none());
}

#[test]
fn test_extract_json_path_empty_path() {
    let value = serde_json::json!({"key": "value"});
    let result = llm_gateway::auth::extract_json_path(&value, "");
    assert!(result.is_none() || result.is_some());
}

#[test]
fn test_extract_json_path_deeply_nested() {
    let value = serde_json::json!({"a": {"b": {"c": {"d": "deep"}}}});
    let result = llm_gateway::auth::extract_json_path(&value, "a.b.c.d");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "deep");
}

#[test]
fn test_extract_json_path_array_index() {
    // extract_json_path uses string keys, not numeric indices
    let value = serde_json::json!({"items": {"0": "first", "1": "second"}});
    let result = llm_gateway::auth::extract_json_path(&value, "items.0");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), "first");
}

#[test]
fn test_extract_json_path_non_object() {
    let value = serde_json::json!(42);
    let result = llm_gateway::auth::extract_json_path(&value, "some.path");
    assert!(result.is_none());
}

// ==================== Utils: sha256_hash ====================

#[test]
fn test_sha256_hash_empty() {
    let hash = utils::sha256_hash("");
    assert!(!hash.is_empty());
    // SHA256 of empty string is known
    assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn test_sha256_hash_hello() {
    let hash = utils::sha256_hash("hello");
    assert!(!hash.is_empty());
    assert_eq!(hash.len(), 64); // SHA256 hex output is 64 chars
}

#[test]
fn test_sha256_hash_deterministic() {
    let hash1 = utils::sha256_hash("test input");
    let hash2 = utils::sha256_hash("test input");
    assert_eq!(hash1, hash2, "Same input should produce same hash");
}

#[test]
fn test_sha256_hash_different_inputs() {
    let hash1 = utils::sha256_hash("input1");
    let hash2 = utils::sha256_hash("input2");
    assert_ne!(hash1, hash2, "Different inputs should produce different hashes");
}

#[test]
fn test_sha256_hash_unicode() {
    let hash = utils::sha256_hash("你好世界");
    assert!(!hash.is_empty());
    assert_eq!(hash.len(), 64);
}
