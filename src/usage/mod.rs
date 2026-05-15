/// Token usage extraction from LLM API responses
use serde_json::Value;

/// Extract token usage from an OpenAI-compatible response
pub fn extract_openai_usage(response: &Value) -> (i64, i64, i64) {
    let usage = response.get("usage");
    match usage {
        Some(u) => {
            let prompt = u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let completion = u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let total = u.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(prompt + completion);
            (prompt, completion, total)
        }
        None => (0, 0, 0),
    }
}

/// Extract token usage from a streaming SSE chunk
/// OpenAI streaming format: data: {"choices":[{"delta":...}],"usage":{"prompt_tokens":...}}
pub fn extract_streaming_usage(chunk: &str) -> (i64, i64, i64) {
    // SSE chunks start with "data: " prefix
    let json_str = chunk.strip_prefix("data: ").unwrap_or(chunk).trim();

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
