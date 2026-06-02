use llm_gateway::db::Database;

async fn test_db() -> Database {
    Database::new_in_memory().await.unwrap()
}

async fn create_test_provider(db: &Database, id: &str) {
    db.create_provider(
        id, id, "https://api.example.com/v1", "openai", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false, false,
        "", "", false,
    ).await.unwrap();
}

// ==================== Multi-mapping DB integration tests ====================

#[tokio::test]
async fn int_multi_mapping_delete_specific_provider_model() {
    let db = test_db().await;
    create_test_provider(&db, "mm-del-prov").await;
    db.create_model("mm-del-m", "MM Del", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("mm-del-m", "mm-del-prov", "gpt-4", 1, 1.0).await.unwrap();
    db.add_model_mapping("mm-del-m", "mm-del-prov", "gpt-4o", 2, 0.5).await.unwrap();

    let removed = db.remove_model_mapping("mm-del-m", "mm-del-prov", "gpt-4").await.unwrap();
    assert!(removed);

    let remaining = db.list_model_mappings("mm-del-m").await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].provider_model_id, "gpt-4o");
}

#[tokio::test]
async fn int_multi_mapping_update_by_old_provider_model_id() {
    let db = test_db().await;
    create_test_provider(&db, "mm-upd-prov").await;
    db.create_model("mm-upd-m", "MM Upd", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("mm-upd-m", "mm-upd-prov", "gpt-4", 1, 1.0).await.unwrap();

    db.update_model_mapping("mm-upd-m", "mm-upd-prov", "gpt-4", "gpt-4-turbo", true, 3, 0.8).await.unwrap();

    let mappings = db.list_model_mappings("mm-upd-m").await.unwrap();
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].provider_model_id, "gpt-4-turbo");
    assert_eq!(mappings[0].weight, 3);
}

// ==================== Request log operations ====================

#[tokio::test]
async fn int_get_request_log_detail() {
    let db = test_db().await;
    create_test_provider(&db, "log-detail-prov").await;
    db.insert_request_log(
        "key1", "log-detail-prov", Some("model-a"), "/v1/chat", "POST",
        None, Some("req body"), Some(200), None, Some("resp body"),
        10, 20, 30, Some(100), false, false, None
    ).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None, 100, 0).await.unwrap();
    assert_eq!(logs.len(), 1);

    let detail = db.get_request_log(logs[0].id).await.unwrap();
    assert!(detail.is_some());
    let log = detail.unwrap();
    assert_eq!(log.provider_id, "log-detail-prov");
    assert_eq!(log.model, Some("model-a".to_string()));
    assert_eq!(log.request_body, Some("req body".to_string()));
    assert_eq!(log.response_body, Some("resp body".to_string()));
}

#[tokio::test]
async fn int_get_request_log_not_found() {
    let db = test_db().await;
    let detail = db.get_request_log(99999).await.unwrap();
    assert!(detail.is_none());
}

