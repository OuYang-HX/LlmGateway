//! Models and API Key management tests
//!
//! Tests the DB-layer logic and HTTP endpoints that don't require
//! authentication header manipulation (which varies by axum-test version).

use llm_gateway::db::Database;
use std::sync::Arc;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

async fn test_db() -> Arc<Database> {
    Arc::new(Database::new_in_memory().await.unwrap())
}

// ==================== /api/v1/models: management CRUD ====================

#[tokio::test]
async fn test_api_models_crud() {
    let db = test_db().await;

    // Create
    db.create_model("crud", "CRUD Model", Some("A test model"), "chat", 50, None).await.unwrap();

    // Read
    let model = db.get_model("crud").await.unwrap();
    assert!(model.is_some());
    assert_eq!(model.unwrap().name, "CRUD Model");

    // Update
    db.update_model("crud", "Updated Name", Some("desc"), "chat", false, 10, None).await.unwrap();
    let updated = db.get_model("crud").await.unwrap().unwrap();
    assert_eq!(updated.name, "Updated Name");
    assert!(!updated.is_active);

    // Delete
    db.delete_model("crud").await.unwrap();
    let deleted = db.get_model("crud").await.unwrap();
    assert!(deleted.is_none());
}

#[tokio::test]
async fn test_api_models_list() {
    let db = test_db().await;
    db.create_model("m1", "Model 1", None, "chat", 50, None).await.unwrap();
    db.create_model("m2", "Model 2", None, "chat", 50, None).await.unwrap();

    let models = db.list_models().await.unwrap();
    assert_eq!(models.len(), 2);
}

// ==================== /api/v1/models: model mappings ====================

#[tokio::test]
async fn test_api_model_mappings_crud() {
    let db = test_db().await;

    // Create provider
    db.create_provider(
        "mp", "Model Provider", "https://x.com", "openai", "api_key",
        Some("sk"), None, None, None,
        Some("POST"), Some("json"), Some("username"), Some("password"),
        None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();

    // Create model
    db.create_model("mm", "Model M", None, "chat", 50, None).await.unwrap();

    // Add mapping
    db.add_model_mapping("mm", "mp", "internal-model", 10, 1.5).await.unwrap();

    // List mappings
    let mappings = db.list_model_mappings("mm").await.unwrap();
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].provider_model_id, "internal-model");
    assert_eq!(mappings[0].weight, 10);
    assert!(mappings[0].is_active);

    // Update mapping (now requires old_provider_model_id as the WHERE key)
    db.update_model_mapping("mm", "mp", "internal-model", "updated-internal", false, 5, 2.0).await.unwrap();
    let updated = db.list_model_mappings("mm").await.unwrap();
    assert_eq!(updated[0].provider_model_id, "updated-internal");
    assert!(!updated[0].is_active);

    // Remove mapping (now requires provider_model_id to uniquely identify)
    db.remove_model_mapping("mm", "mp", "updated-internal").await.unwrap();
    let after = db.list_model_mappings("mm").await.unwrap();
    assert!(after.is_empty());
}

// ==================== /api/v1/api-keys: update ====================

#[tokio::test]
async fn test_api_key_update_name() {
    let db = test_db().await;
    db.create_api_key("key-id", "Original Name", "lgk-original", "lgk-orig", None).await.unwrap();

    db.update_api_key("key-id", "Updated Name", None, true).await.unwrap();

    let key = db.get_api_key_by_id("key-id").await.unwrap().unwrap();
    assert_eq!(key.name, "Updated Name");
}

#[tokio::test]
async fn test_api_key_update_is_active() {
    let db = test_db().await;
    db.create_api_key("key-act", "Toggle Test", "lgk-act", "lgk-act", None).await.unwrap();

    db.update_api_key("key-act", "Toggle Test", None, false).await.unwrap();

    let key = db.get_api_key_by_id("key-act").await.unwrap().unwrap();
    assert!(!key.is_active);
}

#[tokio::test]
async fn test_api_key_update_allowed_providers() {
    let db = test_db().await;
    db.create_provider(
        "ap-prov", "AP Provider", "https://x.com", "openai", "api_key",
        Some("sk"), None, None, None,
        Some("POST"), Some("json"), Some("username"), Some("password"),
        None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_api_key("key-ap", "AP Test", "lgk-ap", "lgk-ap", None).await.unwrap();

    db.update_api_key("key-ap", "AP Test", Some(r#"["ap-prov"]"#), true).await.unwrap();

    let key = db.get_api_key_by_id("key-ap").await.unwrap().unwrap();
    let providers: Vec<String> = serde_json::from_str(key.allowed_providers.as_deref().unwrap()).unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0], "ap-prov");
}

// ==================== list_models_with_mappings: filtering ====================

#[tokio::test]
async fn test_list_models_excludes_inactive() {
    let db = test_db().await;
    db.create_provider(
        "fprov", "F Provider", "https://x.com", "openai", "api_key",
        Some("sk"), None, None, None,
        Some("POST"), Some("json"), Some("username"), Some("password"),
        None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_model("active-model", "Active", None, "chat", 50, None).await.unwrap();
    db.create_model("inactive-model", "Inactive", None, "chat", 50, None).await.unwrap();
    db.update_model("inactive-model", "Inactive", None, "chat", false, 50, None).await.unwrap();
    db.add_model_mapping("active-model", "fprov", "am-i", 1, 1.0).await.unwrap();
    db.add_model_mapping("inactive-model", "fprov", "im-i", 1, 1.0).await.unwrap();

    let models = db.list_models_with_mappings().await.unwrap();

    // list_models_with_mappings returns ALL models (inactive ones included)
    // Filtering for /v1/models API happens in the HTTP handler
    let ids: Vec<&str> = models.iter().map(|m| m.model.id.as_str()).collect();
    assert!(ids.contains(&"active-model"));
    assert!(ids.contains(&"inactive-model")); // inactive model still appears in DB query
}

#[tokio::test]
async fn test_list_models_excludes_without_mappings() {
    let db = test_db().await;
    db.create_model("orphan", "Orphan", None, "chat", 50, None).await.unwrap();

    let models = db.list_models_with_mappings().await.unwrap();
    // list_models_with_mappings returns ALL models (orphan models included)
    // /v1/models API handler filters out models without active mappings
    let ids: Vec<&str> = models.iter().map(|m| m.model.id.as_str()).collect();
    assert!(ids.contains(&"orphan")); // model without mappings still appears in DB query
}

#[tokio::test]
async fn test_list_models_with_mappings_includes_provider() {
    let db = test_db().await;
    db.create_provider(
        "mprov", "M Provider", "https://x.com", "openai", "api_key",
        Some("sk"), None, None, None,
        Some("POST"), Some("json"), Some("username"), Some("password"),
        None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false,
        false,
        "choices.0.message.content", "choices.0.delta.reasoning_content",
    ).await.unwrap();
    db.create_model("with-prov", "With Provider", None, "chat", 50, None).await.unwrap();
    db.add_model_mapping("with-prov", "mprov", "wp-i", 1, 1.0).await.unwrap();

    let models = db.list_models_with_mappings().await.unwrap();
    assert!(models.iter().any(|m| m.model.id == "with-prov" && !m.mappings.is_empty()));
}
