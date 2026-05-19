use llm_gateway::db::Database;
use sqlx::SqlitePool;

async fn test_db() -> Database {
    Database::new_in_memory().await.expect("Failed to create test DB")
}

async fn setup_provider(db: &Database, id: &str) {
    db.create_provider_simple(
        id, id, "https://api.test.com", "openai", "api_key",
        Some("test-key"), None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1,
    ).await.unwrap();
}

async fn setup_api_key(db: &Database, id: &str) {
    db.create_api_key(id, id, &format!("lgk-{}", id), &format!("lgk-{}", id), None).await.unwrap();
}

#[tokio::test]
async fn test_cleanup_mappings_removes_mapping_on_model_delete() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    
    // Add provider model
    db.add_provider_model("test-provider", "gpt-4").await.unwrap();
    
    // Create unified model and mapping
    db.create_model("my-gpt4", "My GPT-4", None, "chat", 0, None).await.unwrap();
    db.add_model_mapping("my-gpt4", "test-provider", "gpt-4", 1, 1.0).await.unwrap();
    
    // Verify mapping exists
    let model = db.get_model_with_mappings("my-gpt4").await.unwrap();
    assert!(model.is_some());
    assert_eq!(model.unwrap().mappings.len(), 1);
    
    // Cleanup mappings when provider model is removed
    db.cleanup_mappings_for_provider_model("test-provider", "gpt-4").await.unwrap();
    
    // Unified model should be deleted since it has no more mappings
    let model = db.get_model_with_mappings("my-gpt4").await.unwrap();
    assert!(model.is_none());
}

#[tokio::test]
async fn test_cleanup_mappings_keeps_model_with_other_mappings() {
    let db = test_db().await;
    setup_provider(&db, "provider-a").await;
    setup_provider(&db, "provider-b").await;
    
    // Add provider models
    db.add_provider_model("provider-a", "gpt-4").await.unwrap();
    db.add_provider_model("provider-b", "gpt-4").await.unwrap();
    
    // Create unified model with two mappings
    db.create_model("my-gpt4", "My GPT-4", None, "chat", 0, None).await.unwrap();
    db.add_model_mapping("my-gpt4", "provider-a", "gpt-4", 1, 1.0).await.unwrap();
    db.add_model_mapping("my-gpt4", "provider-b", "gpt-4", 1, 1.0).await.unwrap();
    
    // Cleanup mappings for provider-a only
    db.cleanup_mappings_for_provider_model("provider-a", "gpt-4").await.unwrap();
    
    // Unified model should still exist with provider-b mapping
    let model = db.get_model_with_mappings("my-gpt4").await.unwrap();
    assert!(model.is_some());
    let model = model.unwrap();
    assert_eq!(model.mappings.len(), 1);
    assert_eq!(model.mappings[0].mapping.provider_id, "provider-b");
}

#[tokio::test]
async fn test_cleanup_mappings_noop_when_no_mapping() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    
    // Should not error
    db.cleanup_mappings_for_provider_model("test-provider", "nonexistent-model").await.unwrap();
}

#[tokio::test]
async fn test_query_request_logs_lightweight_basic() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    setup_api_key(&db, "test-key").await;
    let api_key = db.get_api_key_by_key("lgk-test-key").await.unwrap().unwrap();
    
    // Insert a log with large body
    db.insert_request_log(
        &api_key.id, "test-provider", Some("gpt-4"),
        "/v1/chat/completions", "POST",
        None,
        Some("{\"messages\":[{\"role\":\"user\",\"content\":\"very long request body\"}]}"),
        Some(200), None,
        Some("{\"choices\":[{\"message\":{\"content\":\"very long response body\"}}]}"),
        100, 50, 150, Some(500), true, false, None,
    ).await.unwrap();
    
    // Query lightweight
    let logs = db.query_request_logs_lightweight(None, None, None, None, None, None, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].model, Some("gpt-4".to_string()));
    assert_eq!(logs[0].response_status, Some(200));
    assert_eq!(logs[0].prompt_tokens, 100);
    assert_eq!(logs[0].completion_tokens, 50);
    assert_eq!(logs[0].duration_ms, Some(500));
}

#[tokio::test]
async fn test_query_request_logs_lightweight_with_search() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    setup_api_key(&db, "test-key").await;
    let api_key = db.get_api_key_by_key("lgk-test-key").await.unwrap().unwrap();
    
    // Insert two logs
    db.insert_request_log(
        &api_key.id, "test-provider", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, Some("hello world request"), Some(200), None, Some("response data"),
        100, 50, 150, Some(500), true, false, None,
    ).await.unwrap();
    
    db.insert_request_log(
        &api_key.id, "test-provider", Some("gpt-3.5"), "/v1/chat/completions", "POST",
        None, Some("different content"), Some(200), None, Some("other response"),
        50, 25, 75, Some(300), false, false, None,
    ).await.unwrap();
    
    // Search should still work even with lightweight query
    let logs = db.query_request_logs_lightweight(None, None, None, None, Some("hello"), None, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].model, Some("gpt-4".to_string()));
}