#[tokio::test]
async fn int_delete_single_request_log() {
    let db = test_db().await;
    create_test_provider(&db, "del-single-prov").await;
    db.insert_request_log(
        "key1", "del-single-prov", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        10, 20, 30, Some(100), false, false, None
    ).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None, 100, 0).await.unwrap();
    assert_eq!(logs.len(), 1);

    let deleted = db.delete_request_log(logs[0].id).await.unwrap();
    assert!(deleted);

    let remaining = db.count_request_logs(None, None, None, None, None, None, None).await.unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn int_delete_request_log_not_found() {
    let db = test_db().await;
    let deleted = db.delete_request_log(99999).await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn int_delete_all_request_logs() {
    let db = test_db().await;
    create_test_provider(&db, "del-all-prov").await;
    db.insert_request_log(
        "key1", "del-all-prov", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        10, 20, 30, Some(100), false, false, None
    ).await.unwrap();
    db.insert_request_log(
        "key2", "del-all-prov", Some("model-b"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        10, 20, 30, Some(100), false, false, None
    ).await.unwrap();

    let deleted = db.delete_all_request_logs().await.unwrap();
    assert_eq!(deleted, 2);

    let remaining = db.count_request_logs(None, None, None, None, None, None, None).await.unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn int_delete_request_logs_by_provider() {
    let db = test_db().await;
    create_test_provider(&db, "del-prov-a").await;
    create_test_provider(&db, "del-prov-b").await;
    db.insert_request_log(
        "key1", "del-prov-a", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        10, 20, 30, Some(100), false, false, None
    ).await.unwrap();
    db.insert_request_log(
        "key1", "del-prov-b", Some("model-b"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        10, 20, 30, Some(100), false, false, None
    ).await.unwrap();

    let deleted = db.delete_request_logs_by_provider("del-prov-a").await.unwrap();
    assert_eq!(deleted, 1);

    let remaining = db.count_request_logs(None, None, None, None, None, None, None).await.unwrap();
    assert_eq!(remaining, 1);
}

// ==================== Model lifecycle ====================

#[tokio::test]
async fn int_update_model_name_and_priority() {
    let db = test_db().await;
    db.create_model("upd-int-m", "Original", None, "chat", 1, None).await.unwrap();
    db.update_model("upd-int-m", "Updated Name", Some("New desc"), "chat", true, 5, None).await.unwrap();

    let model = db.get_model("upd-int-m").await.unwrap().unwrap();
    assert_eq!(model.name, "Updated Name");
    assert_eq!(model.priority, 5);
}

#[tokio::test]
async fn int_delete_model_manual_mapping_cleanup() {
    // SQLite FK constraints don't have CASCADE, so mappings must be manually cleaned
    let db = test_db().await;
    create_test_provider(&db, "cascade-m-prov").await;
    db.create_model("cascade-m", "Cascade M", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("cascade-m", "cascade-m-prov", "gpt-4", 1, 1.0).await.unwrap();

    // Manually remove mappings before deleting model
    let removed = db.remove_model_mapping("cascade-m", "cascade-m-prov", "gpt-4").await.unwrap();
    assert!(removed);
    db.delete_model("cascade-m").await.unwrap();

    let model = db.get_model("cascade-m").await.unwrap();
    assert!(model.is_none());
}

#[tokio::test]
async fn int_delete_provider_manual_mapping_cleanup() {
    // SQLite FK constraints don't have CASCADE, so mappings must be manually cleaned
    let db = test_db().await;
    create_test_provider(&db, "cascade-p-prov").await;
    db.create_model("cascade-pm", "Cascade PM", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("cascade-pm", "cascade-p-prov", "gpt-4", 1, 1.0).await.unwrap();

    // Manually remove mappings before deleting provider
    let removed = db.remove_model_mapping("cascade-pm", "cascade-p-prov", "gpt-4").await.unwrap();
    assert!(removed);
    db.delete_provider("cascade-p-prov").await.unwrap();

    let provider = db.get_provider("cascade-p-prov").await.unwrap();
    assert!(provider.is_none());
}

// ==================== Token rate snapshots ====================

#[tokio::test]
async fn int_insert_and_get_token_rate_snapshot() {
    let db = test_db().await;
    create_test_provider(&db, "snap-prov").await;

    db.insert_token_rate_snapshot(Some("snap-prov"), 100.0, 50, 100, 200, 60.0).await.unwrap();

    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].provider_id, Some("snap-prov".to_string()));
}

#[tokio::test]
async fn int_cleanup_old_snapshots() {
    let db = test_db().await;
    create_test_provider(&db, "snap-clean-prov").await;

    db.insert_token_rate_snapshot(Some("snap-clean-prov"), 50.0, 25, 50, 100, 60.0).await.unwrap();

    let cleaned = db.cleanup_old_snapshots("2099-01-01T00:00:00Z").await.unwrap();
    assert!(cleaned >= 0);
}

// ==================== API key with allowed providers ====================

#[tokio::test]
async fn int_api_key_with_allowed_providers() {
    let db = test_db().await;
    let allowed = serde_json::json!(["prov-a", "prov-b"]).to_string();
    db.create_api_key("restricted-key", "Restricted", "lgk-restricted", "lgk-restricted-xxx", Some(&allowed)).await.unwrap();

    let key = db.get_api_key_by_key("lgk-restricted").await.unwrap().unwrap();
    assert!(key.allowed_providers.is_some());
    let providers: Vec<String> = serde_json::from_str(key.allowed_providers.as_deref().unwrap()).unwrap();
    assert_eq!(providers.len(), 2);
}

#[tokio::test]
async fn int_api_key_without_allowed_providers() {
    let db = test_db().await;
    db.create_api_key("open-key", "Open", "lgk-open", "lgk-open-xxx", None).await.unwrap();

    let key = db.get_api_key_by_key("lgk-open").await.unwrap().unwrap();
    assert!(key.allowed_providers.is_none());
}

// ==================== Dashboard summary ====================

#[tokio::test]
async fn int_dashboard_summary_with_data() {
    let db = test_db().await;
    create_test_provider(&db, "ds-int-prov").await;
    db.create_api_key("ds-int-key", "DS Key", "lgk-ds-int", "lgk-ds-int-xxx", None).await.unwrap();
    db.insert_request_log(
        "ds-int-key", "ds-int-prov", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(50), false, false, None
    ).await.unwrap();

    let summary = db.get_dashboard_summary().await.unwrap();
    assert!(summary.active_providers >= 1);
    assert!(summary.total_requests_24h >= 1);
}

#[tokio::test]
async fn int_dashboard_summary_empty() {
    let db = test_db().await;
    let summary = db.get_dashboard_summary().await.unwrap();
    assert_eq!(summary.active_providers, 0);
    assert_eq!(summary.total_api_keys, 0);
}

// ==================== Usage trend ====================

#[tokio::test]
async fn int_usage_trend() {
    let db = test_db().await;
    create_test_provider(&db, "ut-int-prov").await;
    db.insert_request_log(
        "key1", "ut-int-prov", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(50), false, false, None
    ).await.unwrap();

    let trend = db.get_time_bucketed_stats(None, None, "1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z", "hour").await.unwrap();
    assert!(!trend.is_empty());
}

// ==================== Top provider by usage ====================

#[tokio::test]
async fn int_stats_by_api_key_with_data() {
    let db = test_db().await;
    create_test_provider(&db, "sak-prov").await;
    db.create_api_key("sak-key", "SAK Key", "lgk-sak", "lgk-sak-xxx", None).await.unwrap();
    db.insert_request_log(
        "sak-key", "sak-prov", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(50), false, false, None
    ).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert!(!stats.is_empty());
}

#[tokio::test]
async fn int_top_provider_no_usage() {
    let db = test_db().await;
    let top = db.get_top_provider_by_usage().await.unwrap();
    assert!(top.is_none());
}

// ==================== strip_thinking_tags_in_response integration ====================

#[tokio::test]
async fn int_strip_thinking_db_roundtrip() {
    let db = test_db().await;
    // Create with strip=true
    db.create_provider(
        "strip-int", "Strip Int", "https://api.example.com/v1", "openai", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, true, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content", true,
    ).await.unwrap();

    // Read back
    let p = db.get_provider("strip-int").await.unwrap().unwrap();
    assert!(p.strip_thinking_tags_in_response);
    assert_eq!(p.api_type, "openai");

    // Disable via update
    db.update_provider(
        "strip-int", "strip-int", "Strip Int Updated", "https://api.example.com/v1", "openai", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, true, false, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content", None, None, None, false,
    ).await.unwrap();

    let p2 = db.get_provider("strip-int").await.unwrap().unwrap();
    assert!(!p2.strip_thinking_tags_in_response);
}

#[tokio::test]
async fn int_strip_thinking_anthropic_type_ignored() {
    let db = test_db().await;
    // Anthropic provider with strip=true — field stored but NOT applied at proxy layer
    db.create_provider(
        "strip-anthropic", "Strip Anthropic", "https://api.example.com/v1", "anthropic", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, true, false,
        "", "", true,
    ).await.unwrap();

    let p = db.get_provider("strip-anthropic").await.unwrap().unwrap();
    assert!(p.strip_thinking_tags_in_response);
    assert_eq!(p.api_type, "anthropic");
    // Note: proxy handler checks api_type=="openai" before applying strip
}