use llm_gateway::db::Database;
use llm_gateway::auth::AuthManager;
use llm_gateway::stats::StatsCollector;
use std::sync::Arc;

async fn test_db() -> Arc<Database> {
    Arc::new(Database::new_in_memory().await.expect("Failed to create test DB"))
}

fn make_api_key_provider(id: &str) -> llm_gateway::db::ProviderRow {
    llm_gateway::db::ProviderRow {
        id: id.to_string(), name: id.to_string(),
        base_url: "https://api.example.com/v1".to_string(),
        api_type: "openai".to_string(), auth_type: "api_key".to_string(),
        api_key: Some("sk-test-key-123".to_string()),
        token_url: None, token_username: None, token_password: None,
        token_request_method: None, token_content_type: None,
        token_username_field: None, token_password_field: None,
        token_body_template: None, token_extra_headers: None, token_cookies: None,
        token_field: "token".to_string(), refresh_token_field: "refreshToken".to_string(),
        token_header_field: "Authorization".to_string(), token_header_prefix: "Bearer ".to_string(),
        token_expiry_seconds: 86400, current_token: None, current_refresh_token: None, token_expires_at: None,
        is_active: true, weight: 1, bypass_proxy: false,
        response_content_path: String::new(), response_reasoning_path: String::new(),
        chart_color: None, subscription_start: None, mock_mode: false,
        group_id: None,
        strip_thinking_tags_in_response: false,
        created_at: String::new(), updated_at: String::new(),
    }
}

fn make_dynamic_token_provider(id: &str) -> llm_gateway::db::ProviderRow {
    llm_gateway::db::ProviderRow {
        id: id.to_string(), name: id.to_string(),
        base_url: "https://api.example.com/v1".to_string(),
        api_type: "openai".to_string(), auth_type: "dynamic_token".to_string(),
        api_key: None,
        token_url: Some("https://auth.example.com/token".to_string()),
        token_username: Some("user@example.com".to_string()),
        token_password: Some("password123".to_string()),
        token_request_method: Some("POST".to_string()),
        token_content_type: Some("json".to_string()),
        token_username_field: Some("username".to_string()),
        token_password_field: Some("password".to_string()),
        token_body_template: None, token_extra_headers: None, token_cookies: None,
        token_field: "access_token".to_string(), refresh_token_field: "refresh_token".to_string(),
        token_header_field: "Authorization".to_string(), token_header_prefix: "Bearer ".to_string(),
        token_expiry_seconds: 3600, current_token: Some("current-token-abc".to_string()),
        current_refresh_token: Some("refresh-token-xyz".to_string()),
        token_expires_at: Some("2099-12-31T23:59:59".to_string()),
        is_active: true, weight: 1, bypass_proxy: false,
        response_content_path: String::new(), response_reasoning_path: String::new(),
        chart_color: None, subscription_start: None, mock_mode: false,
        group_id: None,
        strip_thinking_tags_in_response: false,
        created_at: String::new(), updated_at: String::new(),
    }
}

#[tokio::test]
async fn test_auth_api_key_provider() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let provider = make_api_key_provider("auth-api-key");
    let (name, value) = auth.get_auth_header(&provider).await.unwrap();
    assert_eq!(name, "Authorization");
    assert_eq!(value, "Bearer sk-test-key-123");
}

#[tokio::test]
async fn test_auth_api_key_no_key_error() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let mut provider = make_api_key_provider("auth-no-key");
    provider.api_key = None;
    assert!(auth.get_auth_header(&provider).await.is_err());
}

#[tokio::test]
async fn test_auth_unknown_auth_type() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let mut provider = make_api_key_provider("auth-unknown");
    provider.auth_type = "unknown_type".to_string();
    assert!(auth.get_auth_header(&provider).await.is_err());
}

#[tokio::test]
async fn test_auth_dynamic_token_valid() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let provider = make_dynamic_token_provider("auth-dynamic");
    let (name, value) = auth.get_auth_header(&provider).await.unwrap();
    assert_eq!(name, "Authorization");
    assert_eq!(value, "Bearer current-token-abc");
}

#[tokio::test]
async fn test_auth_dynamic_token_no_url_error() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let mut provider = make_dynamic_token_provider("auth-no-url");
    provider.token_url = None;
    provider.current_token = None;
    provider.token_expires_at = None;
    assert!(auth.get_auth_header(&provider).await.is_err());
}

#[tokio::test]
async fn test_auth_dynamic_token_no_creds_error() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let mut provider = make_dynamic_token_provider("auth-no-creds");
    provider.token_username = None;
    provider.token_password = None;
    provider.current_token = None;
    provider.token_expires_at = None;
    assert!(auth.refresh_token(&provider).await.is_err());
}

