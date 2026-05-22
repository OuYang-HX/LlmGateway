use llm_gateway::db::Database;
use llm_gateway::auth::AuthManager;
use llm_gateway::proxy::LlmProxy;
use llm_gateway::stats::StatsCollector;
use std::sync::Arc;

async fn test_db() -> Arc<Database> {
    Arc::new(Database::new_in_memory().await.expect("Failed to create test DB"))
}

fn make_proxy(db: Arc<Database>) -> LlmProxy {
    let auth = Arc::new(AuthManager::new(db.clone()));
    let stats = Arc::new(StatsCollector::new(db.clone()));
    LlmProxy::new(db, auth, stats)
}

async fn create_test_provider(db: &Database, id: &str, weight: i64, active: bool) {
    db.create_provider(
        id, id, "https://api.example.com/v1", "openai", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, weight, active, false,
        "", "",
    ).await.unwrap();
}

// ==================== extract_model_from_body Tests ====================

#[tokio::test]
async fn test_extract_model_from_body_valid() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let body = br#"{"model":"gpt-4","messages":[]}"#;
    assert_eq!(proxy.extract_model_from_body(body), Some("gpt-4".to_string()));
}

#[tokio::test]
async fn test_extract_model_from_body_no_model() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let body = br#"{"messages":[]}"#;
    assert_eq!(proxy.extract_model_from_body(body), None);
}

#[tokio::test]
async fn test_extract_model_from_body_invalid_json() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let body = b"not json";
    assert_eq!(proxy.extract_model_from_body(body), None);
}

#[tokio::test]
async fn test_extract_model_from_body_empty() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let body = b"";
    assert_eq!(proxy.extract_model_from_body(body), None);
}

#[tokio::test]
async fn test_extract_model_from_body_model_null() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let body = br#"{"model":null}"#;
    assert_eq!(proxy.extract_model_from_body(body), None);
}

#[tokio::test]
async fn test_extract_model_from_body_model_number() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let body = br#"{"model":42}"#;
    assert_eq!(proxy.extract_model_from_body(body), None);
}

#[tokio::test]
async fn test_extract_model_from_body_model_object() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let body = br#"{"model":{"name":"gpt-4"}}"#;
    assert_eq!(proxy.extract_model_from_body(body), None);
}

#[tokio::test]
async fn test_extract_model_from_body_nested_model() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let body = br#"{"data":{"model":"gpt-4"}}"#;
    assert_eq!(proxy.extract_model_from_body(body), None);
}

// ==================== select_provider Edge Cases ====================

#[tokio::test]
async fn test_select_provider_single() {
    let db = test_db().await;
    create_test_provider(&db, "single-p", 1, true).await;
    let proxy = make_proxy(db);
    let provider = proxy.select_provider(None).await.unwrap();
    assert_eq!(provider.id, "single-p");
}

#[tokio::test]
async fn test_select_provider_no_providers() {
    let db = test_db().await;
    let proxy = make_proxy(db);
    let result = proxy.select_provider(None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_select_provider_skips_inactive() {
    let db = test_db().await;
    create_test_provider(&db, "inactive-p", 1, true).await;
    // Deactivate the provider
    db.batch_set_provider_active_status(&["inactive-p".to_string()], false).await.unwrap();
    let proxy = make_proxy(db);
    let result = proxy.select_provider(None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_select_provider_prefers_active() {
    let db = test_db().await;
    create_test_provider(&db, "active-p", 1, true).await;
    create_test_provider(&db, "inactive-p2", 2, true).await;
    db.batch_set_provider_active_status(&["inactive-p2".to_string()], false).await.unwrap();
    let proxy = make_proxy(db);
    let provider = proxy.select_provider(None).await.unwrap();
    assert_eq!(provider.id, "active-p");
}

#[tokio::test]
async fn test_select_provider_with_allowed_list() {
    let db = test_db().await;
    create_test_provider(&db, "allowed-p", 1, true).await;
    create_test_provider(&db, "not-allowed-p", 2, true).await;
    let proxy = make_proxy(db);
    let allowed = vec!["allowed-p".to_string()];
    let provider = proxy.select_provider(Some(&allowed)).await.unwrap();
    assert_eq!(provider.id, "allowed-p");
}

#[tokio::test]
async fn test_select_provider_allowed_list_no_match() {
    let db = test_db().await;
    create_test_provider(&db, "existing-p", 1, true).await;
    let proxy = make_proxy(db);
    let allowed = vec!["nonexistent-p".to_string()];
    let result = proxy.select_provider(Some(&allowed)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_select_provider_weighted() {
    let db = test_db().await;
    create_test_provider(&db, "heavy-p", 10, true).await;
    create_test_provider(&db, "light-p", 1, true).await;
    let proxy = make_proxy(db);
    // With weight 10 vs 1, heavy-p should be selected more often
    let mut heavy_count = 0;
    for _ in 0..100 {
        let provider = proxy.select_provider(None).await.unwrap();
        if provider.id == "heavy-p" {
            heavy_count += 1;
        }
    }
    // Should be selected more than 50% of the time
    assert!(heavy_count > 50);
}
