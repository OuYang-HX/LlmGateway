use llm_gateway::db::Database;
use llm_gateway::utils;
use llm_gateway::proxy::LlmProxy;
use llm_gateway::auth::AuthManager;
use llm_gateway::stats::StatsCollector;
use llm_gateway::config::*;
use std::sync::Arc;

/// Helper to create an in-memory database for testing
async fn test_db() -> Arc<Database> {
    Arc::new(Database::new_in_memory().await.expect("Failed to create test DB"))
}

// ==================== Database Tests ====================

#[tokio::test]
async fn test_database_creation() {
    let db = Database::new_in_memory().await;
    assert!(db.is_ok(), "Database should be created successfully");
}

#[tokio::test]
async fn test_create_and_get_api_key() {
    let db = test_db().await;
    let id = "test-key-1";
    let name = "Test Key";
    let api_key = "lgk-test-secret-key";
    let key_prefix = "lgk-test-xxx";

    db.create_api_key(id, name, api_key, key_prefix, None).await.unwrap();

    let result = db.get_api_key_by_key(api_key).await;
    assert!(result.is_ok(), "Should find key by value");
    let key_row = result.unwrap().unwrap();
    assert_eq!(key_row.id, id);
    assert_eq!(key_row.name, name);
    assert_eq!(key_row.key_prefix, key_prefix);
    assert!(key_row.is_active);
}

#[tokio::test]
async fn test_create_api_key_with_allowed_providers() {
    let db = test_db().await;
    let providers = serde_json::to_string(&vec!["provider-a".to_string(), "provider-b".to_string()]).unwrap();

    db.create_api_key("key-2", "Restricted Key", "lgk-restricted-key", "lgk-rest", Some(&providers)).await.unwrap();

    let key_row = db.get_api_key_by_key("lgk-restricted-key").await.unwrap().unwrap();
    assert!(key_row.allowed_providers.is_some());
    let parsed: Vec<String> = serde_json::from_str(&key_row.allowed_providers.unwrap()).unwrap();
    assert_eq!(parsed, vec!["provider-a", "provider-b"]);
}

#[tokio::test]
async fn test_list_api_keys() {
    let db = test_db().await;
    db.create_api_key("key-a", "Key A", "hash-a", "lgk-a", None).await.unwrap();
    db.create_api_key("key-b", "Key B", "hash-b", "lgk-b", None).await.unwrap();

    let keys = db.list_api_keys().await.unwrap();
    assert_eq!(keys.len(), 2);
}

#[tokio::test]
async fn test_deactivate_api_key() {
    let db = test_db().await;
    db.create_api_key("key-c", "Key C", "lgk-hash-c", "lgk-c", None).await.unwrap();

    let deactivated = db.deactivate_api_key("key-c").await.unwrap();
    assert!(deactivated, "Should deactivate existing key");

    // Deactivated key should not be found by hash (active only)
    let result = db.get_api_key_by_key("lgk-hash-c").await.unwrap();
    assert!(result.is_none(), "Deactivated key should not be found in active lookup");
}

#[tokio::test]
async fn test_delete_api_key() {
    let db = test_db().await;
    db.create_api_key("key-d", "Key D", "hash-d", "lgk-d", None).await.unwrap();

    let deleted = db.delete_api_key("key-d").await.unwrap();
    assert!(deleted, "Should delete existing key");

    let result = db.get_api_key_by_id("key-d").await.unwrap();
    assert!(result.is_none(), "Deleted key should not be found");
}

#[tokio::test]
async fn test_delete_nonexistent_api_key() {
    let db = test_db().await;
    let deleted = db.delete_api_key("nonexistent").await.unwrap();
    assert!(!deleted, "Should return false for nonexistent key");
}

#[tokio::test]
async fn test_get_api_key_by_id() {
    let db = test_db().await;
    db.create_api_key("key-byid", "Key By ID", "hash-byid", "lgk-byid", None).await.unwrap();

    let result = db.get_api_key_by_id("key-byid").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().name, "Key By ID");
}

#[tokio::test]
async fn test_get_nonexistent_api_key_by_id() {
    let db = test_db().await;
    let result = db.get_api_key_by_id("nonexistent").await.unwrap();
    assert!(result.is_none());
}

// ==================== Provider Tests ====================

#[tokio::test]
async fn test_create_and_get_provider() {
    let db = test_db().await;
    db.create_provider_simple(
        "provider-1",
        "Test Provider",
        "https://api.example.com/v1",
        "openai",
        "api_key",
        Some("sk-test-key"),
        None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ",
        86400, 1,
    ).await.unwrap();

    let provider = db.get_provider("provider-1").await.unwrap().unwrap();
    assert_eq!(provider.id, "provider-1");
    assert_eq!(provider.name, "Test Provider");
    assert_eq!(provider.base_url, "https://api.example.com/v1");
    assert_eq!(provider.api_type, "openai");
    assert_eq!(provider.auth_type, "api_key");
    assert_eq!(provider.api_key, Some("sk-test-key".to_string()));
    assert!(provider.is_active);
}

#[tokio::test]
async fn test_create_provider_with_dynamic_token() {
    let db = test_db().await;
    db.create_provider_simple(
        "provider-dynamic",
        "Dynamic Token Provider",
        "https://internal.example.com/v1",
        "openai",
        "dynamic_token",
        None,
        Some("https://auth.example.com/login"),
        Some("admin"),
        Some("password123"),
        "access_token",
        "refresh_token",
        "X-Auth-Token",
        "",
        28800, 2,
    ).await.unwrap();

    let provider = db.get_provider("provider-dynamic").await.unwrap().unwrap();
    assert_eq!(provider.auth_type, "dynamic_token");
    assert_eq!(provider.token_url, Some("https://auth.example.com/login".to_string()));
    assert_eq!(provider.token_username, Some("admin".to_string()));
    assert_eq!(provider.token_password, Some("password123".to_string()));
    assert_eq!(provider.token_field, "access_token");
    assert_eq!(provider.refresh_token_field, "refresh_token");
    assert_eq!(provider.token_header_field, "X-Auth-Token");
    assert_eq!(provider.token_header_prefix, "");
    assert_eq!(provider.token_expiry_seconds, 28800);
    assert_eq!(provider.weight, 2);
}

