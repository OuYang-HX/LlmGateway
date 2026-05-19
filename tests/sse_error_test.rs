use llm_gateway::proxy::handler::{is_rate_limit_error, strip_thinking_tags};

#[test]
fn test_is_rate_limit_error_common_patterns() {
    // OpenAI patterns
    assert!(is_rate_limit_error("rate_limit_exceeded"));
    assert!(is_rate_limit_error("insufficient_quota"));
    assert!(is_rate_limit_error("quota exceeded"));
    assert!(is_rate_limit_error("Rate limit hit"));
    
    // Xunfei patterns
    assert!(is_rate_limit_error("NotEnoughCvError"));
    
    // Generic patterns
    assert!(is_rate_limit_error("too many requests"));
    assert!(is_rate_limit_error("throttled"));
    assert!(is_rate_limit_error("credits depleted"));
    assert!(is_rate_limit_error("balance insufficient"));
    assert!(is_rate_limit_error("usage limit reached"));
    assert!(is_rate_limit_error("billing limit exceeded"));
    assert!(is_rate_limit_error("capacity exceeded"));
}

#[test]
fn test_is_rate_limit_error_negative_cases() {
    // Engine Busy is NOT a rate limit error — it's a server error
    assert!(!is_rate_limit_error("RecvFromEngineError:Engine Busy"));
    assert!(!is_rate_limit_error("Internal server error"));
    assert!(!is_rate_limit_error("Bad request"));
    assert!(!is_rate_limit_error("Model not found"));
    assert!(!is_rate_limit_error("Invalid API key"));
    assert!(!is_rate_limit_error("Connection timeout"));
}

#[test]
fn test_is_rate_limit_error_case_insensitive() {
    assert!(is_rate_limit_error("RATE_LIMIT_EXCEEDED"));
    assert!(is_rate_limit_error("Rate_Limit_Exceeded"));
    assert!(is_rate_limit_error("QUOTA EXCEEDED"));
    assert!(is_rate_limit_error("Throttled"));
}

#[test]
fn test_strip_thinking_tags_basic() {
    let input = "Hello<think>\nLet me think\n</think> World";
    let result = strip_thinking_tags(input);
    assert_eq!(result, "Hello World");
}

#[test]
fn test_strip_thinking_tags_multiline() {
    let input = "Hello<think>\nStep 1: First\nStep 2: Second\n</think>\n\nAnswer";
    let result = strip_thinking_tags(input);
    assert_eq!(result, "Hello\nAnswer");
}

#[test]
fn test_strip_thinking_tags_no_tags() {
    let input = "Hello World";
    let result = strip_thinking_tags(input);
    assert_eq!(result, "Hello World");
}

#[test]
fn test_strip_thinking_tags_empty_content() {
    let input = "<think></think>Answer";
    let result = strip_thinking_tags(input);
    assert_eq!(result, "Answer");
}

#[test]
fn test_strip_thinking_tags_multiple_sections() {
    let input = "Start<think>reasoning1</think>Middle<think>reasoning2</think>End";
    let result = strip_thinking_tags(input);
    assert!(result.contains("Start"));
    assert!(result.contains("Middle"));
    assert!(result.contains("End"));
    assert!(!result.contains("reasoning"));
}

#[test]
fn test_is_rate_limit_error_xunfei_engine_busy_not_rate_limit() {
    // This is the exact error from the bug report — it should NOT be treated as rate limit
    let error_msg = "Xunfei request failed with Sid: cht000b17dd@dx19e3fac06cfb87e700 code: 10010, msg: RecvFromEngineError:Engine Busy, timeStamp:18:00:25.596";
    assert!(!is_rate_limit_error(error_msg));
}

#[test]
fn test_is_rate_limit_error_xunfei_concurrent_limit() {
    // Xunfei concurrent limit IS a rate limit
    let error_msg = "NotEnoughCvError: concurrent limit exceeded";
    assert!(is_rate_limit_error(error_msg));
}
