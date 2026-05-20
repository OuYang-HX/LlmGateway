/// Token usage extraction from LLM API responses
use serde_json::Value;

/// Simple token estimator for when API doesn't return usage info.
pub fn estimate_tokens(text: &str) -> i64 {
    if text.is_empty() {
        return 0;
    }
    let char_count = text.chars().count();
    // Rough estimation: use 2.5 as a middle ground for mixed Chinese/English
    ((char_count as f64) / 2.5) as i64
}

pub fn estimate_completion_tokens(content: &str) -> i64 {
    estimate_tokens(content)
}

pub fn estimate_prompt_tokens(request_body: &str) -> i64 {
    estimate_tokens(request_body)
}

/// Extract token usage from an OpenAI-compatible response
pub fn extract_openai_usage(response: &Value) -> (i64, i64, i64) {
    let usage = response.get("usage");
    match usage {
        Some(u) => {
            let prompt = u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let completion = u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let total = u.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(prompt + completion);

            // If API returns 0 for both, try to estimate
            if prompt == 0 && completion == 0 {
                if let Some(model) = response.get("model").and_then(|v| v.as_str()) {
                    let estimated = estimate_from_response_body(response, model);
                    if estimated.0 > 0 || estimated.1 > 0 {
                        return estimated;
                    }
                }
            }

            (prompt, completion, total)
        }
        None => (0, 0, 0),
    }
}

fn estimate_from_response_body(response: &Value, _model: &str) -> (i64, i64, i64) {
    let mut estimated_completion = 0i64;

    if let Some(choices) = response.get("choices").and_then(|v| v.as_array()) {
        for choice in choices {
            // Check message.content
            if let Some(msg) = choice.get("message").and_then(|v| v.as_object()) {
                if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                    estimated_completion += estimate_tokens(content);
                }
            }
            // Check delta.content (streaming format)
            if let Some(delta) = choice.get("delta").and_then(|v| v.as_object()) {
                if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                    estimated_completion += estimate_tokens(content);
                }
            }
        }
    }

    (0, estimated_completion, estimated_completion)
}

pub fn estimate_from_response(response: &Value) -> i64 {
    let (_, completion, _) = estimate_from_response_body(response, "");
    completion
}

/// Extract token usage from a streaming SSE chunk
/// OpenAI streaming format: data: {"choices":[{"delta":...}],"usage":{"prompt_tokens":...}}
pub fn extract_streaming_usage(chunk: &str) -> (i64, i64, i64) {
    // SSE chunks can be "data: " (with space) or "data:" (no space)
    let trimmed = chunk.trim();
    if trimmed.is_empty() {
        return (0, 0, 0);
    }

    let json_str = if trimmed.starts_with("data:") {
        trimmed.strip_prefix("data:").unwrap_or(trimmed).trim_start()
    } else {
        trimmed
    };

    if json_str == "[DONE]" {
        return (0, 0, 0);
    }

    match serde_json::from_str::<Value>(json_str) {
        Ok(v) => extract_openai_usage(&v),
        Err(_) => (0, 0, 0),
    }
}

/// Extract model name from an OpenAI-compatible response
pub fn extract_model_from_response(response: &Value) -> Option<String> {
    response.get("model").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Check if a response indicates a rate limit (429)
pub fn is_rate_limited(status_code: u16) -> bool {
    status_code == 429
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_openai_usage_full() {
        let response = serde_json::json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 200,
                "total_tokens": 300
            }
        });
        let (p, c, t) = extract_openai_usage(&response);
        assert_eq!(p, 100);
        assert_eq!(c, 200);
        assert_eq!(t, 300);
    }

    #[test]
    fn test_extract_openai_usage_no_total() {
        let response = serde_json::json!({
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 75
            }
        });
        let (p, c, t) = extract_openai_usage(&response);
        assert_eq!(p, 50);
        assert_eq!(c, 75);
        assert_eq!(t, 125); // Should calculate from prompt + completion
    }

    #[test]
    fn test_extract_openai_usage_no_usage() {
        let response = serde_json::json!({"id": "chatcmpl-123"});
        let (p, c, t) = extract_openai_usage(&response);
        assert_eq!((p, c, t), (0, 0, 0));
    }

    #[test]
    fn test_extract_streaming_usage() {
        let chunk = r#"data: {"id":"chatcmpl-1","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":200,"total_tokens":300}}"#;
        let (p, c, t) = extract_streaming_usage(chunk);
        assert_eq!(p, 100);
        assert_eq!(c, 200);
        assert_eq!(t, 300);
    }

    #[test]
    fn test_extract_streaming_usage_done() {
        let (p, c, t) = extract_streaming_usage("data: [DONE]");
        assert_eq!((p, c, t), (0, 0, 0));
    }

    #[test]
    fn test_extract_streaming_usage_no_usage() {
        let chunk = r#"data: {"id":"chatcmpl-1","choices":[{"delta":{"content":"hello"}}"#;
        let (p, c, t) = extract_streaming_usage(chunk);
        assert_eq!((p, c, t), (0, 0, 0));
    }

    #[test]
    fn test_extract_streaming_usage_no_space() {
        // MiniMax format: data:{...} (no space after colon)
        let chunk = r#"data:{"id":"chatcmpl-1","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":200,"total_tokens":300}}"#;
        let (p, c, t) = extract_streaming_usage(chunk);
        assert_eq!(p, 100);
        assert_eq!(c, 200);
        assert_eq!(t, 300);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert!(estimate_tokens("hello world") > 0);
        assert!(estimate_tokens("你好世界") > 0);
    }

    #[test]
    fn test_estimate_from_response_body() {
        let response = serde_json::json!({
            "model": "gpt-4",
            "choices": [{"message": {"content": "Hello world"}}]
        });
        let (p, c, t) = estimate_from_response_body(&response, "gpt-4");
        assert_eq!(p, 0);
        assert!(c > 0);
        assert_eq!(t, c);
    }

    #[test]
    fn test_extract_model_from_response() {
        let response = serde_json::json!({"model": "gpt-4-0613"});
        assert_eq!(extract_model_from_response(&response), Some("gpt-4-0613".to_string()));
    }

    #[test]
    fn test_is_rate_limited() {
        assert!(is_rate_limited(429));
        assert!(!is_rate_limited(200));
        assert!(!is_rate_limited(500));
    }
}