#[tokio::test]
async fn test_list_providers() {
    let db = test_db().await;
    db.create_provider_simple("p1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key1"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("p2", "Provider 2", "https://p2.com", "openai", "api_key", Some("key2"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let providers = db.list_providers().await.unwrap();
    assert_eq!(providers.len(), 2);
}

#[tokio::test]
async fn test_list_active_providers() {
    let db = test_db().await;
    db.create_provider_simple("p1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key1"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("p2", "Provider 2", "https://p2.com", "openai", "api_key", Some("key2"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.deactivate_provider("p2").await.unwrap();

    let active = db.list_active_providers().await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "p1");
}

#[tokio::test]
async fn test_delete_provider() {
    let db = test_db().await;
    db.create_provider_simple("p-del", "To Delete", "https://del.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let deleted = db.delete_provider("p-del").await.unwrap();
    assert!(deleted);

    let result = db.get_provider("p-del").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_update_provider_token() {
    let db = test_db().await;
    db.create_provider_simple("p-token", "Token Provider", "https://tp.com", "openai", "dynamic_token", None, Some("https://auth.com"), Some("user"), Some("pass"), "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.update_provider_token("p-token", "new-access-token", Some("new-refresh-token"), "2099-12-31 23:59:59").await.unwrap();

    let provider = db.get_provider("p-token").await.unwrap().unwrap();
    assert_eq!(provider.current_token, Some("new-access-token".to_string()));
    assert_eq!(provider.current_refresh_token, Some("new-refresh-token".to_string()));
    assert_eq!(provider.token_expires_at, Some("2099-12-31 23:59:59".to_string()));
}

#[tokio::test]
async fn test_update_provider_token_without_refresh() {
    let db = test_db().await;
    db.create_provider_simple("p-token2", "Token Provider 2", "https://tp2.com", "openai", "dynamic_token", None, Some("https://auth.com"), Some("user"), Some("pass"), "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.update_provider_token("p-token2", "access-only", None, "2099-12-31 23:59:59").await.unwrap();

    let provider = db.get_provider("p-token2").await.unwrap().unwrap();
    assert_eq!(provider.current_token, Some("access-only".to_string()));
    assert_eq!(provider.current_refresh_token, None);
}

#[tokio::test]
async fn test_delete_nonexistent_provider() {
    let db = test_db().await;
    let deleted = db.delete_provider("nonexistent").await.unwrap();
    assert!(!deleted);
}

// ==================== Request Log Tests ====================

#[tokio::test]
async fn test_insert_and_query_request_logs() {
    let db = test_db().await;
    db.create_api_key("log-key", "Log Key", "log-hash", "lgk-log", None).await.unwrap();
    db.create_provider_simple("log-prov", "Log Provider", "https://lp.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let _log_id = db.insert_request_log(
        "log-key", "log-prov", Some("gpt-4"),
        "/v1/chat/completions", "POST",
        None, Some(r#"{"model":"gpt-4","messages":[{"role":"user","content":"hi"}]}"#),
        Some(200), None, Some(r#"{"id":"chatcmpl-1"}"#),
        10, 20, 30,
        Some(1500), false, false, None,
    ).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].model, Some("gpt-4".to_string()));
    assert_eq!(logs[0].prompt_tokens, 10);
    assert_eq!(logs[0].completion_tokens, 20);
    assert_eq!(logs[0].total_tokens, 30);
    assert_eq!(logs[0].duration_ms, Some(1500));
}

#[tokio::test]
async fn test_query_logs_by_api_key() {
    let db = test_db().await;
    db.create_api_key("key-a", "Key A", "hash-a", "lgk-a", None).await.unwrap();
    db.create_api_key("key-b", "Key B", "hash-b", "lgk-b", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key-a", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 5, 10, 15, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key-b", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 5, 10, 15, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key-a", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 5, 10, 15, Some(100), false, false, None).await.unwrap();

    let logs_a = db.query_request_logs(Some("key-a"), None, None, None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs_a.len(), 2);

    let logs_b = db.query_request_logs(Some("key-b"), None, None, None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs_b.len(), 1);
}

#[tokio::test]
async fn test_query_logs_by_provider() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov-1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("prov-2", "Provider 2", "https://p2.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov-1", None, "/v1/chat", "POST", None, None, Some(200), None, None, 5, 10, 15, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key", "prov-2", None, "/v1/chat", "POST", None, None, Some(200), None, None, 5, 10, 15, Some(100), false, false, None).await.unwrap();

    let logs_1 = db.query_request_logs(None, Some("prov-1"), None, None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs_1.len(), 1);
}

#[tokio::test]
async fn test_count_request_logs() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    for _ in 0..5 {
        db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 5, 10, 15, Some(100), false, false, None).await.unwrap();
    }

    let count = db.count_request_logs(None, None, None, None, None, None).await.unwrap();
    assert_eq!(count, 5);
}

#[tokio::test]
async fn test_throttled_request_log() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(429), None, None, 0, 0, 0, Some(50), false, true, Some("Rate limit exceeded")).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].is_throttled);
    assert_eq!(logs[0].response_status, Some(429));
    assert_eq!(logs[0].error_message, Some("Rate limit exceeded".to_string()));
}

#[tokio::test]
async fn test_error_request_log() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(500), None, None, 0, 0, 0, Some(2000), false, false, Some("Internal server error")).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs[0].error_message, Some("Internal server error".to_string()));
    assert_eq!(logs[0].response_status, Some(500));
}

#[tokio::test]
async fn test_query_logs_with_time_range() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 5, 10, 15, Some(100), false, false, None).await.unwrap();

    // Query with past time range - should find the log
    let past = (chrono::Utc::now() - chrono::Duration::hours(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let future = (chrono::Utc::now() + chrono::Duration::hours(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let logs = db.query_request_logs(None, None, Some(&past), Some(&future), None, None,  10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);

    // Query with far future start time - should find nothing
    let far_future = (chrono::Utc::now() + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let logs = db.query_request_logs(None, None, Some(&far_future), None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs.len(), 0);
}

// ==================== Statistics Tests ====================

#[tokio::test]
async fn test_get_stats() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov", Some("gpt-4"), "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(1500), false, false, None).await.unwrap();
    db.insert_request_log("key", "prov", Some("gpt-4"), "/v1/chat", "POST", None, None, Some(200), None, None, 50, 100, 150, Some(1000), false, false, None).await.unwrap();
    db.insert_request_log("key", "prov", Some("gpt-4"), "/v1/chat", "POST", None, None, Some(429), None, None, 0, 0, 0, Some(50), false, true, Some("Rate limited")).await.unwrap();

    let stats = db.get_stats(None, None, None, None).await.unwrap();
    assert_eq!(stats.total_requests, 3);
    assert_eq!(stats.total_prompt_tokens, 150);
    assert_eq!(stats.total_completion_tokens, 300);
    assert_eq!(stats.total_tokens, 450);
    assert_eq!(stats.throttle_count, 1);
    assert_eq!(stats.error_count, 1);
}

#[tokio::test]
async fn test_get_stats_by_api_key() {
    let db = test_db().await;
    db.create_api_key("key-1", "Key 1", "hash-1", "lgk-1", None).await.unwrap();
    db.create_api_key("key-2", "Key 2", "hash-2", "lgk-2", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key-1", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(1500), false, false, None).await.unwrap();
    db.insert_request_log("key-2", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 50, 100, 150, Some(1000), false, false, None).await.unwrap();

    let stats_1 = db.get_stats(Some("key-1"), None, None, None).await.unwrap();
    assert_eq!(stats_1.total_requests, 1);
    assert_eq!(stats_1.total_tokens, 300);

    let stats_2 = db.get_stats(Some("key-2"), None, None, None).await.unwrap();
    assert_eq!(stats_2.total_requests, 1);
    assert_eq!(stats_2.total_tokens, 150);
}

#[tokio::test]
async fn test_get_stats_by_provider() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov-1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("prov-2", "Provider 2", "https://p2.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov-1", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(1500), false, false, None).await.unwrap();
    db.insert_request_log("key", "prov-2", None, "/v1/chat", "POST", None, None, Some(200), None, None, 50, 100, 150, Some(1000), false, false, None).await.unwrap();

    let stats_1 = db.get_stats(None, Some("prov-1"), None, None).await.unwrap();
    assert_eq!(stats_1.total_requests, 1);
    assert_eq!(stats_1.total_tokens, 300);
}

#[tokio::test]
async fn test_get_stats_with_time_range() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(1500), false, false, None).await.unwrap();

    let past = (chrono::Utc::now() - chrono::Duration::hours(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let future = (chrono::Utc::now() + chrono::Duration::hours(1)).format("%Y-%m-%d %H:%M:%S").to_string();

    let stats = db.get_stats(None, None, Some(&past), Some(&future)).await.unwrap();
    assert_eq!(stats.total_requests, 1);
}

#[tokio::test]
async fn test_get_stats_empty_database() {
    let db = test_db().await;
    let stats = db.get_stats(None, None, None, None).await.unwrap();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.total_tokens, 0);
    assert_eq!(stats.throttle_count, 0);
}

// ==================== Token Rate Tests ====================

#[tokio::test]
async fn test_insert_and_get_token_rate_snapshots() {
    let db = test_db().await;

    db.insert_token_rate_snapshot(None, 150.5, 100, 50, 10).await.unwrap();
    db.insert_token_rate_snapshot(None, 200.0, 130, 70, 12).await.unwrap();

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let snapshots = db.get_token_rate_snapshots(None, &now, 100).await.unwrap();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].tokens_per_second, 150.5);
    assert_eq!(snapshots[1].tokens_per_second, 200.0);
}

#[tokio::test]
async fn test_token_rate_snapshot_by_provider() {
    let db = test_db().await;
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_token_rate_snapshot(Some("prov"), 100.0, 60, 40, 5).await.unwrap();
    db.insert_token_rate_snapshot(None, 50.0, 30, 20, 3).await.unwrap();

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let prov_snapshots = db.get_token_rate_snapshots(Some("prov"), &now, 100).await.unwrap();
    assert_eq!(prov_snapshots.len(), 1);
    assert_eq!(prov_snapshots[0].tokens_per_second, 100.0);
}

