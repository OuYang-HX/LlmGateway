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

// ==================== DB: list_active_models ====================

#[tokio::test]
async fn test_list_active_models_empty() {
    let db = test_db().await;
    let models = db.list_active_models().await.unwrap();
    assert!(models.is_empty());
}

#[tokio::test]
async fn test_list_active_models_filters_inactive() {
    let db = test_db().await;
    db.create_model("active-m", "Active", None, "chat", 1, None).await.unwrap();
    db.create_model("inactive-m", "Inactive", None, "chat", 2, None).await.unwrap();
    db.update_model("inactive-m", "Inactive", None, "chat", false, 2, None).await.unwrap();
    
    let models = db.list_active_models().await.unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "active-m");
}

#[tokio::test]
async fn test_list_active_models_orders_by_priority() {
    let db = test_db().await;
    db.create_model("low-p", "Low Priority", None, "chat", 1, None).await.unwrap();
    db.create_model("high-p", "High Priority", None, "chat", 10, None).await.unwrap();
    db.create_model("mid-p", "Mid Priority", None, "chat", 5, None).await.unwrap();
    
    let models = db.list_active_models().await.unwrap();
    assert_eq!(models.len(), 3);
    assert_eq!(models[0].id, "high-p");
    assert_eq!(models[1].id, "mid-p");
    assert_eq!(models[2].id, "low-p");
}

// ==================== DB: get_model_by_external_id ====================

#[tokio::test]
async fn test_get_model_by_external_id_found() {
    let db = test_db().await;
    db.create_model("my-gpt4", "My GPT-4", None, "chat", 1, None).await.unwrap();
    let result = db.get_model_by_external_id("my-gpt4").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "my-gpt4");
}

#[tokio::test]
async fn test_get_model_by_external_id_not_found() {
    let db = test_db().await;
    let result = db.get_model_by_external_id("nonexistent").await.unwrap();
    assert!(result.is_none());
}

// ==================== DB: delete_request_logs_filtered ====================