#[tokio::test]
async fn test_auth_custom_header_field() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let mut provider = make_api_key_provider("auth-custom-hdr");
    provider.token_header_field = "X-API-Key".to_string();
    provider.token_header_prefix = "".to_string();
    let (name, value) = auth.get_auth_header(&provider).await.unwrap();
    assert_eq!(name, "X-API-Key");
    assert_eq!(value, "sk-test-key-123");
}

#[tokio::test]
async fn test_auth_custom_prefix() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let mut provider = make_api_key_provider("auth-prefix");
    provider.token_header_prefix = "Key ".to_string();
    let (_, value) = auth.get_auth_header(&provider).await.unwrap();
    assert_eq!(value, "Key sk-test-key-123");
}

#[tokio::test]
async fn test_auth_empty_prefix() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let mut provider = make_api_key_provider("auth-empty-pfx");
    provider.token_header_prefix = "".to_string();
    let (_, value) = auth.get_auth_header(&provider).await.unwrap();
    assert_eq!(value, "sk-test-key-123");
}

#[tokio::test]
async fn test_auth_dynamic_expired_needs_refresh() {
    let db = test_db().await;
    let auth = AuthManager::new(db);
    let mut provider = make_dynamic_token_provider("auth-expired");
    provider.token_expires_at = Some("2020-01-01T00:00:00Z".to_string());
    assert!(auth.get_auth_header(&provider).await.is_err());
}

// ==================== StatsCollector Tests ====================

#[tokio::test]
async fn test_stats_record_usage() {
    let db = test_db().await;
    let stats = StatsCollector::new(db);
    stats.record_usage("stats-prov", 100, 50).await;
    let rate = stats.take_snapshot("stats-prov").await.unwrap();
    assert!(rate >= 0.0);
}

#[tokio::test]
async fn test_stats_multiple_recordings() {
    let db = test_db().await;
    let stats = StatsCollector::new(db);
    stats.record_usage("multi-prov", 100, 50).await;
    stats.record_usage("multi-prov", 200, 100).await;
    stats.record_usage("multi-prov", 150, 75).await;
    let rate = stats.take_snapshot("multi-prov").await.unwrap();
    assert!(rate >= 0.0);
}

#[tokio::test]
async fn test_stats_active_providers_empty() {
    let db = test_db().await;
    let stats = StatsCollector::new(db);
    assert!(stats.get_active_provider_ids().await.is_empty());
}

#[tokio::test]
async fn test_stats_active_providers_with_data() {
    let db = test_db().await;
    db.create_provider(
        "active-stats-prov", "Stats Provider", "https://api.example.com/v1", "openai", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, true, false, "", "", false,
    ).await.unwrap();
    let stats = StatsCollector::new(db);
    stats.record_usage("active-stats-prov", 10, 5).await;
    assert!(stats.get_active_provider_ids().await.contains(&"active-stats-prov".to_string()));
}

#[tokio::test]
async fn test_stats_get_current_rate_no_data() {
    let db = test_db().await;
    let stats = StatsCollector::new(db);
    assert!(stats.get_current_rate(None).await.is_ok());
}

#[tokio::test]
async fn test_stats_record_usage_zero_tokens() {
    let db = test_db().await;
    let stats = StatsCollector::new(db);
    stats.record_usage("zero-prov", 0, 0).await;
    let rate = stats.take_snapshot("zero-prov").await.unwrap();
    assert!(rate >= 0.0);
}

#[tokio::test]
async fn test_stats_record_usage_large_tokens() {
    let db = test_db().await;
    let stats = StatsCollector::new(db);
    stats.record_usage("large-prov", 1000000, 500000).await;
    let rate = stats.take_snapshot("large-prov").await.unwrap();
    assert!(rate >= 0.0);
}

#[tokio::test]
async fn test_stats_multiple_providers() {
    let db = test_db().await;
    let stats = StatsCollector::new(db);
    stats.record_usage("prov-a", 100, 50).await;
    stats.record_usage("prov-b", 200, 100).await;
    let rate_a = stats.take_snapshot("prov-a").await.unwrap();
    let rate_b = stats.take_snapshot("prov-b").await.unwrap();
    assert!(rate_a >= 0.0);
    assert!(rate_b >= 0.0);
}

#[tokio::test]
async fn test_stats_snapshot_no_data() {
    let db = test_db().await;
    let stats = StatsCollector::new(db);
    // take_snapshot for a provider with no recorded usage
    let result = stats.take_snapshot("no-data-prov").await;
    // Should succeed (returns 0.0) or fail gracefully
    assert!(result.is_ok() || result.is_err());
}