#[tokio::test]
async fn test_cleanup_old_snapshots() {
    let db = test_db().await;

    db.insert_token_rate_snapshot(None, 100.0, 50, 50, 5).await.unwrap();

    // Cleanup with a future time should delete the snapshot
    let future = (chrono::Utc::now() + chrono::Duration::hours(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let deleted = db.cleanup_old_snapshots(&future).await.unwrap();
    assert_eq!(deleted, 1, "Should delete snapshot older than future time");
}

// ==================== Utility Tests ====================

#[test]
fn test_sha256_hash() {
    let hash1 = utils::sha256_hash("test-key");
    let hash2 = utils::sha256_hash("test-key");
    let hash3 = utils::sha256_hash("different-key");

    assert_eq!(hash1, hash2, "Same input should produce same hash");
    assert_ne!(hash1, hash3, "Different inputs should produce different hashes");
    assert_eq!(hash1.len(), 64, "SHA-256 hex output should be 64 chars");
}

#[test]
fn test_sha256_hash_empty_string() {
    let hash = utils::sha256_hash("");
    assert_eq!(hash.len(), 64);
    assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn test_sha256_hash_special_characters() {
    let hash = utils::sha256_hash("lgk-测试-key!@#$%");
    assert_eq!(hash.len(), 64);
    // Should be deterministic
    assert_eq!(hash, utils::sha256_hash("lgk-测试-key!@#$%"));
}

// ==================== Proxy Tests ====================

#[tokio::test]
async fn test_proxy_select_provider_round_robin() {
    let db = test_db().await;
    db.create_provider_simple("p1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key1"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("p2", "Provider 2", "https://p2.com", "openai", "api_key", Some("key2"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let mut p1_count = 0;
    let mut p2_count = 0;
    for _ in 0..10 {
        let provider = proxy.select_provider(None).await.unwrap();
        match provider.id.as_str() {
            "p1" => p1_count += 1,
            "p2" => p2_count += 1,
            _ => {}
        }
    }
    assert!(p1_count > 0 && p2_count > 0, "Should distribute across providers");
}

#[tokio::test]
async fn test_proxy_select_provider_with_allowed_list() {
    let db = test_db().await;
    db.create_provider_simple("p1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key1"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("p2", "Provider 2", "https://p2.com", "openai", "api_key", Some("key2"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let allowed = vec!["p1".to_string()];
    for _ in 0..5 {
        let provider = proxy.select_provider(Some(&allowed)).await.unwrap();
        assert_eq!(provider.id, "p1", "Should only select allowed provider");
    }
}

#[tokio::test]
async fn test_proxy_select_provider_no_providers() {
    let db = test_db().await;
    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let result = proxy.select_provider(None).await;
    assert!(result.is_err(), "Should fail with no providers");
}

#[tokio::test]
async fn test_proxy_select_provider_no_matching() {
    let db = test_db().await;
    db.create_provider_simple("p1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key1"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let allowed = vec!["nonexistent".to_string()];
    let result = proxy.select_provider(Some(&allowed)).await;
    assert!(result.is_err(), "Should fail with no matching providers");
}

#[tokio::test]
async fn test_proxy_weighted_selection() {
    let db = test_db().await;
    db.create_provider_simple("p-heavy", "Heavy Provider", "https://ph.com", "openai", "api_key", Some("key1"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 9).await.unwrap();
    db.create_provider_simple("p-light", "Light Provider", "https://pl.com", "openai", "api_key", Some("key2"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let mut heavy_count = 0;
    let mut light_count = 0;
    for _ in 0..100 {
        let provider = proxy.select_provider(None).await.unwrap();
        match provider.id.as_str() {
            "p-heavy" => heavy_count += 1,
            "p-light" => light_count += 1,
            _ => {}
        }
    }
    assert!(heavy_count > light_count * 3, "Heavy provider should get significantly more requests (got heavy={}, light={})", heavy_count, light_count);
}

#[tokio::test]
async fn test_proxy_select_provider_skips_inactive() {
    let db = test_db().await;
    db.create_provider_simple("p-active", "Active", "https://a.com", "openai", "api_key", Some("key1"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("p-inactive", "Inactive", "https://i.com", "openai", "api_key", Some("key2"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.deactivate_provider("p-inactive").await.unwrap();

    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    for _ in 0..5 {
        let provider = proxy.select_provider(None).await.unwrap();
        assert_eq!(provider.id, "p-active", "Should only select active providers");
    }
}

#[tokio::test]
async fn test_proxy_extract_model_from_body() {
    let db = test_db().await;
    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let body = r#"{"model":"gpt-4","messages":[{"role":"user","content":"hi"}]}"#;
    let model = proxy.extract_model_from_body(body.as_bytes());
    assert_eq!(model, Some("gpt-4".to_string()));

    let empty_body = b"{}";
    let model = proxy.extract_model_from_body(empty_body);
    assert_eq!(model, None);
}

// ==================== Stats Collector Tests ====================

#[tokio::test]
async fn test_stats_collector_record_usage() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    collector.record_usage(100, 50).await;
    collector.record_usage(200, 100).await;

    let window = collector.current_window.read().await;
    assert_eq!(window.prompt_tokens, 300);
    assert_eq!(window.completion_tokens, 150);
    assert_eq!(window.request_count, 2);
}

#[tokio::test]
async fn test_stats_collector_take_snapshot() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    collector.record_usage(1000, 500).await;

    let rate = collector.take_snapshot(None).await.unwrap();
    assert!(rate > 0.0, "Token rate should be positive");

    let window = collector.current_window.read().await;
    assert_eq!(window.prompt_tokens, 0);
    assert_eq!(window.completion_tokens, 0);
    assert_eq!(window.request_count, 0);
}

#[tokio::test]
async fn test_stats_collector_multiple_snapshots() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    collector.record_usage(100, 50).await;
    let rate1 = collector.take_snapshot(None).await.unwrap();
    assert!(rate1 > 0.0);

    // After snapshot, window is reset
    collector.record_usage(200, 100).await;
    let rate2 = collector.take_snapshot(None).await.unwrap();
    assert!(rate2 > 0.0);

    // Verify both snapshots are in the database
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let snapshots = db.get_token_rate_snapshots(None, &now, 100).await.unwrap();
    assert_eq!(snapshots.len(), 2);
}

// ==================== Auth Manager Tests ====================

#[tokio::test]
async fn test_auth_manager_api_key_provider() {
    let db = test_db().await;
    db.create_provider_simple("p-api", "API Key Provider", "https://api.com", "openai", "api_key", Some("sk-test-key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = AuthManager::new(db.clone());
    let provider = db.get_provider("p-api").await.unwrap().unwrap();

    let (header_name, header_value) = auth_manager.get_auth_header(&provider).await.unwrap();
    assert_eq!(header_name, "Authorization");
    assert_eq!(header_value, "Bearer sk-test-key");
}

#[tokio::test]
async fn test_auth_manager_custom_header_provider() {
    let db = test_db().await;
    db.create_provider_simple("p-custom", "Custom Header Provider", "https://api.com", "openai", "api_key", Some("my-token-123"), None, None, None, "token", "refreshToken", "X-API-Key", "", 86400, 1).await.unwrap();

    let auth_manager = AuthManager::new(db.clone());
    let provider = db.get_provider("p-custom").await.unwrap().unwrap();

    let (header_name, header_value) = auth_manager.get_auth_header(&provider).await.unwrap();
    assert_eq!(header_name, "X-API-Key");
    assert_eq!(header_value, "my-token-123");
}

#[tokio::test]
async fn test_auth_manager_no_api_key_fails() {
    let db = test_db().await;
    db.create_provider_simple("p-nokey", "No Key Provider", "https://api.com", "openai", "api_key", None, None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = AuthManager::new(db.clone());
    let provider = db.get_provider("p-nokey").await.unwrap().unwrap();

    let result = auth_manager.get_auth_header(&provider).await;
    assert!(result.is_err(), "Should fail when no API key configured");
}

#[tokio::test]
async fn test_auth_manager_unknown_auth_type() {
    let db = test_db().await;
    // Manually insert a provider with unknown auth type
    db.create_provider_simple("p-unknown", "Unknown Auth", "https://api.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    
    // Update auth_type directly via SQL
    sqlx::query("UPDATE providers SET auth_type = 'oauth2' WHERE id = 'p-unknown'")
        .execute(&db.pool)
        .await
        .unwrap();

    let auth_manager = AuthManager::new(db.clone());
    let provider = db.get_provider("p-unknown").await.unwrap().unwrap();

    let result = auth_manager.get_auth_header(&provider).await;
    assert!(result.is_err(), "Should fail for unknown auth type");
}

// ==================== Dashboard Summary Tests ====================

#[tokio::test]
async fn test_dashboard_summary() {
    let db = test_db().await;
    db.create_api_key("key-1", "Key 1", "hash-1", "lgk-1", None).await.unwrap();
    db.create_api_key("key-2", "Key 2", "hash-2", "lgk-2", None).await.unwrap();
    db.deactivate_api_key("key-2").await.unwrap();
    db.create_provider_simple("prov-1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let summary = db.get_dashboard_summary().await.unwrap();
    assert_eq!(summary.total_api_keys, 2);
    assert_eq!(summary.active_api_keys, 1);
    assert_eq!(summary.total_providers, 1);
    assert_eq!(summary.active_providers, 1);
}

#[tokio::test]
async fn test_dashboard_summary_empty_database() {
    let db = test_db().await;
    let summary = db.get_dashboard_summary().await.unwrap();
    assert_eq!(summary.total_api_keys, 0);
    assert_eq!(summary.active_api_keys, 0);
    assert_eq!(summary.total_providers, 0);
    assert_eq!(summary.active_providers, 0);
    assert_eq!(summary.total_requests_24h, 0);
    assert_eq!(summary.total_tokens_24h, 0);
}

// ==================== Time-bucketed Stats Tests ====================

#[tokio::test]
async fn test_time_bucketed_stats() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    for _ in 0..3 {
        db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();
    }

    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let end = (now + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();

    let buckets = db.get_time_bucketed_stats(None, None, &start, &end, "day").await.unwrap();
    assert!(!buckets.is_empty(), "Should have at least one time bucket");
    assert_eq!(buckets[0].request_count, 3);
    assert_eq!(buckets[0].total_tokens, 900);
    // avg_duration_ms: 3 requests each with duration_ms=500
    assert_eq!(buckets[0].avg_duration_ms, 500.0);
}

#[tokio::test]
async fn test_time_bucketed_stats_by_provider() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov-1", "Provider 1", "https://p1.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("prov-2", "Provider 2", "https://p2.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov-1", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();
    db.insert_request_log("key", "prov-2", None, "/v1/chat", "POST", None, None, Some(200), None, None, 50, 100, 150, Some(300), false, false, None).await.unwrap();

    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let end = (now + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();

    let buckets = db.get_time_bucketed_stats(None, Some("prov-1"), &start, &end, "day").await.unwrap();
    assert_eq!(buckets[0].request_count, 1);
    assert_eq!(buckets[0].total_tokens, 300);
}

#[tokio::test]
async fn test_time_bucketed_stats_with_throttles() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();
    db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(429), None, None, 0, 0, 0, Some(50), false, true, Some("Rate limited")).await.unwrap();

    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let end = (now + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();

    let buckets = db.get_time_bucketed_stats(None, None, &start, &end, "day").await.unwrap();
    assert_eq!(buckets[0].request_count, 2);
    assert_eq!(buckets[0].throttle_count, 1);
}

// ==================== Streaming Log Tests ====================

#[tokio::test]
async fn test_streaming_request_log() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log(
        "key", "prov", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, None, Some(200), None, None,
        50, 100, 150, Some(2000),
        true, false, None,
    ).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].is_streaming);
}

#[tokio::test]
async fn test_non_streaming_request_log() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log(
        "key", "prov", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, None, Some(200), None, None,
        50, 100, 150, Some(500),
        false, false, None,
    ).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None,  10, 0).await.unwrap();
    assert!(!logs[0].is_streaming);
}

// ==================== Pagination Tests ====================

#[tokio::test]
async fn test_log_pagination() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    for i in 0..15 {
        db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, i, i*2, i*3, Some(100), false, false, None).await.unwrap();
    }

    let page1 = db.query_request_logs(None, None, None, None, None, None,  5, 0).await.unwrap();
    assert_eq!(page1.len(), 5);

    let page2 = db.query_request_logs(None, None, None, None, None, None,  5, 5).await.unwrap();
    assert_eq!(page2.len(), 5);

    let page3 = db.query_request_logs(None, None, None, None, None, None,  5, 10).await.unwrap();
    assert_eq!(page3.len(), 5);

    let total = db.count_request_logs(None, None, None, None, None, None).await.unwrap();
    assert_eq!(total, 15);
}

// ==================== Config Serialization Tests ====================

#[test]
fn test_provider_config_serialization() {
    let config = ProviderConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        base_url: "https://api.com".to_string(),
        api_type: "openai".to_string(),
        auth_type: "api_key".to_string(),
        api_key: Some("sk-test".to_string()),
        token_url: None,
        token_username: None,
        token_password: None,
        token_field: "token".to_string(),
        refresh_token_field: "refreshToken".to_string(),
        token_header_field: "Authorization".to_string(),
        token_header_prefix: "Bearer ".to_string(),
        token_expiry_seconds: 86400,
        weight: 1,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "test");
    assert_eq!(deserialized.auth_type, "api_key");
}

#[test]
fn test_create_api_key_request_deserialization() {
    let json = r#"{"name":"My Key","allowed_providers":["prov-1","prov-2"]}"#;
    let req: CreateApiKeyRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.name, "My Key");
    assert_eq!(req.allowed_providers, Some(vec!["prov-1".to_string(), "prov-2".to_string()]));

    let json_no_providers = r#"{"name":"Open Key"}"#;
    let req: CreateApiKeyRequest = serde_json::from_str(json_no_providers).unwrap();
    assert_eq!(req.name, "Open Key");
    assert_eq!(req.allowed_providers, None);
}

#[test]
fn test_llm_request_deserialization() {
    let json = r#"{"model":"gpt-4","messages":[{"role":"user","content":"Hello"}],"stream":true,"temperature":0.7}"#;
    let req: LlmRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.model, "gpt-4");
    assert_eq!(req.messages.len(), 1);
    assert!(req.stream);
    assert_eq!(req.extra.get("temperature").unwrap().as_f64().unwrap(), 0.7);
}

#[test]
fn test_stats_query_deserialization() {
    let json = r#"{"granularity":"day","provider_id":"prov-1"}"#;
    let query: StatsQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.granularity, Some("day".to_string()));
    assert_eq!(query.provider_id, Some("prov-1".to_string()));
}

#[test]
fn test_request_log_query_deserialization() {
    let json = r#"{"page":2,"page_size":50,"start_time":"2024-01-01T00:00:00Z"}"#;
    let query: RequestLogQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.page, Some(2));
    assert_eq!(query.page_size, Some(50));
}

// ==================== Proxy Log Streaming Tests ====================

#[tokio::test]
async fn test_proxy_log_streaming_request() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let log_id = proxy.log_streaming_request(
        "key", "prov", "/v1/chat/completions",
        Some("gpt-4"), 200,
        50, 100, 150, 2000,
        false, None,
    ).await.unwrap();

    assert!(log_id > 0);

    let logs = db.query_request_logs(None, None, None, None, None, None,  10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].is_streaming);
    assert_eq!(logs[0].prompt_tokens, 50);
    assert_eq!(logs[0].completion_tokens, 100);
    assert_eq!(logs[0].total_tokens, 150);
}

#[tokio::test]
async fn test_proxy_log_streaming_with_throttle() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let _log_id = proxy.log_streaming_request(
        "key", "prov", "/v1/chat/completions",
        Some("gpt-4"), 429,
        0, 0, 0, 50,
        true, Some("Rate limit exceeded"),
    ).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None,  10, 0).await.unwrap();
    assert!(logs[0].is_throttled);
    assert_eq!(logs[0].error_message, Some("Rate limit exceeded".to_string()));
}

// ==================== Multiple API Key Provider Mapping Tests ====================

#[tokio::test]
async fn test_api_key_with_specific_providers() {
    let db = test_db().await;
    let providers = serde_json::to_string(&vec!["openai".to_string()]).unwrap();
    db.create_api_key("key-openai-only", "OpenAI Only", "lgk-hash-openai", "lgk-oi", Some(&providers)).await.unwrap();

    let key = db.get_api_key_by_key("lgk-hash-openai").await.unwrap().unwrap();
    let allowed: Vec<String> = serde_json::from_str(&key.allowed_providers.unwrap()).unwrap();
    assert_eq!(allowed, vec!["openai"]);

    // Verify proxy respects this
    db.create_provider_simple("openai", "OpenAI", "https://api.openai.com/v1", "openai", "api_key", Some("sk-key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("anthropic", "Anthropic", "https://api.anthropic.com/v1", "openai", "api_key", Some("sk-key2"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = LlmProxy::new(db.clone(), auth_manager);

    let allowed_providers = serde_json::from_str::<Vec<String>>(&serde_json::to_string(&vec!["openai".to_string()]).unwrap()).unwrap();
    for _ in 0..5 {
        let provider = proxy.select_provider(Some(&allowed_providers)).await.unwrap();
        assert_eq!(provider.id, "openai", "Should only select OpenAI");
    }
}

#[tokio::test]
async fn test_api_key_with_all_providers() {
    let db = test_db().await;
    db.create_api_key("key-all", "All Providers", "lgk-hash-all", "lgk-all", None).await.unwrap();

    let key = db.get_api_key_by_key("lgk-hash-all").await.unwrap().unwrap();
    assert!(key.allowed_providers.is_none(), "No provider restriction means all providers allowed");
}

// ==================== Per-API-Key Statistics Tests ====================

#[tokio::test]
async fn test_per_api_key_statistics() {
    let db = test_db().await;
    db.create_api_key("user-alice", "Alice", "hash-alice", "lgk-alice", None).await.unwrap();
    db.create_api_key("user-bob", "Bob", "hash-bob", "lgk-bob", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Alice makes 3 requests
    for _ in 0..3 {
        db.insert_request_log("user-alice", "prov", Some("gpt-4"), "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();
    }

    // Bob makes 1 request
    db.insert_request_log("user-bob", "prov", Some("gpt-4"), "/v1/chat", "POST", None, None, Some(200), None, None, 50, 100, 150, Some(300), false, false, None).await.unwrap();

    let alice_stats = db.get_stats(Some("user-alice"), None, None, None).await.unwrap();
    assert_eq!(alice_stats.total_requests, 3);
    assert_eq!(alice_stats.total_tokens, 900);

    let bob_stats = db.get_stats(Some("user-bob"), None, None, None).await.unwrap();
    assert_eq!(bob_stats.total_requests, 1);
    assert_eq!(bob_stats.total_tokens, 150);
}

// ==================== Provider Throttle Statistics Tests ====================

#[tokio::test]
async fn test_provider_throttle_statistics() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov-a", "Provider A", "https://a.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();
    db.create_provider_simple("prov-b", "Provider B", "https://b.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Provider A: 2 throttles out of 5 requests
    for _ in 0..3 {
        db.insert_request_log("key", "prov-a", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();
    }
    for _ in 0..2 {
        db.insert_request_log("key", "prov-a", None, "/v1/chat", "POST", None, None, Some(429), None, None, 0, 0, 0, Some(50), false, true, Some("Rate limited")).await.unwrap();
    }

    // Provider B: 0 throttles out of 2 requests
    for _ in 0..2 {
        db.insert_request_log("key", "prov-b", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();
    }

    let stats_a = db.get_stats(None, Some("prov-a"), None, None).await.unwrap();
    assert_eq!(stats_a.total_requests, 5);
    assert_eq!(stats_a.throttle_count, 2);

    let stats_b = db.get_stats(None, Some("prov-b"), None, None).await.unwrap();
    assert_eq!(stats_b.total_requests, 2);
    assert_eq!(stats_b.throttle_count, 0);
}

// ==================== Granularity Tests ====================

#[tokio::test]
async fn test_time_bucketed_stats_hourly_granularity() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();

    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let end = (now + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();

    let buckets = db.get_time_bucketed_stats(None, None, &start, &end, "5h").await.unwrap();
    assert!(!buckets.is_empty());
}

#[tokio::test]
async fn test_time_bucketed_stats_monthly_granularity() {
    let db = test_db().await;
    db.create_api_key("key", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log("key", "prov", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();

    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::days(365)).format("%Y-%m-%d %H:%M:%S").to_string();
    let end = (now + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();

    let buckets = db.get_time_bucketed_stats(None, None, &start, &end, "month").await.unwrap();
    assert!(!buckets.is_empty());
}

// ==================== JSON Path Extraction Tests ====================

#[test]
fn test_extract_json_path_simple_key() {
    let json = serde_json::json!({"token": "abc123"});
    let val = llm_gateway::auth::extract_json_path(&json, "token");
    assert_eq!(val.unwrap().as_str(), Some("abc123"));
}

#[test]
fn test_extract_json_path_nested_one_level() {
    let json = serde_json::json!({"result": {"token": "abc123"}});
    let val = llm_gateway::auth::extract_json_path(&json, "result.token");
    assert_eq!(val.unwrap().as_str(), Some("abc123"));
}

#[test]
fn test_extract_json_path_nested_two_levels() {
    let json = serde_json::json!({"data": {"result": {"token": "xyz"}}});
    let val = llm_gateway::auth::extract_json_path(&json, "data.result.token");
    assert_eq!(val.unwrap().as_str(), Some("xyz"));
}

#[test]
fn test_extract_json_path_result_newtoken() {
    // The exact format the user reported
    let json = serde_json::json!({
        "result": {"newToken": "xxx", "token": "yyy"},
        "status": "ok"
    });
    let val = llm_gateway::auth::extract_json_path(&json, "result.newToken");
    assert_eq!(val.unwrap().as_str(), Some("xxx"));
    let val2 = llm_gateway::auth::extract_json_path(&json, "result.token");
    assert_eq!(val2.unwrap().as_str(), Some("yyy"));
}

#[test]
fn test_extract_json_path_missing_key() {
    let json = serde_json::json!({"result": {"token": "abc"}});
    let val = llm_gateway::auth::extract_json_path(&json, "result.newToken");
    assert!(val.is_none());
}

#[test]
fn test_extract_json_path_missing_parent() {
    let json = serde_json::json!({"status": "ok"});
    let val = llm_gateway::auth::extract_json_path(&json, "result.token");
    assert!(val.is_none());
}

#[test]
fn test_extract_json_path_empty_string() {
    let json = serde_json::json!({"token": ""});
    let val = llm_gateway::auth::extract_json_path(&json, "token");
    assert_eq!(val.unwrap().as_str(), Some(""));
}

#[test]
fn test_extract_json_path_number_value() {
    let json = serde_json::json!({"result": {"expires": 3600}});
    let val = llm_gateway::auth::extract_json_path(&json, "result.expires");
    assert_eq!(val.unwrap().as_i64(), Some(3600));
}

#[test]
fn test_extract_json_path_status_field() {
    let json = serde_json::json!({
        "result": {"newToken": "abc", "token": "def"},
        "status": "ok"
    });
    let val = llm_gateway::auth::extract_json_path(&json, "status");
    assert_eq!(val.unwrap().as_str(), Some("ok"));
}

#[test]
fn test_extract_json_path_array_index_not_supported() {
    // Arrays are not supported - should return None for numeric keys
    let json = serde_json::json!({"tokens": ["a", "b"]});
    let val = llm_gateway::auth::extract_json_path(&json, "tokens.0");
    assert!(val.is_none()); // "0" is not a valid object key
}

#[test]
fn test_extract_json_path_deeply_nested() {
    let json = serde_json::json!({
        "response": {"auth": {"credentials": {"access_token": "deep-token"}}}
    });
    let val = llm_gateway::auth::extract_json_path(&json, "response.auth.credentials.access_token");
    assert_eq!(val.unwrap().as_str(), Some("deep-token"));
}

// ==================== Token Extra Headers Tests ====================

#[test]

#[test]
fn test_provider_extra_headers_stored() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let db = llm_gateway::db::Database::new(":memory:").await.unwrap();
        db.create_provider(
            "prov-eh", "ExtraHeaders", "https://p.com", "openai", "dynamic",
            None, Some("https://auth.com/login"), Some("user"), Some("pass"),
            Some("POST"), Some("json"), Some("username"), Some("password"),
            None, Some(r#"{"X-App-Id":"myapp","X-Api-Version":"v2"}"#), None,
            "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false, "choices.0.message.content", "choices.0.delta.reasoning_content",
        ).await.unwrap();

        let p = db.get_provider("prov-eh").await.unwrap().unwrap();
        assert_eq!(p.token_extra_headers.as_deref(), Some(r#"{"X-App-Id":"myapp","X-Api-Version":"v2"}"#));
    });
    std::fs::remove_file(":memory:").ok();
}

#[test]
fn test_provider_extra_headers_none_by_default() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let db = llm_gateway::db::Database::new(":memory:").await.unwrap();
        db.create_provider_simple(
            "prov-noeh", "NoExtraHeaders", "https://p.com", "openai", "api_key",
            Some("key"), None, None, None,
            "token", "refreshToken", "Authorization", "Bearer ", 86400, 1,
        ).await.unwrap();

        let p = db.get_provider("prov-noeh").await.unwrap().unwrap();
        assert!(p.token_extra_headers.is_none());
    });
    std::fs::remove_file(":memory:").ok();
}

#[test]
fn test_provider_update_extra_headers() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let db = llm_gateway::db::Database::new(":memory:").await.unwrap();
        db.create_provider(
            "prov-eh2", "ExtraHeaders2", "https://p.com", "openai", "dynamic",
            None, Some("https://auth.com/login"), Some("user"), Some("pass"),
            Some("POST"), Some("json"), Some("username"), Some("password"),
            None, Some(r#"{"X-Custom":"val1"}"#), None,
            "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false, "choices.0.message.content", "choices.0.delta.reasoning_content",
        ).await.unwrap();

        // Update via delete + recreate with new headers
        db.delete_provider("prov-eh2").await.unwrap();
        db.create_provider(
            "prov-eh2", "ExtraHeaders2", "https://p.com", "openai", "dynamic",
            None, Some("https://auth.com/login"), Some("user"), Some("pass"),
            Some("POST"), Some("json"), Some("username"), Some("password"),
            None, Some(r#"{"X-Custom":"val2","X-New":"header"}"#), None,
            "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false, "choices.0.message.content", "choices.0.delta.reasoning_content",
        ).await.unwrap();

        let p = db.get_provider("prov-eh2").await.unwrap().unwrap();
        let headers: serde_json::Map<String, serde_json::Value> = serde_json::from_str(p.token_extra_headers.as_deref().unwrap()).unwrap();
    });
    std::fs::remove_file(":memory:").ok();
    std::fs::remove_file(":memory:").ok();
}

#[test]
fn test_extra_headers_json_parsing() {
    // Verify that the JSON format used for extra headers is valid
    let headers_json = r#"{"X-App-Id":"myapp","X-Api-Version":"v2","Authorization":"Basic abc123"}"#;
    let parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(headers_json).unwrap();
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed["X-App-Id"].as_str(), Some("myapp"));
    assert_eq!(parsed["X-Api-Version"].as_str(), Some("v2"));
    assert_eq!(parsed["Authorization"].as_str(), Some("Basic abc123"));
}

#[test]
fn test_extra_headers_empty_object() {
    let headers_json = "{}";
    let parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(headers_json).unwrap();
    assert_eq!(parsed.len(), 0);
}


#[tokio::test]
async fn test_provider_reasoning_path_field() {
    let db = test_db().await;
    db.create_provider(
        "prov-reason", "Reasoning Provider", "https://r.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();

    let provider = db.get_provider("prov-reason").await.unwrap().unwrap();
    assert_eq!(provider.response_content_path, "choices.0.message.content");
    assert_eq!(provider.response_reasoning_path, "choices.0.delta.reasoning_content");

    // Update only the reasoning path
    db.update_provider(
        "prov-reason", "prov-reason", "Updated Provider", "https://r.com", "openai", "api_key",
        Some("key2"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, true, false,
        "custom.content.path", "custom.reasoning.path",
    ).await.unwrap();

    let updated = db.get_provider("prov-reason").await.unwrap().unwrap();
    assert_eq!(updated.response_content_path, "custom.content.path");
    assert_eq!(updated.response_reasoning_path, "custom.reasoning.path");
}

#[tokio::test]
async fn test_provider_model_list_add_remove() {
    let db = test_db().await;
    db.create_provider(
        "prov-models", "Model Test Provider", "https://api.example.com/v1", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();

    // List models (should be empty)
    let models = db.list_provider_models("prov-models").await.unwrap();
    assert_eq!(models.len(), 0);

    // Add a model
    db.add_provider_model("prov-models", "gpt-4o").await.unwrap();
    let models = db.list_provider_models("prov-models").await.unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model_id, "gpt-4o");

    // Add another model
    db.add_provider_model("prov-models", "claude-3-opus").await.unwrap();
    let models = db.list_provider_models("prov-models").await.unwrap();
    assert_eq!(models.len(), 2);

    // Remove a model
    db.remove_provider_model("prov-models", "gpt-4o").await.unwrap();
    let models = db.list_provider_models("prov-models").await.unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model_id, "claude-3-opus");
}

#[tokio::test]
async fn test_stats_by_api_key_grouping() {
    let db = test_db().await;
    // Create provider and API key
    db.create_provider(
        "stat-prov-xyz", "Stat Provider XYZ", "https://s.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("stat-key-xyz-1", "Stat Key XYZ 1", "sk-xyz-test1", "sk-xyz1", None).await.unwrap();
    db.create_api_key("stat-key-xyz-2", "Stat Key XYZ 2", "sk-xyz-test2", "sk-xyz2", None).await.unwrap();

    // Insert a request log for key 1
    db.insert_request_log(
        "stat-key-xyz-1", "stat-prov-xyz", Some("gpt-4o"), "/v1/chat/completions", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        100, 200, 300, Some(150), false, false, None,
    ).await.unwrap();
    db.insert_request_log(
        "stat-key-xyz-1", "stat-prov-xyz", Some("gpt-4o"), "/v1/chat/completions", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        150, 300, 450, Some(200), false, false, None,
    ).await.unwrap();
    db.insert_request_log(
        "stat-key-xyz-2", "stat-prov-xyz", Some("claude-3"), "/v1/chat/completions", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        80, 160, 240, Some(120), false, false, None,
    ).await.unwrap();

    // Get stats by API key
    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 2);

    // stat-key-xyz-1 should have more tokens (750 vs 240)
    let key1_stats = stats.iter().find(|s| s.api_key_id == "stat-key-xyz-1").unwrap();
    assert_eq!(key1_stats.request_count, 2);
    assert_eq!(key1_stats.total_tokens, 750); // 300 + 450

    let key2_stats = stats.iter().find(|s| s.api_key_id == "stat-key-xyz-2").unwrap();
    assert_eq!(key2_stats.request_count, 1);
    assert_eq!(key2_stats.total_tokens, 240);

    // Test with limit
    let limited = db.get_stats_by_api_key(1, None, None).await.unwrap();
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].api_key_id, "stat-key-xyz-1"); // Most tokens first
}



#[tokio::test]
async fn test_time_bucketed_stats_avg_duration() {
    let db = test_db().await;
    db.create_api_key("key-dur", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov-dur", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Insert requests with different durations
    db.insert_request_log("key-dur", "prov-dur", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 100, 200, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key-dur", "prov-dur", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 100, 200, Some(300), false, false, None).await.unwrap();
    db.insert_request_log("key-dur", "prov-dur", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 100, 200, Some(500), false, false, None).await.unwrap();

    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    let end = (now + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();

    let buckets = db.get_time_bucketed_stats(None, None, &start, &end, "day").await.unwrap();
    assert!(!buckets.is_empty());
    // (100 + 300 + 500) / 3 = 300
    assert_eq!(buckets[0].avg_duration_ms, 300.0);
}


#[tokio::test]
async fn test_provider_health_stats_returns_data() {
    let db = test_db().await;
    db.create_api_key("key-h", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov-h", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Insert some request logs
    db.insert_request_log("key-h", "prov-h", None, "/v1/chat", "POST", None, None, Some(200), None, None, 100, 200, 300, Some(500), false, false, None).await.unwrap();
    db.insert_request_log("key-h", "prov-h", None, "/v1/chat", "POST", None, None, Some(500), None, None, 50, 100, 150, Some(300), false, false, Some("Some error")).await.unwrap();

    let stats = db.get_provider_health_stats().await.unwrap();
    // Should have health stats for prov-h
    let prov_stats = stats.iter().find(|s| s.provider_id == "prov-h");
    assert!(prov_stats.is_some(), "Should have stats for prov-h");
    let h = prov_stats.unwrap();
    assert_eq!(h.request_count_24h, 2);
    assert_eq!(h.error_count_24h, 1);
    assert_eq!(h.throttle_count_24h, 0);
    assert!((h.avg_duration_ms - 400.0).abs() < 0.1, "avg should be 400");
    assert!(h.last_request_at.is_some());
}


#[tokio::test]
async fn test_request_logs_search() {
    let db = test_db().await;
    db.create_api_key("key-s", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov-s", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Insert logs with different request bodies
    db.insert_request_log("key-s", "prov-s", None, "/v1/chat", "POST", None, Some(r#"{"messages":[{"role":"user","content":"Hello AI"}]}"#), Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key-s", "prov-s", None, "/v1/chat", "POST", None, Some(r#"{"messages":[{"role":"user","content":"Tell me a joke"}]}"#), Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key-s", "prov-s", None, "/v1/chat", "POST", None, Some(r#"{"messages":[{"role":"user","content":"What is Rust?"}]}"#), Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();

    // Search for "joke" - should return 1
    let logs = db.query_request_logs(None, None, None, None, Some("joke"), None,  20, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].request_body.as_ref().unwrap().contains("joke"));

    // Search for "What" - should return 1 (case sensitive in LIKE, but SQLite LIKE is case-insensitive by default for ASCII)
    let logs2 = db.query_request_logs(None, None, None, None, Some("What"), None,  20, 0).await.unwrap();
    assert_eq!(logs2.len(), 1);

    // Search for "messages" - should return all 3
    let logs3 = db.query_request_logs(None, None, None, None, Some("messages"), None,  20, 0).await.unwrap();
    assert_eq!(logs3.len(), 3);

    // No search - should return all 3
    let all = db.query_request_logs(None, None, None, None, None, None, 20, 0).await.unwrap();
    assert_eq!(all.len(), 3);

    // Count with search
    let count = db.count_request_logs(None, None, None, None, Some("Hello"), None).await.unwrap();
    assert_eq!(count, 1);
}


#[tokio::test]
async fn test_request_logs_search_response_body() {
    let db = test_db().await;
    db.create_api_key("key-rs", "Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("prov-rs", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Insert logs with different response bodies
    db.insert_request_log("key-rs", "prov-rs", None, "/v1/chat", "POST", None, Some(r#"{"messages":[{"role":"user","content":"Hello"}]}"#), Some(200), None, Some(r#"{"choices":[{"message":{"content":"Hello there!"}}]}"#), 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key-rs", "prov-rs", None, "/v1/chat", "POST", None, Some(r#"{"messages":[{"role":"user","content":"Hi"}]}"#), Some(200), None, Some(r#"{"choices":[{"message":{"content":"Greetings!"}}]}"#), 10, 20, 30, Some(100), false, false, None).await.unwrap();

    // Search in response body - should find 1
    let logs = db.query_request_logs(None, None, None, None, Some("Greetings"), None,  20, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert!(logs[0].response_body.as_ref().unwrap().contains("Greetings"));

    // Search in request body - should find 1
    let logs2 = db.query_request_logs(None, None, None, None, Some("Hello"), None,  20, 0).await.unwrap();
    assert_eq!(logs2.len(), 1);
    assert!(logs2[0].request_body.as_ref().unwrap().contains("Hello"));
}


#[tokio::test]
async fn test_model_multiple_mappings_same_provider() {
    let db = test_db().await;
    db.create_provider_simple("prov-multi", "Provider", "https://p.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Add two provider models (just the model_id, no name/description in this API)
    db.add_provider_model("prov-multi", "model-a").await.unwrap();
    db.add_provider_model("prov-multi", "model-b").await.unwrap();

    // Create two unified models, each mapping to different provider models
    db.create_model("unified-a", "Unified A", None, "unified", 50, None).await.unwrap();
    db.add_model_mapping("unified-a", "prov-multi", "model-a", 50, 1.0).await.unwrap();

    db.create_model("unified-b", "Unified B", None, "unified", 50, None).await.unwrap();
    db.add_model_mapping("unified-b", "prov-multi", "model-b", 50, 1.0).await.unwrap();

    // Verify each unified model has exactly one mapping
    let mappings_a = db.list_model_mappings("unified-a").await.unwrap();
    assert_eq!(mappings_a.len(), 1);
    assert_eq!(mappings_a[0].provider_model_id, "model-a");

    let mappings_b = db.list_model_mappings("unified-b").await.unwrap();
    assert_eq!(mappings_b.len(), 1);
    assert_eq!(mappings_b[0].provider_model_id, "model-b");
}


#[tokio::test]
async fn test_stats_by_api_key_with_time_range() {
    let db = test_db().await;
    db.create_provider(
        "time-prov-abc", "Time Provider ABC", "https://t.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("time-key-abc", "Time Key ABC", "sk-time-abc", "sk-tab", None).await.unwrap();

    db.insert_request_log(
        "time-key-abc", "time-prov-abc", Some("gpt-4o"), "/v1/chat/completions", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        100, 200, 300, Some(150), false, false, None,
    ).await.unwrap();

    // No time filter - should find the log
    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert!(!stats.is_empty());

    // Future time filter - should find nothing
    let future = "2099-01-01 00:00:00";
    let empty_stats = db.get_stats_by_api_key(10, Some(future), None).await.unwrap();
    assert!(empty_stats.is_empty());
}


#[tokio::test]
async fn test_query_logs_by_model() {
    let db = test_db().await;
    db.create_api_key("model-key", "Model Key", "hash", "lgk", None).await.unwrap();
    db.create_provider_simple("model-prov", "Model Provider", "https://m.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Insert logs for different models
    db.insert_request_log("model-key", "model-prov", Some("gpt-4o"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("model-key", "model-prov", Some("gpt-4o"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("model-key", "model-prov", Some("claude-3"), "/v1/chat", "POST", None, None, Some(200), None, None, 5, 10, 15, Some(80), false, false, None).await.unwrap();

    // Filter by gpt-4o
    let gpt_logs = db.query_request_logs(None, None, None, None, None, Some("gpt-4o"), 10, 0).await.unwrap();
    assert_eq!(gpt_logs.len(), 2);

    // Filter by claude-3
    let claude_logs = db.query_request_logs(None, None, None, None, None, Some("claude-3"), 10, 0).await.unwrap();
    assert_eq!(claude_logs.len(), 1);

    // Count by model
    let gpt_count = db.count_request_logs(None, None, None, None, None, Some("gpt-4o")).await.unwrap();
    assert_eq!(gpt_count, 2);
}


#[tokio::test]
async fn test_stats_by_api_key_time_range_filter() {
    let db = test_db().await;
    db.create_provider(
        "time-prov-2", "Time Provider 2", "https://t2.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("time-key-2", "Time Key 2", "sk-time-2", "sk-t2", None).await.unwrap();

    db.insert_request_log(
        "time-key-2", "time-prov-2", Some("gpt-4o"), "/v1/chat/completions", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        100, 200, 300, Some(150), false, false, None,
    ).await.unwrap();

    // With no time filter, should find 1
    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 1);

    // With far future start, should find nothing
    let future = "2099-12-31 23:59:59";
    let empty = db.get_stats_by_api_key(10, Some(future), None).await.unwrap();
    assert_eq!(empty.len(), 0);
}


#[tokio::test]
async fn test_stats_by_api_key_token_breakdown() {
    let db = test_db().await;
    db.create_provider(
        "breakdown-prov", "Breakdown Provider", "https://b.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("breakdown-key", "Breakdown Key", "sk-break", "sk-bk", None).await.unwrap();

    // Insert with specific token counts
    db.insert_request_log(
        "breakdown-key", "breakdown-prov", Some("gpt-4o"), "/v1/chat/completions", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        100, 50, 150, Some(100), false, false, None,
    ).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].total_prompt_tokens, 100);
    assert_eq!(stats[0].total_completion_tokens, 50);
    assert_eq!(stats[0].total_tokens, 150);
}


#[tokio::test]
async fn test_stats_by_api_key_empty_db() {
    let db = test_db().await;
    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert!(stats.is_empty());
}


#[tokio::test]
async fn test_list_active_provider_models() {
    let db = test_db().await;
    db.create_provider(
        "model-prov-2", "Model Provider 2", "https://mp2.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();

    db.add_provider_model("model-prov-2", "gpt-4o-mini").await.unwrap();
    
    let active = db.list_active_provider_models("model-prov-2").await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].model_id, "gpt-4o-mini");
    
    // Non-existent provider
    let empty = db.list_active_provider_models("non-existent").await.unwrap();
    assert!(empty.is_empty());
}


#[tokio::test]
async fn test_stats_by_api_key_multiple_keys() {
    let db = test_db().await;
    db.create_provider(
        "multi-prov", "Multi Provider", "https://m.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    
    db.create_api_key("multi-key-1", "Multi Key 1", "sk-multi1", "sk-m1", None).await.unwrap();
    db.create_api_key("multi-key-2", "Multi Key 2", "sk-multi2", "sk-m2", None).await.unwrap();

    db.insert_request_log(
        "multi-key-1", "multi-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        50, 100, 150, Some(100), false, false, None,
    ).await.unwrap();
    db.insert_request_log(
        "multi-key-2", "multi-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        200, 400, 600, Some(200), false, false, None,
    ).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 2);
    
    // Verify ordering by total tokens (key2 has more)
    assert!(stats[0].total_tokens >= stats[1].total_tokens);
}


#[tokio::test]
async fn test_stats_by_api_key_calculated_correctly() {
    let db = test_db().await;
    db.create_provider(
        "calc-prov", "Calc Provider", "https://c.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("calc-key", "Calc Key", "sk-calc", "sk-c", None).await.unwrap();

    // Single log
    db.insert_request_log(
        "calc-key", "calc-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(50), false, false, None,
    ).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].request_count, 1);
    assert_eq!(stats[0].total_prompt_tokens, 10);
    assert_eq!(stats[0].total_completion_tokens, 20);
    assert_eq!(stats[0].total_tokens, 30);
}


#[tokio::test]
async fn test_provider_health_stats_with_requests() {
    let db = test_db().await;
    db.create_provider(
        "health-prov", "Health Provider", "https://h.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("health-key", "Health Key", "sk-health", "sk-h", None).await.unwrap();

    db.insert_request_log(
        "health-key", "health-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(100), false, false, None,
    ).await.unwrap();

    let health = db.get_provider_health_stats().await.unwrap();
    assert!(!health.is_empty());
    let h = health.iter().find(|p| p.provider_id == "health-prov").unwrap();
    assert_eq!(h.request_count_24h, 1);
    assert_eq!(h.avg_duration_ms, 100.0);
}


#[tokio::test]
async fn test_stats_by_api_key_ordering_desc() {
    let db = test_db().await;
    db.create_provider(
        "order-prov", "Order Provider", "https://o.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("order-key-1", "Order Key 1", "sk-order1", "sk-o1", None).await.unwrap();
    db.create_api_key("order-key-2", "Order Key 2", "sk-order2", "sk-o2", None).await.unwrap();

    // Key1: 100 tokens, Key2: 500 tokens
    db.insert_request_log(
        "order-key-1", "order-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        50, 50, 100, Some(100), false, false, None,
    ).await.unwrap();
    db.insert_request_log(
        "order-key-2", "order-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        250, 250, 500, Some(100), false, false, None,
    ).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 2);
    // First should have more tokens (descending order)
    assert!(stats[0].total_tokens > stats[1].total_tokens);
}


#[tokio::test]
async fn test_stats_by_api_key_includes_error_count() {
    let db = test_db().await;
    db.create_provider(
        "err-prov", "Error Provider", "https://e.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("err-key", "Error Key", "sk-err", "sk-e", None).await.unwrap();

    // One success, one error
    db.insert_request_log(
        "err-key", "err-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(100), false, false, None,
    ).await.unwrap();
    db.insert_request_log(
        "err-key", "err-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(500), None, None,
        0, 0, 0, Some(50), false, false, Some("Internal error"),
    ).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].request_count, 2);
    assert_eq!(stats[0].error_count, 1);
}


#[tokio::test]
async fn test_stats_by_api_key_throttle_count() {
    let db = test_db().await;
    db.create_provider(
        "throttle-prov", "Throttle Provider", "https://t.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("throttle-key", "Throttle Key", "sk-throttle", "sk-t", None).await.unwrap();

    // One normal, one throttled
    db.insert_request_log(
        "throttle-key", "throttle-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(100), false, false, None,
    ).await.unwrap();
    db.insert_request_log(
        "throttle-key", "throttle-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(429), None, Some("{}"),
        0, 0, 0, Some(10), false, true, None,
    ).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].throttle_count, 1);
    assert_eq!(stats[0].error_count, 0); // 429 is throttle, not error
}


#[tokio::test]
async fn test_logs_model_filter_integration() {
    let db = test_db().await;
    db.create_api_key("model-filter-key", "Model Filter Key", "hash", "mfk", None).await.unwrap();
    db.create_provider_simple("model-filter-prov", "Model Filter Provider", "https://mfp.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    db.insert_request_log(
        "model-filter-key", "model-filter-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(100), false, false, None,
    ).await.unwrap();
    db.insert_request_log(
        "model-filter-key", "model-filter-prov", Some("claude-3"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        15, 25, 40, Some(100), false, false, None,
    ).await.unwrap();

    // Filter by gpt-4o
    let logs = db.query_request_logs(None, None, None, None, None, Some("gpt-4o"), 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].model.as_deref(), Some("gpt-4o"));
}


#[tokio::test]
async fn test_stats_by_api_key_avg_duration() {
    let db = test_db().await;
    db.create_provider(
        "dur-prov", "Duration Provider", "https://d.com", "openai", "api_key",
        Some("key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("dur-key", "Dur Key", "sk-dur", "sk-d", None).await.unwrap();

    // Two logs: 100ms and 200ms -> avg should be 150ms
    db.insert_request_log(
        "dur-key", "dur-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(100), false, false, None,
    ).await.unwrap();
    db.insert_request_log(
        "dur-key", "dur-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(200), false, false, None,
    ).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 1);
    // Average of 100 and 200 is 150
    assert!((stats[0].avg_duration_ms - 150.0).abs() < 0.01);
}


#[tokio::test]
async fn test_logs_with_error_status_filtering() {
    let db = test_db().await;
    db.create_api_key("err-status-key", "Err Status Key", "hash", "esk", None).await.unwrap();
    db.create_provider_simple("err-status-prov", "Err Status Provider", "https://esp.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Success
    db.insert_request_log(
        "err-status-key", "err-status-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(100), false, false, None,
    ).await.unwrap();
    // Error
    db.insert_request_log(
        "err-status-key", "err-status-prov", Some("gpt-4o"), "/v1/chat", "POST",
        None, Some("{}"), Some(500), None, Some("{}"),
        0, 0, 0, Some(50), false, false, Some("Server error"),
    ).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 2);
    let error_log = logs.iter().find(|l| l.response_status == Some(500)).unwrap();
    assert!(error_log.error_message.is_some());
}


#[tokio::test]
async fn test_logs_model_filter_none_model() {
    let db = test_db().await;
    db.create_api_key("nonemodel-key", "No Model Key", "hash", "nmk", None).await.unwrap();
    db.create_provider_simple("nonemodel-prov", "No Model Provider", "https://nmp.com", "openai", "api_key", Some("key"), None, None, None, "token", "refreshToken", "Authorization", "Bearer ", 86400, 1).await.unwrap();

    // Log with no model
    db.insert_request_log(
        "nonemodel-key", "nonemodel-prov", None, "/v1/chat", "POST",
        None, Some("{}"), Some(200), None, Some("{}"),
        10, 20, 30, Some(100), false, false, None,
    ).await.unwrap();

    let logs = db.query_request_logs(None, None, None, None, None, None, 10, 0).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].model, None);
}