#[tokio::test]
async fn test_delete_request_logs_filtered_by_provider() {
    let db = test_db().await;
    create_test_provider(&db, "del-prov").await;
    create_test_provider(&db, "keep-prov").await;
    
    db.insert_request_log("key1", "del-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key1", "keep-prov", Some("model-b"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    
    let deleted = db.delete_request_logs_filtered(None, Some("del-prov"), None, None, None, None, None).await.unwrap();
    assert_eq!(deleted, 1);
    let remaining = db.count_request_logs(None, None, Some("1970-01-01T00:00:00Z"), Some("2099-01-01T00:00:00Z"), None, None, None).await.unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn test_delete_request_logs_filtered_by_api_key() {
    let db = test_db().await;
    create_test_provider(&db, "df-prov").await;
    
    db.insert_request_log("key-del", "df-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key-keep", "df-prov", Some("model-b"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    
    let deleted = db.delete_request_logs_filtered(Some("key-del"), None, None, None, None, None, None).await.unwrap();
    assert_eq!(deleted, 1);
    let remaining = db.count_request_logs(None, None, Some("1970-01-01T00:00:00Z"), Some("2099-01-01T00:00:00Z"), None, None, None).await.unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn test_delete_request_logs_filtered_by_model() {
    let db = test_db().await;
    create_test_provider(&db, "dfm-prov").await;
    
    db.insert_request_log("key1", "dfm-prov", Some("gpt-4"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key1", "dfm-prov", Some("gpt-3.5"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    
    let deleted = db.delete_request_logs_filtered(None, None, None, None, None, Some("gpt-4"), None).await.unwrap();
    assert_eq!(deleted, 1);
    let remaining = db.count_request_logs(None, None, Some("1970-01-01T00:00:00Z"), Some("2099-01-01T00:00:00Z"), None, None, None).await.unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn test_delete_request_logs_filtered_by_status() {
    let db = test_db().await;
    create_test_provider(&db, "dfs-prov").await;
    
    db.insert_request_log("key1", "dfs-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key1", "dfs-prov", Some("model-b"), "/v1/chat", "POST", None, None, Some(500), None, None, 10, 20, 30, Some(200), false, false, None).await.unwrap();
    
    let deleted = db.delete_request_logs_filtered(None, None, None, None, None, None, Some("error")).await.unwrap();
    assert_eq!(deleted, 1);
    let remaining = db.count_request_logs(None, None, Some("1970-01-01T00:00:00Z"), Some("2099-01-01T00:00:00Z"), None, None, None).await.unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn test_delete_request_logs_filtered_no_match() {
    let db = test_db().await;
    let deleted = db.delete_request_logs_filtered(Some("nonexistent-key"), None, None, None, None, None, None).await.unwrap();
    assert_eq!(deleted, 0);
}

#[tokio::test]
async fn test_delete_request_logs_filtered_all_params() {
    let db = test_db().await;
    create_test_provider(&db, "all-prov").await;
    db.insert_request_log("all-key", "all-prov", Some("all-model"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    
    let deleted = db.delete_request_logs_filtered(
        Some("all-key"), Some("all-prov"), None, None, None, Some("all-model"), None
    ).await.unwrap();
    assert_eq!(deleted, 1);
}

// ==================== DB: delete_request_logs_by_api_key ====================

#[tokio::test]
async fn test_delete_request_logs_by_api_key() {
    let db = test_db().await;
    create_test_provider(&db, "dak-prov").await;
    
    db.insert_request_log("del-key", "dak-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("keep-key", "dak-prov", Some("model-b"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    
    let deleted = db.delete_request_logs_by_api_key("del-key").await.unwrap();
    assert_eq!(deleted, 1);
    let remaining = db.count_request_logs(None, None, Some("1970-01-01T00:00:00Z"), Some("2099-01-01T00:00:00Z"), None, None, None).await.unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn test_delete_request_logs_by_api_key_not_found() {
    let db = test_db().await;
    let deleted = db.delete_request_logs_by_api_key("nonexistent").await.unwrap();
    assert_eq!(deleted, 0);
}

// ==================== DB: multi-mapping edge cases ====================

#[tokio::test]
async fn test_same_provider_model_same_unified_model_rejected() {
    let db = test_db().await;
    create_test_provider(&db, "dup-prov").await;
    db.create_model("dup-model", "Dup Model", None, "chat", 1, None).await.unwrap();
    
    db.add_model_mapping("dup-model", "dup-prov", "gpt-4", 1, 1.0).await.unwrap();
    let result = db.add_model_mapping("dup-model", "dup-prov", "gpt-4", 2, 0.5).await;
    assert!(result.is_err(), "Duplicate mapping should fail UNIQUE constraint");
}

#[tokio::test]
async fn test_different_provider_model_same_provider_ok() {
    let db = test_db().await;
    create_test_provider(&db, "diff-prov").await;
    db.create_model("diff-model", "Diff Model", None, "chat", 1, None).await.unwrap();
    
    db.add_model_mapping("diff-model", "diff-prov", "gpt-4", 1, 1.0).await.unwrap();
    db.add_model_mapping("diff-model", "diff-prov", "gpt-4o", 2, 0.5).await.unwrap();
    
    let mappings = db.list_model_mappings("diff-model").await.unwrap();
    assert_eq!(mappings.len(), 2);
}

#[tokio::test]
async fn test_update_model_mapping_by_old_provider_model_id() {
    let db = test_db().await;
    create_test_provider(&db, "upd-prov").await;
    db.create_model("upd-model", "Upd Model", None, "chat", 1, None).await.unwrap();
    
    db.add_model_mapping("upd-model", "upd-prov", "gpt-4", 1, 1.0).await.unwrap();
    db.add_model_mapping("upd-model", "upd-prov", "gpt-4o", 2, 0.5).await.unwrap();
    
    db.update_model_mapping("upd-model", "upd-prov", "gpt-4", "gpt-4-turbo", true, 3, 0.8).await.unwrap();
    
    let mappings = db.list_model_mappings("upd-model").await.unwrap();
    assert_eq!(mappings.len(), 2);
    
    let gpt4_turbo = mappings.iter().find(|m| m.provider_model_id == "gpt-4-turbo");
    assert!(gpt4_turbo.is_some());
    assert_eq!(gpt4_turbo.unwrap().weight, 3);
    
    let gpt4o = mappings.iter().find(|m| m.provider_model_id == "gpt-4o");
    assert!(gpt4o.is_some());
    assert_eq!(gpt4o.unwrap().weight, 2);
}

// ==================== DB: provider mock_mode ====================

#[tokio::test]
async fn test_provider_mock_mode_default_false() {
    let db = test_db().await;
    create_test_provider(&db, "mock-prov").await;
    let provider = db.get_provider("mock-prov").await.unwrap().unwrap();
    assert!(!provider.mock_mode);
}

#[tokio::test]
async fn test_provider_mock_mode_sql_update() {
    let db = test_db().await;
    create_test_provider(&db, "mock-sql").await;
    // Update mock_mode via SQL
    sqlx::query("UPDATE providers SET mock_mode = 1 WHERE id = ?")
        .bind("mock-sql")
        .execute(&db.pool).await.unwrap();
    let provider = db.get_provider("mock-sql").await.unwrap().unwrap();
    assert!(provider.mock_mode);
}

// ==================== DB: find_providers_with_model ====================

#[tokio::test]
async fn test_find_providers_with_model_no_match() {
    let db = test_db().await;
    create_test_provider(&db, "fpm-prov").await;
    let providers = db.find_providers_with_model("nonexistent-model").await.unwrap();
    assert!(providers.is_empty());
}

#[tokio::test]
async fn test_find_providers_with_model_inactive_excluded() {
    let db = test_db().await;
    create_test_provider(&db, "fpm-inactive").await;
    db.add_provider_model("fpm-inactive", "gpt-4").await.unwrap();
    db.deactivate_provider("fpm-inactive").await.unwrap();
    let providers = db.find_providers_with_model("gpt-4").await.unwrap();
    assert!(providers.is_empty());
}

// ==================== DB: is_model_allowed_for_provider ====================

#[tokio::test]
async fn test_is_model_allowed_no_provider_models() {
    let db = test_db().await;
    create_test_provider(&db, "ima-prov").await;
    let allowed = db.is_model_allowed_for_provider("ima-prov", "any-model").await.unwrap();
    assert!(allowed);
}

#[tokio::test]
async fn test_is_model_allowed_with_specific_models() {
    let db = test_db().await;
    create_test_provider(&db, "ima-spec").await;
    db.add_provider_model("ima-spec", "gpt-4").await.unwrap();
    db.add_provider_model("ima-spec", "gpt-4o").await.unwrap();
    
    let allowed = db.is_model_allowed_for_provider("ima-spec", "gpt-4").await.unwrap();
    assert!(allowed);
    let not_allowed = db.is_model_allowed_for_provider("ima-spec", "claude-3").await.unwrap();
    assert!(!not_allowed);
}

// ==================== DB: cleanup_mappings_for_provider_model ====================

#[tokio::test]
async fn test_cleanup_mappings_removes_stale() {
    let db = test_db().await;
    create_test_provider(&db, "cleanup-prov").await;
    db.create_model("cleanup-model", "Cleanup", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("cleanup-model", "cleanup-prov", "stale-model", 1, 1.0).await.unwrap();
    
    db.cleanup_mappings_for_provider_model("cleanup-prov", "stale-model").await.unwrap();
    let mappings = db.list_model_mappings("cleanup-model").await.unwrap();
    assert!(mappings.is_empty(), "Stale mapping should be cleaned up");
}

#[tokio::test]
async fn test_cleanup_mappings_preserves_valid() {
    let db = test_db().await;
    create_test_provider(&db, "valid-prov").await;
    db.create_model("valid-model", "Valid", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("valid-model", "valid-prov", "existing-model", 1, 1.0).await.unwrap();
    db.add_provider_model("valid-prov", "existing-model").await.unwrap();
    
    // Cleanup a different model - should not affect existing mapping
    db.cleanup_mappings_for_provider_model("valid-prov", "other-model").await.unwrap();
    let mappings = db.list_model_mappings("valid-model").await.unwrap();
    assert_eq!(mappings.len(), 1);
}

// ==================== DB: get_provider_health_stats ====================

#[tokio::test]
async fn test_get_provider_health_stats_with_data() {
    let db = test_db().await;
    create_test_provider(&db, "hs-prov").await;
    db.insert_request_log("key1", "hs-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key1", "hs-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(500), None, None, 10, 20, 30, Some(200), false, false, None).await.unwrap();
    
    let stats = db.get_provider_health_stats().await.unwrap();
    assert!(!stats.is_empty());
}

// ==================== DB: delete_request_logs_by_provider ====================

#[tokio::test]
async fn test_delete_request_logs_by_provider() {
    let db = test_db().await;
    create_test_provider(&db, "dlbp-prov").await;
    create_test_provider(&db, "keep-dlbp").await;
    
    db.insert_request_log("key1", "dlbp-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key1", "keep-dlbp", Some("model-b"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    
    let deleted = db.delete_request_logs_by_provider("dlbp-prov").await.unwrap();
    assert_eq!(deleted, 1);
    let remaining = db.count_request_logs(None, None, Some("1970-01-01T00:00:00Z"), Some("2099-01-01T00:00:00Z"), None, None, None).await.unwrap();
    assert_eq!(remaining, 1);
}

// ==================== DB: set_provider_models ====================

#[tokio::test]
async fn test_set_provider_models_replaces_existing() {
    let db = test_db().await;
    create_test_provider(&db, "spm-prov").await;
    db.add_provider_model("spm-prov", "gpt-3.5").await.unwrap();
    
    db.set_provider_models("spm-prov", &["gpt-4".to_string(), "gpt-4o".to_string()]).await.unwrap();
    
    let models = db.list_provider_models("spm-prov").await.unwrap();
    assert_eq!(models.len(), 2);
    let model_ids: Vec<&str> = models.iter().map(|m| m.model_id.as_str()).collect();
    assert!(model_ids.contains(&"gpt-4"));
    assert!(model_ids.contains(&"gpt-4o"));
    assert!(!model_ids.contains(&"gpt-3.5"));
}

#[tokio::test]
async fn test_set_provider_models_empty_clears_all() {
    let db = test_db().await;
    create_test_provider(&db, "spm-empty").await;
    db.add_provider_model("spm-empty", "gpt-4").await.unwrap();
    
    db.set_provider_models("spm-empty", &[]).await.unwrap();
    let models = db.list_provider_models("spm-empty").await.unwrap();
    assert!(models.is_empty());
}
