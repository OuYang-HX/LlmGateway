use llm_gateway::db::Database;
use llm_gateway::proxy::LlmProxy;
use llm_gateway::auth::AuthManager;
use llm_gateway::usage;
use llm_gateway::stats::StatsCollector;
use std::sync::Arc;

async fn test_db() -> Arc<Database> {
    Arc::new(Database::new_in_memory().await.unwrap())
}

// ==================== Usage Integration with Proxy Tests ====================

#[tokio::test]
async fn test_usage_extraction_from_openai_response() {
    // Simulate an OpenAI response and verify usage extraction
    let response = serde_json::json!({
        "id": "chatcmpl-abc123",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "gpt-4",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello!"},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 9,
            "completion_tokens": 12,
            "total_tokens": 21
        }
    });

    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 9);
    assert_eq!(c, 12);
    assert_eq!(t, 21);
}

#[tokio::test]
async fn test_usage_extraction_from_minimax_response() {
    // MiniMax response format (OpenAI-compatible)
    let response = serde_json::json!({
        "id": "cmpl-123",
        "choices": [{"message": {"content": "Hi"}, "finish_reason": "stop"}],
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

#[tokio::test]
async fn test_usage_extraction_from_astron_response() {
    // Astron/Xunfei response format (OpenAI-compatible via proxy)
    let response = serde_json::json!({
        "id": "chatcmpl-xyz",
        "model": "astron-code-latest",
        "choices": [{"message": {"role": "assistant", "content": "Code here"}, "finish_reason": "stop"}],
        "usage": {
            "prompt_tokens": 200,
            "completion_tokens": 500,
            "total_tokens": 700
        }
    });

    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!(p, 200);
    assert_eq!(c, 500);
    assert_eq!(t, 700);
}

#[tokio::test]
async fn test_usage_extraction_error_response() {
    // Error responses don't have usage
    let response = serde_json::json!({
        "error": {"message": "Rate limit exceeded", "type": "rate_limit_error", "code": "429"}
    });

    let (p, c, t) = usage::extract_openai_usage(&response);
    assert_eq!((p, c, t), (0, 0, 0));
}

#[tokio::test]
async fn test_proxy_logs_usage_from_db() {
    // Verify that when we log a request with actual token counts,
    // they are correctly stored and retrievable
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Log a request with actual token counts (simulating what forward_and_collect does)
    db.insert_request_log(
        "key", "prov", Some("gpt-4"),
        "/v1/chat/completions", "POST",
        None, Some(r#"{"model":"gpt-4","messages":[{"role":"user","content":"hi"}]}"#),
        Some(200), None,
        Some(r#"{"id":"chatcmpl-1","usage":{"prompt_tokens":9,"completion_tokens":12,"total_tokens":21}}"#),
        9, 12, 21,
        Some(1500), false, false, None,
    ).await.unwrap();

    // Verify the stats reflect actual token counts
    let stats = db.get_stats(None, None, None, None).await.unwrap();
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.total_prompt_tokens, 9);
    assert_eq!(stats.total_completion_tokens, 12);
    assert_eq!(stats.total_tokens, 21);
}

#[tokio::test]
async fn test_proxy_logs_usage_per_api_key() {
    let db = test_db().await;
    db.create_api_key("alice", "Alice", "hash-a", "lgk-a", None).await.unwrap();
    db.create_api_key("bob", "Bob", "hash-b", "lgk-b", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Alice's request
    db.insert_request_log(
        "alice", "prov", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(500), false, false, None,
    ).await.unwrap();

    // Bob's request
    db.insert_request_log(
        "bob", "prov", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        50, 75, 125, Some(300), false, false, None,
    ).await.unwrap();

    let alice_stats = db.get_stats(Some("alice"), None, None, None).await.unwrap();
    assert_eq!(alice_stats.total_prompt_tokens, 100);
    assert_eq!(alice_stats.total_tokens, 300);

    let bob_stats = db.get_stats(Some("bob"), None, None, None).await.unwrap();
    assert_eq!(bob_stats.total_prompt_tokens, 50);
    assert_eq!(bob_stats.total_tokens, 125);
}

#[tokio::test]
async fn test_proxy_logs_usage_per_provider() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("openai", "OpenAI", "https://api.openai.com/v1", "openai", "api_key", Some("key1"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("internal", "Internal", "https://internal.com/v1", "openai", "api_key", Some("key2"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 2).await.unwrap();

    // Request to OpenAI
    db.insert_request_log(
        "key", "openai", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(800), false, false, None,
    ).await.unwrap();

    // Request to Internal
    db.insert_request_log(
        "key", "internal", Some("internal-model"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        50, 100, 150, Some(400), false, false, None,
    ).await.unwrap();

    let openai_stats = db.get_stats(None, Some("openai"), None, None).await.unwrap();
    assert_eq!(openai_stats.total_tokens, 300);
    assert_eq!(openai_stats.avg_duration_ms, 800.0);

    let internal_stats = db.get_stats(None, Some("internal"), None, None).await.unwrap();
    assert_eq!(internal_stats.total_tokens, 150);
    assert_eq!(internal_stats.avg_duration_ms, 400.0);
}

#[tokio::test]
async fn test_streaming_usage_extraction_from_chunks() {
    // Simulate processing multiple SSE chunks
    let chunks = vec![
        r#"data: {"id":"chatcmpl-1","choices":[{"delta":{"role":"assistant","content":""}}]}"#,
        r#"data: {"id":"chatcmpl-1","choices":[{"delta":{"content":"Hello"}}]}"#,
        r#"data: {"id":"chatcmpl-1","choices":[{"delta":{"content":" world"}}]}"#,
        r#"data: {"id":"chatcmpl-1","choices":[{"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#,
        "data: [DONE]",
    ];

    let mut total_prompt = 0i64;
    let mut total_completion = 0i64;
    let mut total_tokens = 0i64;

    for chunk in &chunks {
        let (p, c, t) = usage::extract_streaming_usage(chunk);
        total_prompt += p;
        total_completion += c;
        total_tokens += t;
    }

    // Only the second-to-last chunk has usage
    assert_eq!(total_prompt, 10);
    assert_eq!(total_completion, 5);
    assert_eq!(total_tokens, 15);
}

#[tokio::test]
async fn test_proxy_model_extraction_from_response() {
    // When the response includes a model field, we should prefer it
    let response = serde_json::json!({
        "id": "chatcmpl-1",
        "model": "gpt-4-0613",
        "choices": [],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    });

    let model = usage::extract_model_from_response(&response);
    assert_eq!(model, Some("gpt-4-0613".to_string()));
}

#[tokio::test]
async fn test_proxy_response_struct() {
    // Verify the ProxyResponse struct works correctly
    let db = test_db().await;
    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let stats: Arc<llm_gateway::stats::StatsCollector> = Arc::new(llm_gateway::stats::StatsCollector::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager, stats);

    // Just verify the proxy can be created and the method exists
    assert!(proxy.http_client().get("https://example.com").build().is_ok());
}

#[tokio::test]
async fn test_dashboard_summary_with_actual_usage() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Log some requests with actual token counts
    for _ in 0..5 {
        db.insert_request_log(
            "key", "prov", Some("gpt-4"), "/v1/chat", "POST",
            None, None, Some(200), None, None,
            100, 200, 300, Some(500), false, false, None,
        ).await.unwrap();
    }

    let summary = db.get_dashboard_summary().await.unwrap();
    assert_eq!(summary.total_api_keys, 1);
    assert_eq!(summary.active_providers, 1);
    // The 24h stats should reflect the logged requests
    assert!(summary.total_requests_24h >= 5);
    assert!(summary.total_tokens_24h >= 1500);
}

#[tokio::test]
async fn test_time_bucketed_stats_with_actual_usage() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log(
        "key", "prov", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(500), false, false, None,
    ).await.unwrap();

    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let end = (now + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();

    let buckets = db.get_time_bucketed_stats(None, None, &start, &end, "day").await.unwrap();
    assert!(!buckets.is_empty());
    assert_eq!(buckets[0].prompt_tokens, 100);
    assert_eq!(buckets[0].completion_tokens, 200);
    assert_eq!(buckets[0].total_tokens, 300);
}
