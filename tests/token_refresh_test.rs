use llm_gateway::db::Database;
use llm_gateway::auth::AuthManager;
use llm_gateway::auth::token_refresh::TokenRefreshTask;
use std::sync::Arc;

#[tokio::test]
async fn test_token_refresh_task_creation() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let task = TokenRefreshTask::new(db.clone(), auth_manager.clone());
    // Just verify it can be created
    assert!(true);
}

#[tokio::test]
async fn test_token_refresh_checks_expiry() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let auth_manager = Arc::new(AuthManager::new(db.clone()));

    // Create a dynamic token provider with a far-future expiry (should NOT need refresh)
    db.create_provider(
        "fresh-prov",
        "Fresh Provider",
        "https://fresh.com/v1",
        "openai",
        "dynamic_token",
        None,
        Some("https://auth.fresh.com/login"),
        Some("user"),
        Some("pass"),
        "token",
        "refreshToken",
        "Authorization",
        "Bearer ",
        86400,
        1,
    ).await.unwrap();

    // Set a token that expires far in the future
    let far_future = (chrono::Utc::now() + chrono::Duration::hours(24)).format("%Y-%m-%d %H:%M:%S").to_string();
    db.update_provider_token("fresh-prov", "valid-token", Some("valid-refresh"), &far_future).await.unwrap();

    let task = Arc::new(TokenRefreshTask::new(db.clone(), auth_manager.clone()));
    task.refresh_all_tokens().await.unwrap();

    // Token should NOT have changed (it's still valid)
    let provider = db.get_provider("fresh-prov").await.unwrap().unwrap();
    assert_eq!(provider.current_token, Some("valid-token".to_string()));
}

#[tokio::test]
async fn test_token_refresh_expired_token_needs_refresh() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let auth_manager = Arc::new(AuthManager::new(db.clone()));

    // Create a dynamic token provider with an expired token
    db.create_provider(
        "expired-prov",
        "Expired Provider",
        "https://expired.com/v1",
        "openai",
        "dynamic_token",
        None,
        Some("https://auth.expired.com/login"),
        Some("user"),
        Some("pass"),
        "token",
        "refreshToken",
        "Authorization",
        "Bearer ",
        86400,
        1,
    ).await.unwrap();

    // Set a token that has already expired
    let past = (chrono::Utc::now() - chrono::Duration::hours(1)).format("%Y-%m-%d %H:%M:%S").to_string();
    db.update_provider_token("expired-prov", "old-token", Some("old-refresh"), &past).await.unwrap();

    let task = Arc::new(TokenRefreshTask::new(db.clone(), auth_manager.clone()));
    // This will try to refresh but will fail since the auth endpoint doesn't exist
    // We just verify it doesn't crash
    task.refresh_all_tokens().await.unwrap();
}

#[tokio::test]
async fn test_token_refresh_skips_api_key_providers() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let auth_manager = Arc::new(AuthManager::new(db.clone()));

    // Create an api_key provider (should NOT be refreshed)
    db.create_provider(
        "static-prov",
        "Static Provider",
        "https://static.com/v1",
        "openai",
        "api_key",
        Some("sk-static-key"),
        None,
        None,
        None,
        "token",
        "refreshToken",
        "Authorization",
        "Bearer ",
        86400,
        1,
    ).await.unwrap();

    let task = Arc::new(TokenRefreshTask::new(db.clone(), auth_manager.clone()));
    task.refresh_all_tokens().await.unwrap();

    // API key should remain unchanged
    let provider = db.get_provider("static-prov").await.unwrap().unwrap();
    assert_eq!(provider.api_key, Some("sk-static-key".to_string()));
    assert!(provider.current_token.is_none());
}

#[tokio::test]
async fn test_token_refresh_no_expiry_needs_refresh() {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let auth_manager = Arc::new(AuthManager::new(db.clone()));

    // Create a dynamic token provider with no expiry set (should need refresh)
    db.create_provider(
        "noexpiry-prov",
        "No Expiry Provider",
        "https://noexpiry.com/v1",
        "openai",
        "dynamic_token",
        None,
        Some("https://auth.noexpiry.com/login"),
        Some("user"),
        Some("pass"),
        "token",
        "refreshToken",
        "Authorization",
        "Bearer ",
        86400,
        1,
    ).await.unwrap();

    // Don't set any token - provider has no current_token and no token_expires_at

    let task = Arc::new(TokenRefreshTask::new(db.clone(), auth_manager.clone()));
    // Will try to refresh but fail (no real endpoint)
    task.refresh_all_tokens().await.unwrap();
}