#[tokio::test]
async fn test_query_request_logs_lightweight_with_model_filter() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    setup_api_key(&db, "test-key").await;
    let api_key = db.get_api_key_by_key("lgk-test-key").await.unwrap().unwrap();
    
    // Insert two logs with different models
    db.insert_request_log(
        &api_key.id, "test-provider", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, None, Some(200), None, None,
        100, 50, 150, Some(500), true, false, None,
    ).await.unwrap();
    
    db.insert_request_log(
        &api_key.id, "test-provider", Some("gpt-3.5"), "/v1/chat/completions", "POST",
        None, None, Some(200), None, None,
        50, 25, 75, Some(300), false, false, None,
    ).await.unwrap();
    
    // Filter by model
    let logs = db.query_request_logs_lightweight(None, None, None, None, None, Some("gpt-4"), 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].model, Some("gpt-4".to_string()));
}

#[tokio::test]
async fn test_query_request_logs_lightweight_with_provider_filter() {
    let db = test_db().await;
    setup_provider(&db, "provider-a").await;
    setup_provider(&db, "provider-b").await;
    setup_api_key(&db, "test-key").await;
    let api_key = db.get_api_key_by_key("lgk-test-key").await.unwrap().unwrap();
    
    // Insert logs for both providers
    db.insert_request_log(
        &api_key.id, "provider-a", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, None, Some(200), None, None,
        100, 50, 150, Some(500), true, false, None,
    ).await.unwrap();
    
    db.insert_request_log(
        &api_key.id, "provider-b", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, None, Some(200), None, None,
        50, 25, 75, Some(300), false, false, None,
    ).await.unwrap();
    
    // Filter by provider
    let logs = db.query_request_logs_lightweight(None, Some("provider-a"), None, None, None, None, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].provider_id, "provider-a");
}

#[tokio::test]
async fn test_query_request_logs_lightweight_pagination() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    setup_api_key(&db, "test-key").await;
    let api_key = db.get_api_key_by_key("lgk-test-key").await.unwrap().unwrap();
    
    // Insert 5 logs
    for i in 0..5 {
        db.insert_request_log(
            &api_key.id, "test-provider", Some("gpt-4"), "/v1/chat/completions", "POST",
            None, None, Some(200), None, None,
            100 + i, 50 + i, 150 + i * 2, Some(500 + i * 100), true, false, None,
        ).await.unwrap();
    }
    
    let page1 = db.query_request_logs_lightweight(None, None, None, None, None, None, 2, 0).await.unwrap();
    assert_eq!(page1.len(), 2);
    
    let page2 = db.query_request_logs_lightweight(None, None, None, None, None, None, 2, 2).await.unwrap();
    assert_eq!(page2.len(), 2);
    
    let page3 = db.query_request_logs_lightweight(None, None, None, None, None, None, 2, 4).await.unwrap();
    assert_eq!(page3.len(), 1);
}

#[tokio::test]
async fn test_request_log_list_row_has_error_fields() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    setup_api_key(&db, "test-key").await;
    let api_key = db.get_api_key_by_key("lgk-test-key").await.unwrap().unwrap();
    
    // Insert a log with error
    db.insert_request_log(
        &api_key.id, "test-provider", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, None, Some(503), None, None,
        0, 0, 0, Some(100), true, false,
        Some("RecvFromEngineError:Engine Busy"),
    ).await.unwrap();
    
    let logs = db.query_request_logs_lightweight(None, None, None, None, None, None, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].response_status, Some(503));
    assert_eq!(logs[0].error_message, Some("RecvFromEngineError:Engine Busy".to_string()));
    assert!(!logs[0].is_throttled);
}

#[tokio::test]
async fn test_request_log_list_row_has_throttle_flag() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    setup_api_key(&db, "test-key").await;
    let api_key = db.get_api_key_by_key("lgk-test-key").await.unwrap().unwrap();
    
    // Insert a throttled log
    db.insert_request_log(
        &api_key.id, "test-provider", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, None, Some(429), None, None,
        0, 0, 0, Some(50), true, true,
        Some("rate_limit_exceeded"),
    ).await.unwrap();
    
    let logs = db.query_request_logs_lightweight(None, None, None, None, None, None, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].is_throttled);
    assert_eq!(logs[0].response_status, Some(429));
}

#[tokio::test]
async fn test_cleanup_mappings_multiple_models_same_provider() {
    let db = test_db().await;
    setup_provider(&db, "test-provider").await;
    
    // Add two provider models
    db.add_provider_model("test-provider", "gpt-4").await.unwrap();
    db.add_provider_model("test-provider", "gpt-3.5").await.unwrap();
    
    // Create two unified models
    db.create_model("my-gpt4", "My GPT-4", None, "chat", 0, None).await.unwrap();
    db.add_model_mapping("my-gpt4", "test-provider", "gpt-4", 1, 1.0).await.unwrap();
    
    db.create_model("my-gpt35", "My GPT-3.5", None, "chat", 0, None).await.unwrap();
    db.add_model_mapping("my-gpt35", "test-provider", "gpt-3.5", 1, 1.0).await.unwrap();
    
    // Cleanup only gpt-4 mapping
    db.cleanup_mappings_for_provider_model("test-provider", "gpt-4").await.unwrap();
    
    // my-gpt4 should be deleted
    assert!(db.get_model_with_mappings("my-gpt4").await.unwrap().is_none());
    
    // my-gpt35 should still exist
    let model = db.get_model_with_mappings("my-gpt35").await.unwrap();
    assert!(model.is_some());
    assert_eq!(model.unwrap().mappings.len(), 1);
}
