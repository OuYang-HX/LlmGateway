use llm_gateway::db::Database;
use llm_gateway::utils;
use llm_gateway::auth::extract_json_path;
use std::sync::Arc;

async fn test_db() -> Arc<Database> {
    Arc::new(Database::new_in_memory().await.expect("Failed to create test DB"))
}

async fn create_test_provider(db: &Database, id: &str) {
    db.create_provider(
        id, id, "https://api.example.com/v1", "openai", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false, false,
        "", "", false,
    ).await.unwrap();
}

async fn create_test_api_key(db: &Database, id: &str) {
    db.create_api_key(id, id, &format!("lgk-{}", id), &format!("lgk-{}-xxx", id), None).await.unwrap();
}

#[test]
fn test_sha256_hash_empty_string() {
    assert_eq!(utils::sha256_hash("").len(), 64);
}

#[test]
fn test_sha256_hash_known_value() {
    assert_eq!(utils::sha256_hash("hello"), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_sha256_hash_unicode() {
    let h1 = utils::sha256_hash("你好世界");
    let h2 = utils::sha256_hash("hello world");
    assert_eq!(h1.len(), 64);
    assert_ne!(h1, h2);
}

#[test]
fn test_sha256_hash_deterministic() {
    assert_eq!(utils::sha256_hash("test-input"), utils::sha256_hash("test-input"));
}

#[test]
fn test_sha256_hash_long_input() {
    assert_eq!(utils::sha256_hash(&"a".repeat(10000)).len(), 64);
}

#[test]
fn test_extract_json_path_simple() {
    let v = serde_json::json!({"token": "abc123"});
    assert_eq!(extract_json_path(&v, "token").and_then(|v| v.as_str()), Some("abc123"));
}

#[test]
fn test_extract_json_path_nested() {
    let v = serde_json::json!({"result": {"token": "abc123"}});
    assert_eq!(extract_json_path(&v, "result.token").and_then(|v| v.as_str()), Some("abc123"));
}

#[test]
fn test_extract_json_path_deep() {
    let v = serde_json::json!({"a": {"b": {"c": {"d": "deep"}}}});
    assert_eq!(extract_json_path(&v, "a.b.c.d").and_then(|v| v.as_str()), Some("deep"));
}

#[test]
fn test_extract_json_path_missing() {
    let v = serde_json::json!({"token": "abc123"});
    assert!(extract_json_path(&v, "nonexistent").is_none());
    assert!(extract_json_path(&v, "result.nonexistent").is_none());
    assert!(extract_json_path(&v, "missing.token").is_none());
}

#[test]
fn test_extract_json_path_types() {
    let v = serde_json::json!({"num": 42, "flag": true, "nil": null, "arr": [1,2], "obj": {"k":"v"}});
    assert_eq!(extract_json_path(&v, "num").and_then(|v| v.as_i64()), Some(42));
    assert_eq!(extract_json_path(&v, "flag").and_then(|v| v.as_bool()), Some(true));
    assert!(extract_json_path(&v, "nil").unwrap().is_null());
    assert!(extract_json_path(&v, "arr").unwrap().is_array());
    assert!(extract_json_path(&v, "obj").unwrap().is_object());
}

#[test]
fn test_extract_json_path_empty_path() {
    let v = serde_json::json!({"key": "val"});
    assert!(extract_json_path(&v, "").is_none());
}

#[test]
fn test_extract_json_path_access_token() {
    let v = serde_json::json!({"data": {"access_token": "tok_abc", "expires_in": 3600}});
    assert_eq!(extract_json_path(&v, "data.access_token").and_then(|v| v.as_str()), Some("tok_abc"));
}

#[tokio::test]
async fn test_update_api_key_name() {
    let db = test_db().await;
    create_test_api_key(&db, "upd-key").await;
    assert!(db.update_api_key("upd-key", "New Name", None, true).await.unwrap());
    assert_eq!(db.get_api_key_by_id("upd-key").await.unwrap().unwrap().name, "New Name");
}

#[tokio::test]
async fn test_update_api_key_deactivate() {
    let db = test_db().await;
    create_test_api_key(&db, "deact-key").await;
    db.update_api_key("deact-key", "Deact Key", None, false).await.unwrap();
    assert!(!db.get_api_key_by_id("deact-key").await.unwrap().unwrap().is_active);
}

#[tokio::test]
async fn test_update_api_key_with_providers() {
    let db = test_db().await;
    create_test_api_key(&db, "prov-key").await;
    let providers = serde_json::to_string(&vec!["prov-a".to_string()]).unwrap();
    db.update_api_key("prov-key", "Prov Key", Some(&providers), true).await.unwrap();
    assert!(db.get_api_key_by_id("prov-key").await.unwrap().unwrap().allowed_providers.is_some());
}

#[tokio::test]
async fn test_update_api_key_nonexistent() {
    let db = test_db().await;
    assert!(!db.update_api_key("nonexistent", "Name", None, true).await.unwrap());
}

#[tokio::test]
async fn test_update_api_key_reactivate() {
    let db = test_db().await;
    create_test_api_key(&db, "react-key").await;
    db.update_api_key("react-key", "React Key", None, false).await.unwrap();
    assert!(!db.get_api_key_by_id("react-key").await.unwrap().unwrap().is_active);
    db.update_api_key("react-key", "React Key", None, true).await.unwrap();
    assert!(db.get_api_key_by_id("react-key").await.unwrap().unwrap().is_active);
}

#[tokio::test]
async fn test_regenerate_api_key() {
    let db = test_db().await;
    create_test_api_key(&db, "regen-key").await;
    assert!(db.regenerate_api_key("regen-key", "lgk-new-secret", "lgk-new-xxx").await.unwrap());
    let key = db.get_api_key_by_key("lgk-new-secret").await.unwrap().unwrap();
    assert_eq!(key.id, "regen-key");
    assert_eq!(key.key_prefix, "lgk-new-xxx");
}

#[tokio::test]
async fn test_regenerate_api_key_nonexistent() {
    let db = test_db().await;
    assert!(!db.regenerate_api_key("nonexistent", "lgk-new", "lgk-new-xxx").await.unwrap());
}

#[tokio::test]
async fn test_regenerate_api_key_old_key_invalid() {
    let db = test_db().await;
    create_test_api_key(&db, "old-key").await;
    db.regenerate_api_key("old-key", "lgk-new-secret", "lgk-new-xxx").await.unwrap();
    assert!(db.get_api_key_by_key("lgk-old-key").await.unwrap().is_none());
}

#[tokio::test]
async fn test_update_provider_cookies() {
    let db = test_db().await;
    create_test_provider(&db, "cookie-prov").await;
    db.update_provider_cookies("cookie-prov", "session=abc123; token=xyz").await.unwrap();
    let p = db.get_provider("cookie-prov").await.unwrap().unwrap();
    assert_eq!(p.token_cookies, Some("session=abc123; token=xyz".to_string()));
}

#[tokio::test]
async fn test_batch_set_provider_active_status() {
    let db = test_db().await;
    create_test_provider(&db, "batch-p1").await;
    create_test_provider(&db, "batch-p2").await;
    let count = db.batch_set_provider_active_status(&["batch-p1".to_string(), "batch-p2".to_string()], false).await.unwrap();
    assert_eq!(count, 2);
    assert!(!db.get_provider("batch-p1").await.unwrap().unwrap().is_active);
    assert!(!db.get_provider("batch-p2").await.unwrap().unwrap().is_active);
}

#[tokio::test]
async fn test_batch_set_provider_active_status_reactivate() {
    let db = test_db().await;
    create_test_provider(&db, "react-p1").await;
    db.batch_set_provider_active_status(&["react-p1".to_string()], false).await.unwrap();
    assert!(!db.get_provider("react-p1").await.unwrap().unwrap().is_active);
    db.batch_set_provider_active_status(&["react-p1".to_string()], true).await.unwrap();
    assert!(db.get_provider("react-p1").await.unwrap().unwrap().is_active);
}

#[tokio::test]
async fn test_batch_set_provider_active_status_empty() {
    let db = test_db().await;
    let count = db.batch_set_provider_active_status(&[], false).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_get_provider_model() {
    let db = test_db().await;
    create_test_provider(&db, "pm-prov").await;
    db.add_provider_model("pm-prov", "gpt-4").await.unwrap();
    let model = db.get_provider_model("pm-prov", "gpt-4").await.unwrap().unwrap();
    assert_eq!(model.provider_id, "pm-prov");
    assert_eq!(model.model_id, "gpt-4");
}

#[tokio::test]
async fn test_get_provider_model_not_found() {
    let db = test_db().await;
    create_test_provider(&db, "pm-nf").await;
    assert!(db.get_provider_model("pm-nf", "nonexistent").await.unwrap().is_none());
}

#[tokio::test]
async fn test_update_provider_model_test_status() {
    let db = test_db().await;
    create_test_provider(&db, "ms-prov").await;
    db.add_provider_model("ms-prov", "gpt-4").await.unwrap();
    db.update_provider_model_test_status("ms-prov", "gpt-4", "success", Some("OK"), true).await.unwrap();
    let model = db.get_provider_model("ms-prov", "gpt-4").await.unwrap().unwrap();
    assert!(model.is_active);
}

#[tokio::test]
async fn test_update_provider_model_test_status_failed() {
    let db = test_db().await;
    create_test_provider(&db, "ms-fail").await;
    db.add_provider_model("ms-fail", "gpt-4").await.unwrap();
    db.update_provider_model_test_status("ms-fail", "gpt-4", "failed", Some("Connection error"), false).await.unwrap();
    let model = db.get_provider_model("ms-fail", "gpt-4").await.unwrap().unwrap();
    assert!(!model.is_active);
}

#[tokio::test]
async fn test_set_provider_models() {
    let db = test_db().await;
    create_test_provider(&db, "set-pm").await;
    db.add_provider_model("set-pm", "gpt-3.5").await.unwrap();
    db.set_provider_models("set-pm", &["gpt-4".to_string(), "gpt-4o".to_string()]).await.unwrap();
    let models = db.list_provider_models("set-pm").await.unwrap();
    assert_eq!(models.len(), 2);
}

#[tokio::test]
async fn test_find_providers_with_model() {
    let db = test_db().await;
    create_test_provider(&db, "find-p1").await;
    create_test_provider(&db, "find-p2").await;
    db.add_provider_model("find-p1", "gpt-4").await.unwrap();
    db.add_provider_model("find-p2", "gpt-4").await.unwrap();
    let providers = db.find_providers_with_model("gpt-4").await.unwrap();
    assert_eq!(providers.len(), 2);
}

#[tokio::test]
async fn test_find_providers_with_model_none() {
    let db = test_db().await;
    let providers = db.find_providers_with_model("nonexistent").await.unwrap();
    assert!(providers.is_empty());
}

#[tokio::test]
async fn test_is_model_allowed_for_provider() {
    let db = test_db().await;
    create_test_provider(&db, "allow-prov").await;
    db.add_provider_model("allow-prov", "gpt-4").await.unwrap();
    assert!(db.is_model_allowed_for_provider("allow-prov", "gpt-4").await.unwrap());
    assert!(!db.is_model_allowed_for_provider("allow-prov", "claude-3").await.unwrap());
}

#[tokio::test]
async fn test_cleanup_mappings_for_provider_model() {
    let db = test_db().await;
    create_test_provider(&db, "clean-prov").await;
    db.add_provider_model("clean-prov", "gpt-4").await.unwrap();
    db.create_model("clean-model", "Clean Model", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("clean-model", "clean-prov", "gpt-4", 1, 1.0).await.unwrap();
    db.cleanup_mappings_for_provider_model("clean-prov", "gpt-4").await.unwrap();
    let mappings = db.list_model_mappings("clean-model").await.unwrap();
    assert!(mappings.is_empty());
}

// ==================== Request Log Detail Tests ====================

#[tokio::test]
async fn test_get_request_log() {
    let db = test_db().await;
    create_test_api_key(&db, "log-key").await;
    create_test_provider(&db, "log-prov").await;
    let id = db.insert_request_log(
        "log-key", "log-prov", Some("gpt-4"), "/v1/chat/completions", "POST",
        None, Some("{\"model\":\"gpt-4\"}"), Some(200), None, Some("{\"choices\":[]}"),
        100, 200, 300, Some(1500), false, false, None,
    ).await.unwrap();
    let log = db.get_request_log(id).await.unwrap().unwrap();
    assert_eq!(log.id, id);
    assert_eq!(log.api_key_id, "log-key");
    assert_eq!(log.provider_id, "log-prov");
}

#[tokio::test]
async fn test_get_request_log_not_found() {
    let db = test_db().await;
    assert!(db.get_request_log(99999).await.unwrap().is_none());
}

#[tokio::test]
async fn test_delete_request_log() {
    let db = test_db().await;
    create_test_api_key(&db, "del-log-key").await;
    create_test_provider(&db, "del-log-prov").await;
    let id = db.insert_request_log(
        "del-log-key", "del-log-prov", None, "/v1/test", "GET",
        None, None, Some(200), None, None, 0, 0, 0, None, false, false, None,
    ).await.unwrap();
    assert!(db.delete_request_log(id).await.unwrap());
    assert!(db.get_request_log(id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_delete_request_log_not_found() {
    let db = test_db().await;
    assert!(!db.delete_request_log(99999).await.unwrap());
}

#[tokio::test]
async fn test_delete_request_logs_by_provider() {
    let db = test_db().await;
    create_test_api_key(&db, "dp-key").await;
    create_test_provider(&db, "dp-prov1").await;
    create_test_provider(&db, "dp-prov2").await;
    db.insert_request_log("dp-key", "dp-prov1", None, "/v1/test", "GET", None, None, Some(200), None, None, 0, 0, 0, None, false, false, None).await.unwrap();
    db.insert_request_log("dp-key", "dp-prov1", None, "/v1/test2", "GET", None, None, Some(200), None, None, 0, 0, 0, None, false, false, None).await.unwrap();
    db.insert_request_log("dp-key", "dp-prov2", None, "/v1/test3", "GET", None, None, Some(200), None, None, 0, 0, 0, None, false, false, None).await.unwrap();
    let count = db.delete_request_logs_by_provider("dp-prov1").await.unwrap();
    assert_eq!(count, 2);
    assert_eq!(db.count_request_logs(None, None, None, None, None, None, None).await.unwrap(), 1);
}

#[tokio::test]
async fn test_delete_request_logs_by_api_key() {
    let db = test_db().await;
    create_test_api_key(&db, "dk-key1").await;
    create_test_api_key(&db, "dk-key2").await;
    create_test_provider(&db, "dk-prov").await;
    db.insert_request_log("dk-key1", "dk-prov", None, "/v1/test", "GET", None, None, Some(200), None, None, 0, 0, 0, None, false, false, None).await.unwrap();
    db.insert_request_log("dk-key2", "dk-prov", None, "/v1/test2", "GET", None, None, Some(200), None, None, 0, 0, 0, None, false, false, None).await.unwrap();
    let count = db.delete_request_logs_by_api_key("dk-key1").await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(db.count_request_logs(None, None, None, None, None, None, None).await.unwrap(), 1);
}

#[tokio::test]
async fn test_delete_all_request_logs() {
    let db = test_db().await;
    create_test_api_key(&db, "da-key").await;
    create_test_provider(&db, "da-prov").await;
    for i in 0..5 {
        db.insert_request_log("da-key", "da-prov", None, &format!("/v1/test{}", i), "GET", None, None, Some(200), None, None, 0, 0, 0, None, false, false, None).await.unwrap();
    }
    let count = db.delete_all_request_logs().await.unwrap();
    assert_eq!(count, 5);
    assert_eq!(db.count_request_logs(None, None, None, None, None, None, None).await.unwrap(), 0);
}

#[tokio::test]
async fn test_delete_request_logs_filtered() {
    let db = test_db().await;
    create_test_api_key(&db, "df-key").await;
    create_test_provider(&db, "df-prov").await;
    db.insert_request_log("df-key", "df-prov", Some("gpt-4"), "/v1/test", "POST", None, Some("{\"model\":\"gpt-4\",\"messages\":\"hello\"}"), Some(200), None, Some("{\"response\":\"world\"}"), 10, 20, 30, None, false, false, None).await.unwrap();
    db.insert_request_log("df-key", "df-prov", Some("claude-3"), "/v1/test2", "POST", None, Some("{\"model\":\"claude-3\"}"), Some(200), None, None, 0, 0, 0, None, false, false, None).await.unwrap();
    let count = db.delete_request_logs_filtered(Some("df-key"), None, None, None, Some("hello"), None, None).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_get_top_provider_by_usage() {
    let db = test_db().await;
    create_test_provider(&db, "top-p1").await;
    create_test_provider(&db, "top-p2").await;
    db.insert_token_rate_snapshot(Some("top-p1"), 500.0, 250, 250, 5, 10.0).await.unwrap();
    db.insert_token_rate_snapshot(Some("top-p2"), 100.0, 50, 50, 2, 5.0).await.unwrap();
    let top = db.get_top_provider_by_usage().await.unwrap();
    assert_eq!(top, Some("top-p1".to_string()));
}

#[tokio::test]
async fn test_get_top_provider_by_usage_empty() {
    let db = test_db().await;
    assert!(db.get_top_provider_by_usage().await.unwrap().is_none());
}

// ==================== Model CRUD Tests ====================

#[tokio::test]
async fn test_get_model() {
    let db = test_db().await;
    db.create_model("get-model", "GPT-4", Some("OpenAI GPT-4"), "chat", 1, None).await.unwrap();
    let model = db.get_model("get-model").await.unwrap().unwrap();
    assert_eq!(model.id, "get-model");
    assert_eq!(model.name, "GPT-4");
    assert_eq!(model.description, Some("OpenAI GPT-4".to_string()));
}

#[tokio::test]
async fn test_get_model_not_found() {
    let db = test_db().await;
    assert!(db.get_model("nonexistent").await.unwrap().is_none());
}

#[tokio::test]
async fn test_list_models() {
    let db = test_db().await;
    db.create_model("lm-1", "Model 1", None, "chat", 1, None).await.unwrap();
    db.create_model("lm-2", "Model 2", None, "chat", 2, None).await.unwrap();
    let models = db.list_models().await.unwrap();
    assert_eq!(models.len(), 2);
}

#[tokio::test]
async fn test_update_model() {
    let db = test_db().await;
    db.create_model("upd-model", "Original", None, "chat", 1, None).await.unwrap();
    db.update_model("upd-model", "Updated", Some("New desc"), "chat", true, 2, None).await.unwrap();
    let model = db.get_model("upd-model").await.unwrap().unwrap();
    assert_eq!(model.name, "Updated");
    assert_eq!(model.description, Some("New desc".to_string()));
    assert_eq!(model.priority, 2);
}

#[tokio::test]
async fn test_update_model_deactivate() {
    let db = test_db().await;
    db.create_model("deact-model", "Model", None, "chat", 1, None).await.unwrap();
    db.update_model("deact-model", "Model", None, "chat", false, 1, None).await.unwrap();
    let model = db.get_model("deact-model").await.unwrap().unwrap();
    assert!(!model.is_active);
}

#[tokio::test]
async fn test_delete_model() {
    let db = test_db().await;
    db.create_model("del-model", "Model", None, "chat", 1, None).await.unwrap();
    assert!(db.delete_model("del-model").await.unwrap());
    assert!(db.get_model("del-model").await.unwrap().is_none());
}

#[tokio::test]
async fn test_delete_model_not_found() {
    let db = test_db().await;
    assert!(!db.delete_model("nonexistent").await.unwrap());
}

#[tokio::test]
async fn test_remove_model_mapping() {
    let db = test_db().await;
    create_test_provider(&db, "rm-prov").await;
    db.create_model("rm-model", "Model", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("rm-model", "rm-prov", "gpt-4", 1, 1.0).await.unwrap();
    assert!(db.remove_model_mapping("rm-model", "rm-prov", "gpt-4").await.unwrap());
    assert!(db.list_model_mappings("rm-model").await.unwrap().is_empty());
}

#[tokio::test]
async fn test_remove_model_mapping_not_found() {
    let db = test_db().await;
    assert!(!db.remove_model_mapping("nonexistent", "nonexistent", "nonexistent").await.unwrap());
}

#[tokio::test]
async fn test_list_active_model_mappings() {
    let db = test_db().await;
    create_test_provider(&db, "lam-prov").await;
    db.create_model("lam-model", "Model", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("lam-model", "lam-prov", "gpt-4", 1, 1.0).await.unwrap();
    let mappings = db.list_active_model_mappings("lam-model").await.unwrap();
    assert_eq!(mappings.len(), 1);
}

#[tokio::test]
async fn test_update_model_mapping() {
    let db = test_db().await;
    create_test_provider(&db, "um-prov").await;
    db.create_model("um-model", "Model", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("um-model", "um-prov", "gpt-4", 1, 1.0).await.unwrap();
    db.update_model_mapping("um-model", "um-prov", "gpt-4", "gpt-4o", true, 2, 0.5).await.unwrap();
    let mappings = db.list_model_mappings("um-model").await.unwrap();
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].provider_model_id, "gpt-4o");
    assert_eq!(mappings[0].weight, 2);
}

#[tokio::test]
async fn test_get_model_with_mappings() {
    let db = test_db().await;
    create_test_provider(&db, "gwm-prov").await;
    db.create_model("gwm-model", "Model", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("gwm-model", "gwm-prov", "gpt-4", 1, 1.0).await.unwrap();
    let result = db.get_model_with_mappings("gwm-model").await.unwrap().unwrap();
    assert_eq!(result.model.id, "gwm-model");
    assert_eq!(result.mappings.len(), 1);
}

#[tokio::test]
async fn test_get_model_with_mappings_not_found() {
    let db = test_db().await;
    assert!(db.get_model_with_mappings("nonexistent").await.unwrap().is_none());
}

#[tokio::test]
async fn test_list_models_with_mappings() {
    let db = test_db().await;
    create_test_provider(&db, "lmw-prov").await;
    db.create_model("lmw-1", "Model 1", None, "chat", 1, None).await.unwrap();
    db.create_model("lmw-2", "Model 2", None, "chat", 2, None).await.unwrap();
    db.add_model_mapping("lmw-1", "lmw-prov", "gpt-4", 1, 1.0).await.unwrap();
    let models = db.list_models_with_mappings().await.unwrap();
    assert_eq!(models.len(), 2);
}

#[tokio::test]
async fn test_get_model_by_external_id() {
    let db = test_db().await;
    db.create_model("ext-model", "External Model", None, "chat", 1, None).await.unwrap();
    // The model ID serves as the external ID
    let result = db.get_model_by_external_id("ext-model").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "ext-model");
}

#[tokio::test]
async fn test_get_model_by_external_id_not_found() {
    let db = test_db().await;
    assert!(db.get_model_by_external_id("nonexistent").await.unwrap().is_none());
}

// ==================== Quota Extended Tests ====================

#[tokio::test]
async fn test_list_all_quotas() {
    let db = test_db().await;
    create_test_provider(&db, "laq-p1").await;
    create_test_provider(&db, "laq-p2").await;
    db.set_provider_quota("laq-p1", "fixed:5h", "fixed", "5h", None, None, None, 1000, true).await.unwrap();
    db.set_provider_quota("laq-p2", "fixed:30d", "fixed", "30d", None, None, None, 5000, true).await.unwrap();
    let quotas = db.list_all_quotas().await.unwrap();
    assert_eq!(quotas.len(), 2);
}

#[tokio::test]
async fn test_delete_provider_quota() {
    let db = test_db().await;
    create_test_provider(&db, "dpq-prov").await;
    db.set_provider_quota("dpq-prov", "fixed:5h", "fixed", "5h", None, None, None, 1000, true).await.unwrap();
    assert!(db.delete_provider_quota("dpq-prov", "fixed:5h").await.unwrap());
    let quotas = db.list_provider_quotas("dpq-prov").await.unwrap();
    assert!(quotas.is_empty());
}

#[tokio::test]
async fn test_delete_provider_quota_not_found() {
    let db = test_db().await;
    assert!(!db.delete_provider_quota("nonexistent", "fixed:5h").await.unwrap());
}

#[tokio::test]
async fn test_list_provider_calibrations() {
    let db = test_db().await;
    create_test_provider(&db, "lpc-prov").await;
    db.set_provider_quota("lpc-prov", "fixed:5h", "fixed", "5h", None, None, None, 1000, true).await.unwrap();
    db.set_quota_calibration("lpc-prov", "fixed:5h", 50, None, None, Some("test note")).await.unwrap();
    let calibrations = db.list_provider_calibrations("lpc-prov").await.unwrap();
    assert_eq!(calibrations.len(), 1);
}

#[tokio::test]
async fn test_list_provider_calibrations_empty() {
    let db = test_db().await;
    create_test_provider(&db, "lpc-empty").await;
    let calibrations = db.list_provider_calibrations("lpc-empty").await.unwrap();
    assert!(calibrations.is_empty());
}

#[tokio::test]
async fn test_count_provider_requests() {
    let db = test_db().await;
    create_test_api_key(&db, "cpr-key").await;
    create_test_provider(&db, "cpr-prov").await;
    for _ in 0..3 {
        db.insert_request_log("cpr-key", "cpr-prov", None, "/v1/test", "POST", None, None, Some(200), None, None, 0, 0, 0, None, false, false, None).await.unwrap();
    }
    let count = db.count_provider_requests("cpr-prov", "2000-01-01T00:00:00Z", "2099-12-31T23:59:59Z").await.unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_count_provider_requests_no_match() {
    let db = test_db().await;
    create_test_provider(&db, "cpr-nomatch").await;
    let count = db.count_provider_requests("cpr-nomatch", "2000-01-01T00:00:00Z", "2000-01-02T00:00:00Z").await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_get_all_quota_usage() {
    let db = test_db().await;
    create_test_provider(&db, "gaqu-p1").await;
    create_test_provider(&db, "gaqu-p2").await;
    db.set_provider_quota("gaqu-p1", "fixed:5h", "fixed", "5h", None, None, None, 1000, true).await.unwrap();
    db.set_provider_quota("gaqu-p2", "sliding:5h", "sliding", "5h", None, None, None, 2000, true).await.unwrap();
    let usage = db.get_all_quota_usage().await.unwrap();
    assert_eq!(usage.len(), 2);
}

#[tokio::test]
async fn test_get_provider_quota_usage() {
    let db = test_db().await;
    create_test_provider(&db, "gpqu-prov").await;
    db.set_provider_quota("gpqu-prov", "fixed:5h", "fixed", "5h", None, None, None, 1000, true).await.unwrap();
    db.set_provider_quota("gpqu-prov", "fixed:30d", "fixed", "30d", None, None, None, 5000, true).await.unwrap();
    let usage = db.get_provider_quota_usage("gpqu-prov").await.unwrap();
    assert_eq!(usage.len(), 2);
}

#[tokio::test]
async fn test_get_provider_quota_usage_empty() {
    let db = test_db().await;
    create_test_provider(&db, "gpqu-empty").await;
    let usage = db.get_provider_quota_usage("gpqu-empty").await.unwrap();
    assert!(usage.is_empty());
}

#[tokio::test]
async fn test_same_provider_model_multiple_unified_models() {
    // Test: same provider's provider_model_id can be mapped to multiple unified models
    // e.g., provider "openai" with model "gpt-4o" can map to both "my-gpt4o" and "company-gpt4o"
    let db = test_db().await;
    create_test_provider(&db, "multi-prov").await;
    db.create_model("unified-a", "Unified A", None, "chat", 1, None).await.unwrap();
    db.create_model("unified-b", "Unified B", None, "chat", 2, None).await.unwrap();

    // Add mapping: unified-a -> multi-prov/gpt-4o
    db.add_model_mapping("unified-a", "multi-prov", "gpt-4o", 1, 1.0).await.unwrap();
    // Add mapping: unified-b -> multi-prov/gpt-4o (same provider_model_id, different unified model)
    db.add_model_mapping("unified-b", "multi-prov", "gpt-4o", 2, 0.5).await.unwrap();

    let mappings_a = db.list_model_mappings("unified-a").await.unwrap();
    assert_eq!(mappings_a.len(), 1);
    assert_eq!(mappings_a[0].provider_model_id, "gpt-4o");

    let mappings_b = db.list_model_mappings("unified-b").await.unwrap();
    assert_eq!(mappings_b.len(), 1);
    assert_eq!(mappings_b[0].provider_model_id, "gpt-4o");

    // Remove mapping from unified-a should not affect unified-b
    db.remove_model_mapping("unified-a", "multi-prov", "gpt-4o").await.unwrap();
    let mappings_b_after = db.list_model_mappings("unified-b").await.unwrap();
    assert_eq!(mappings_b_after.len(), 1);
}

#[tokio::test]
async fn test_same_unified_model_multiple_provider_models() {
    // Test: same unified model can have multiple provider_model_ids from the same provider
    // e.g., unified model "my-gpt4" can map to both "gpt-4" and "gpt-4o" from provider "openai"
    let db = test_db().await;
    create_test_provider(&db, "mpm-prov").await;
    db.create_model("mpm-model", "Multi PM Model", None, "chat", 1, None).await.unwrap();

    db.add_model_mapping("mpm-model", "mpm-prov", "gpt-4", 1, 1.0).await.unwrap();
    db.add_model_mapping("mpm-model", "mpm-prov", "gpt-4o", 2, 0.5).await.unwrap();

    let mappings = db.list_model_mappings("mpm-model").await.unwrap();
    assert_eq!(mappings.len(), 2);

    // Remove one mapping
    db.remove_model_mapping("mpm-model", "mpm-prov", "gpt-4").await.unwrap();
    let after = db.list_model_mappings("mpm-model").await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].provider_model_id, "gpt-4o");
}

// ==================== Request log: error/throttled/streaming fields ====================

#[tokio::test]
async fn test_insert_log_with_error_message() {
    let db = test_db().await;
    create_test_provider(&db, "err-log-prov").await;
    db.create_api_key("err-log-key", "Key", "hash", "elk", None).await.unwrap();
    let id = db.insert_request_log(
        "err-log-key", "err-log-prov", Some("gpt-4"), "/v1/chat", "POST",
        None, Some("request"), Some(429), None, Some("rate limit response"),
        0, 0, 0, Some(100), false, true, Some("Rate limit exceeded"),
    ).await.unwrap();
    let log = db.get_request_log(id).await.unwrap().unwrap();
    assert_eq!(log.response_status, Some(429));
    assert_eq!(log.is_throttled, true);
    assert_eq!(log.error_message, Some("Rate limit exceeded".to_string()));
}

#[tokio::test]
async fn test_insert_log_streaming() {
    let db = test_db().await;
    create_test_provider(&db, "str-log-prov").await;
    db.create_api_key("str-log-key", "Key", "hash", "slk", None).await.unwrap();
    let id = db.insert_request_log(
        "str-log-key", "str-log-prov", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        10, 20, 30, Some(1500), true, false, None,
    ).await.unwrap();
    let log = db.get_request_log(id).await.unwrap().unwrap();
    assert_eq!(log.is_streaming, true);
    assert_eq!(log.duration_ms, Some(1500));
}

#[tokio::test]
async fn test_insert_log_with_null_model() {
    let db = test_db().await;
    create_test_provider(&db, "null-model-prov").await;
    db.create_api_key("null-model-key", "Key", "hash", "nmk", None).await.unwrap();
    let id = db.insert_request_log(
        "null-model-key", "null-model-prov", None, "/v1/chat", "POST",
        None, None, Some(200), None, None,
        10, 20, 30, None, false, false, None,
    ).await.unwrap();
    let log = db.get_request_log(id).await.unwrap().unwrap();
    assert!(log.model.is_none());
    assert!(log.duration_ms.is_none());
}

#[tokio::test]
async fn test_insert_log_server_error() {
    let db = test_db().await;
    create_test_provider(&db, "srv-err-prov").await;
    db.create_api_key("srv-err-key", "Key", "hash", "sek", None).await.unwrap();
    let id = db.insert_request_log(
        "srv-err-key", "srv-err-prov", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(503), None, None,
        0, 0, 0, Some(5000), false, false, Some("Service Unavailable"),
    ).await.unwrap();
    let log = db.get_request_log(id).await.unwrap().unwrap();
    assert_eq!(log.response_status, Some(503));
    assert_eq!(log.error_message, Some("Service Unavailable".to_string()));
}

// ==================== Provider health stats ====================

#[tokio::test]
async fn test_provider_health_stats_basic() {
    let db = test_db().await;
    create_test_provider(&db, "health-prov").await;
    db.create_api_key("health-key", "Key", "hash", "hk", None).await.unwrap();
    db.insert_request_log("health-key", "health-prov", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("health-key", "health-prov", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(500), None, None, 0, 0, 0, Some(200), false, false, Some("server error")).await.unwrap();

    let stats = db.get_provider_health_stats().await.unwrap();
    let prov_stat = stats.iter().find(|s| s.provider_id == "health-prov").unwrap();
    assert_eq!(prov_stat.request_count_24h, 2);
    assert_eq!(prov_stat.error_count_24h, 1); // only the 500 log has error_message
    assert!(prov_stat.avg_duration_ms > 0.0);
}

#[tokio::test]
async fn test_provider_health_stats_no_logs() {
    let db = test_db().await;
    create_test_provider(&db, "health-empty").await;
    let stats = db.get_provider_health_stats().await.unwrap();
    let found = stats.iter().find(|s| s.provider_id == "health-empty");
    assert!(found.is_none(), "provider with no logs should not appear in health stats");
}

// ==================== Stats by API key with time filter ====================

#[tokio::test]
async fn test_stats_by_api_key_with_time_filter() {
    let db = test_db().await;
    create_test_provider(&db, "stats-tf-prov").await;
    db.create_api_key("stats-tf-key", "Key", "hash", "stk", None).await.unwrap();
    db.insert_request_log("stats-tf-key", "stats-tf-prov", Some("gpt-4"), "/v1/chat", "POST",
        None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();

    // Wide range should include the log
    let stats = db.get_stats_by_api_key(10, Some("2020-01-01"), Some("2099-12-31")).await.unwrap();
    assert!(!stats.is_empty());
    assert_eq!(stats[0].api_key_id, "stats-tf-key");
    assert_eq!(stats[0].request_count, 1);
    assert_eq!(stats[0].total_tokens, 30);

    // Future range should exclude the log
    let future = db.get_stats_by_api_key(10, Some("2100-01-01"), Some("2101-12-31")).await.unwrap();
    assert!(future.is_empty());
}

#[tokio::test]
async fn test_stats_by_api_key_error_count() {
    let db = test_db().await;
    create_test_provider(&db, "stats-te-prov").await;
    db.create_api_key("stats-te-key", "Key", "hash", "stek", None).await.unwrap();
    db.insert_request_log("stats-te-key", "stats-te-prov", None, "/v1/chat", "POST",
        None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("stats-te-key", "stats-te-prov", None, "/v1/chat", "POST",
        None, None, Some(500), None, None, 0, 0, 0, Some(200), false, false, Some("error")).await.unwrap();

    let stats = db.get_stats_by_api_key(10, None, None).await.unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].error_count, 1); // only the 500 log has error_message
    assert_eq!(stats[0].request_count, 2);
    assert_eq!(stats[0].total_tokens, 30); // only the 200 log has tokens
}

// ==================== Quota calibration edge cases ====================

#[tokio::test]
async fn test_quota_calibration_create_and_get() {
    let db = test_db().await;
    create_test_provider(&db, "cal-prov").await;
    db.set_provider_quota("cal-prov", "daily", "fixed", "1d", None, None, Some("Asia/Shanghai"), 1000, true).await.unwrap();

    // Set calibration
    db.set_quota_calibration("cal-prov", "daily", 500, Some("2026-01-15T08:00:00+08:00"), None, None).await.unwrap();

    let cal = db.get_quota_calibration("cal-prov", "daily").await.unwrap().unwrap();
    assert_eq!(cal.calibration_offset, 500);
    assert!(cal.calibration_window_start.is_some()); // window_start was provided
}

#[tokio::test]
async fn test_quota_calibration_update_overwrite() {
    let db = test_db().await;
    create_test_provider(&db, "cal-upd-prov").await;
    db.set_provider_quota("cal-upd-prov", "daily", "fixed", "1d", None, None, None, 1000, true).await.unwrap();

    db.set_quota_calibration("cal-upd-prov", "daily", 500, Some("2026-01-15T08:00:00+08:00"), None, None).await.unwrap();
    db.set_quota_calibration("cal-upd-prov", "daily", 800, Some("2026-01-16T10:00:00+08:00"), None, None).await.unwrap();

    let cal = db.get_quota_calibration("cal-upd-prov", "daily").await.unwrap().unwrap();
    assert_eq!(cal.calibration_offset, 800); // overwritten
    assert!(cal.calibration_window_start.is_some());
}

#[tokio::test]
async fn test_quota_calibration_not_found() {
    let db = test_db().await;
    let cal = db.get_quota_calibration("nonexistent", "daily").await.unwrap();
    assert!(cal.is_none());
}

#[tokio::test]
async fn test_list_provider_calibrations_multi() {
    let db = test_db().await;
    create_test_provider(&db, "cal-list-prov").await;
    db.set_provider_quota("cal-list-prov", "daily", "fixed", "1d", None, None, None, 1000, true).await.unwrap();
    db.set_provider_quota("cal-list-prov", "monthly", "fixed", "1M", None, None, None, 50000, true).await.unwrap();

    db.set_quota_calibration("cal-list-prov", "daily", 100, Some("2026-01-15T08:00:00+08:00"), None, None).await.unwrap();
    db.set_quota_calibration("cal-list-prov", "monthly", 5000, Some("2026-01-15T08:00:00+08:00"), None, None).await.unwrap();

    let cals = db.list_provider_calibrations("cal-list-prov").await.unwrap();
    assert_eq!(cals.len(), 2);
}
