use llm_gateway::db::Database;

async fn test_db() -> Database {
    Database::new_in_memory().await.unwrap()
}

async fn create_test_provider(db: &Database, id: &str) {
    db.create_provider(
        id, id, "https://api.example.com/v1", "openai", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false, false,
        "", "",
    ).await.unwrap();
}

// ==================== API key: deactivate, regenerate, update, batch ====================

#[tokio::test]
async fn db_api_key_deactivate() {
    let db = test_db().await;
    db.create_api_key("deact-key", "Deact", "lgk-deact", "lgk-deact-xxx", None).await.unwrap();
    let deactivated = db.deactivate_api_key("deact-key").await.unwrap();
    assert!(deactivated);
    let key = db.get_api_key_by_id("deact-key").await.unwrap().unwrap();
    assert!(!key.is_active);
}

#[tokio::test]
async fn db_api_key_deactivate_not_found() {
    let db = test_db().await;
    let deactivated = db.deactivate_api_key("nonexistent").await.unwrap();
    assert!(!deactivated);
}

#[tokio::test]
async fn db_api_key_regenerate() {
    let db = test_db().await;
    db.create_api_key("regen-key", "Regen", "lgk-regen", "lgk-regen-xxx", None).await.unwrap();
    let regenerated = db.regenerate_api_key("regen-key", "lgk-regen-new", "lgk-regen-new-xxx").await.unwrap();
    assert!(regenerated);
    let key = db.get_api_key_by_key("lgk-regen-new").await.unwrap();
    assert!(key.is_some());
    let old = db.get_api_key_by_key("lgk-regen").await.unwrap();
    assert!(old.is_none(), "Old key should no longer be valid");
}

#[tokio::test]
async fn db_api_key_regenerate_not_found() {
    let db = test_db().await;
    let regenerated = db.regenerate_api_key("nonexistent", "lgk-new", "lgk-new-xxx").await.unwrap();
    assert!(!regenerated);
}

#[tokio::test]
async fn db_api_key_update_name() {
    let db = test_db().await;
    db.create_api_key("upd-ak", "Original Name", "lgk-upd-ak", "lgk-upd-ak-xxx", None).await.unwrap();
    db.update_api_key("upd-ak", "Updated Name", None, true).await.unwrap();
    let key = db.get_api_key_by_id("upd-ak").await.unwrap().unwrap();
    assert_eq!(key.name, "Updated Name");
}

#[tokio::test]
async fn db_api_key_update_allowed_providers() {
    let db = test_db().await;
    db.create_api_key("upd-ap", "AP Key", "lgk-upd-ap", "lgk-upd-ap-xxx", None).await.unwrap();
    let allowed = serde_json::json!(["prov-a", "prov-b"]).to_string();
    db.update_api_key("upd-ap", "AP Key", Some(&allowed), true).await.unwrap();
    let key = db.get_api_key_by_id("upd-ap").await.unwrap().unwrap();
    assert!(key.allowed_providers.is_some());
}

#[tokio::test]
async fn db_api_key_batch_activate() {
    let db = test_db().await;
    db.create_api_key("batch-1", "Batch1", "lgk-batch-1", "lgk-batch-1-xxx", None).await.unwrap();
    db.create_api_key("batch-2", "Batch2", "lgk-batch-2", "lgk-batch-2-xxx", None).await.unwrap();
    db.deactivate_api_key("batch-1").await.unwrap();
    db.deactivate_api_key("batch-2").await.unwrap();

    let count = db.batch_set_api_key_active_status(&["batch-1".to_string(), "batch-2".to_string()], true).await.unwrap();
    assert_eq!(count, 2);
    let key1 = db.get_api_key_by_id("batch-1").await.unwrap().unwrap();
    assert!(key1.is_active);
}

#[tokio::test]
async fn db_api_key_batch_deactivate() {
    let db = test_db().await;
    db.create_api_key("bdeact-1", "BDeact1", "lgk-bdeact-1", "lgk-bdeact-1-xxx", None).await.unwrap();
    db.create_api_key("bdeact-2", "BDeact2", "lgk-bdeact-2", "lgk-bdeact-2-xxx", None).await.unwrap();

    let count = db.batch_set_api_key_active_status(&["bdeact-1".to_string(), "bdeact-2".to_string()], false).await.unwrap();
    assert_eq!(count, 2);
    let key1 = db.get_api_key_by_id("bdeact-1").await.unwrap().unwrap();
    assert!(!key1.is_active);
}

#[tokio::test]
async fn db_api_key_delete() {
    let db = test_db().await;
    db.create_api_key("del-ak", "Del Key", "lgk-del-ak", "lgk-del-ak-xxx", None).await.unwrap();
    let deleted = db.delete_api_key("del-ak").await.unwrap();
    assert!(deleted);
    let key = db.get_api_key_by_id("del-ak").await.unwrap();
    assert!(key.is_none());
}

#[tokio::test]
async fn db_api_key_delete_not_found() {
    let db = test_db().await;
    let deleted = db.delete_api_key("nonexistent").await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn db_api_key_list() {
    let db = test_db().await;
    db.create_api_key("list-1", "Key1", "lgk-list-1", "lgk-list-1-xxx", None).await.unwrap();
    db.create_api_key("list-2", "Key2", "lgk-list-2", "lgk-list-2-xxx", None).await.unwrap();
    let keys = db.list_api_keys().await.unwrap();
    assert_eq!(keys.len(), 2);
}

// ==================== Provider: chart_color, cookies, batch, models ====================

#[tokio::test]
async fn db_provider_chart_color() {
    let db = test_db().await;
    create_test_provider(&db, "color-prov").await;
    db.update_provider_chart_color("color-prov", Some("#FF5733")).await.unwrap();
    let provider = db.get_provider("color-prov").await.unwrap().unwrap();
    assert_eq!(provider.chart_color, Some("#FF5733".to_string()));
}

#[tokio::test]
async fn db_provider_chart_color_remove() {
    let db = test_db().await;
    create_test_provider(&db, "no-color-prov").await;
    db.update_provider_chart_color("no-color-prov", Some("#FF5733")).await.unwrap();
    db.update_provider_chart_color("no-color-prov", None).await.unwrap();
    let provider = db.get_provider("no-color-prov").await.unwrap().unwrap();
    assert!(provider.chart_color.is_none());
}

#[tokio::test]
async fn db_provider_update_cookies() {
    let db = test_db().await;
    create_test_provider(&db, "cookie-prov").await;
    db.update_provider_cookies("cookie-prov", "session=abc123; token=xyz").await.unwrap();
    let provider = db.get_provider("cookie-prov").await.unwrap().unwrap();
    assert_eq!(provider.token_cookies, Some("session=abc123; token=xyz".to_string()));
}

#[tokio::test]
async fn db_provider_batch_activate() {
    let db = test_db().await;
    create_test_provider(&db, "bp-1").await;
    create_test_provider(&db, "bp-2").await;
    db.deactivate_provider("bp-1").await.unwrap();
    db.deactivate_provider("bp-2").await.unwrap();

    let count = db.batch_set_provider_active_status(&["bp-1".to_string(), "bp-2".to_string()], true).await.unwrap();
    assert_eq!(count, 2);
    let prov = db.get_provider("bp-1").await.unwrap().unwrap();
    assert!(prov.is_active);
}

#[tokio::test]
async fn db_provider_batch_deactivate() {
    let db = test_db().await;
    create_test_provider(&db, "bd-1").await;
    create_test_provider(&db, "bd-2").await;

    let count = db.batch_set_provider_active_status(&["bd-1".to_string(), "bd-2".to_string()], false).await.unwrap();
    assert_eq!(count, 2);
    let prov = db.get_provider("bd-1").await.unwrap().unwrap();
    assert!(!prov.is_active);
}

#[tokio::test]
async fn db_provider_list_active() {
    let db = test_db().await;
    create_test_provider(&db, "la-1").await;
    create_test_provider(&db, "la-2").await;
    db.deactivate_provider("la-2").await.unwrap();

    let active = db.list_active_providers().await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "la-1");
}

#[tokio::test]
async fn db_provider_delete_not_found() {
    let db = test_db().await;
    let deleted = db.delete_provider("nonexistent").await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn db_provider_model_get() {
    let db = test_db().await;
    create_test_provider(&db, "pm-get-prov").await;
    db.add_provider_model("pm-get-prov", "gpt-4").await.unwrap();
    let model = db.get_provider_model("pm-get-prov", "gpt-4").await.unwrap();
    assert!(model.is_some());
}

#[tokio::test]
async fn db_provider_model_get_not_found() {
    let db = test_db().await;
    create_test_provider(&db, "pm-nf-prov").await;
    let model = db.get_provider_model("pm-nf-prov", "nonexistent").await.unwrap();
    assert!(model.is_none());
}

#[tokio::test]
async fn db_provider_model_remove() {
    let db = test_db().await;
    create_test_provider(&db, "pm-rem-prov").await;
    db.add_provider_model("pm-rem-prov", "gpt-4").await.unwrap();
    let removed = db.remove_provider_model("pm-rem-prov", "gpt-4").await.unwrap();
    assert!(removed);
    let models = db.list_provider_models("pm-rem-prov").await.unwrap();
    assert!(models.is_empty());
}

#[tokio::test]
async fn db_provider_model_remove_not_found() {
    let db = test_db().await;
    create_test_provider(&db, "pm-rnf-prov").await;
    let removed = db.remove_provider_model("pm-rnf-prov", "nonexistent").await.unwrap();
    assert!(!removed);
}

#[tokio::test]
async fn db_provider_list_active_models() {
    let db = test_db().await;
    create_test_provider(&db, "lam-prov").await;
    db.add_provider_model("lam-prov", "gpt-4").await.unwrap();
    db.add_provider_model("lam-prov", "gpt-4o").await.unwrap();
    let models = db.list_active_provider_models("lam-prov").await.unwrap();
    assert_eq!(models.len(), 2);
}

#[tokio::test]
async fn db_provider_model_test_status() {
    let db = test_db().await;
    create_test_provider(&db, "test-prov").await;
    db.add_provider_model("test-prov", "test-model").await.unwrap();
    db.update_provider_model_test_status("test-prov", "test-model", "success", Some("All good"), true).await.unwrap();
    let model = db.get_provider_model("test-prov", "test-model").await.unwrap();
    assert!(model.is_some());
    assert_eq!(model.unwrap().last_test_status, Some("success".to_string()));
}

// ==================== Model: with_mappings, list_with_mappings ====================

#[tokio::test]
async fn db_model_with_mappings() {
    let db = test_db().await;
    create_test_provider(&db, "wm-prov").await;
    db.create_model("wm-model", "WM Model", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("wm-model", "wm-prov", "gpt-4", 1, 1.0).await.unwrap();

    let result = db.get_model_with_mappings("wm-model").await.unwrap();
    assert!(result.is_some());
    let mwm = result.unwrap();
    assert_eq!(mwm.model.id, "wm-model");
    assert_eq!(mwm.mappings.len(), 1);
}

#[tokio::test]
async fn db_model_with_mappings_not_found() {
    let db = test_db().await;
    let result = db.get_model_with_mappings("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn db_list_models_with_mappings() {
    let db = test_db().await;
    create_test_provider(&db, "lwm-prov").await;
    db.create_model("lwm-1", "M1", None, "chat", 1, None).await.unwrap();
    db.create_model("lwm-2", "M2", None, "chat", 2, None).await.unwrap();
    db.add_model_mapping("lwm-1", "lwm-prov", "gpt-4", 1, 1.0).await.unwrap();

    let list = db.list_models_with_mappings().await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn db_model_delete_not_found() {
    let db = test_db().await;
    let deleted = db.delete_model("nonexistent").await.unwrap();
    assert!(!deleted);
}

// ==================== Stats: get_stats ====================

#[tokio::test]
async fn db_stats_with_data() {
    let db = test_db().await;
    create_test_provider(&db, "stats-prov").await;
    db.insert_request_log(
        "key1", "stats-prov", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(50), false, false, None
    ).await.unwrap();

    let stats = db.get_stats(None, None, None, None).await.unwrap();
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.total_tokens, 300);
}

#[tokio::test]
async fn db_stats_empty() {
    let db = test_db().await;
    let stats = db.get_stats(None, None, None, None).await.unwrap();
    assert_eq!(stats.total_requests, 0);
}

// ==================== Provider: update_token ====================

#[tokio::test]
async fn db_provider_update_token() {
    let db = test_db().await;
    create_test_provider(&db, "tok-prov").await;
    db.update_provider_token("tok-prov", "new-token-abc", Some("refresh-xyz"), "2099-12-31T23:59:59Z").await.unwrap();
    let provider = db.get_provider("tok-prov").await.unwrap().unwrap();
    assert_eq!(provider.current_token, Some("new-token-abc".to_string()));
    assert_eq!(provider.current_refresh_token, Some("refresh-xyz".to_string()));
}

#[tokio::test]
async fn db_provider_update_token_no_refresh() {
    let db = test_db().await;
    create_test_provider(&db, "tok-nr-prov").await;
    db.update_provider_token("tok-nr-prov", "new-token", None, "2099-12-31T23:59:59Z").await.unwrap();
    let provider = db.get_provider("tok-nr-prov").await.unwrap().unwrap();
    assert_eq!(provider.current_token, Some("new-token".to_string()));
}

// ==================== Quota: basic CRUD ====================

#[tokio::test]
async fn db_quota_set_and_list() {
    let db = test_db().await;
    create_test_provider(&db, "quota-prov").await;
    db.set_provider_quota("quota-prov", "request", "fixed", "day", None, None, None, 1000, true).await.unwrap();

    let quotas = db.list_provider_quotas("quota-prov").await.unwrap();
    assert_eq!(quotas.len(), 1);
    assert_eq!(quotas[0].quota_type, "request");
    assert_eq!(quotas[0].limit_count, 1000);
}

#[tokio::test]
async fn db_quota_delete() {
    let db = test_db().await;
    create_test_provider(&db, "qdel-prov").await;
    db.set_provider_quota("qdel-prov", "request", "fixed", "day", None, None, None, 1000, true).await.unwrap();

    let deleted = db.delete_provider_quota("qdel-prov", "request").await.unwrap();
    assert!(deleted);
    let quotas = db.list_provider_quotas("qdel-prov").await.unwrap();
    assert!(quotas.is_empty());
}

#[tokio::test]
async fn db_quota_delete_not_found() {
    let db = test_db().await;
    let deleted = db.delete_provider_quota("nonexistent", "request").await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn db_quota_list_all() {
    let db = test_db().await;
    create_test_provider(&db, "qa-prov1").await;
    create_test_provider(&db, "qa-prov2").await;
    db.set_provider_quota("qa-prov1", "request", "fixed", "day", None, None, None, 100, true).await.unwrap();
    db.set_provider_quota("qa-prov2", "token", "fixed", "day", None, None, None, 50000, true).await.unwrap();

    let all = db.list_all_quotas().await.unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn db_quota_rolling_mode() {
    let db = test_db().await;
    create_test_provider(&db, "roll-prov").await;
    db.set_provider_quota("roll-prov", "request", "rolling", "8h", None, Some("2h"), Some("Asia/Shanghai"), 1000, true).await.unwrap();

    let quotas = db.list_provider_quotas("roll-prov").await.unwrap();
    assert_eq!(quotas.len(), 1);
    assert_eq!(quotas[0].window_mode, "rolling");
    assert_eq!(quotas[0].rolling_step, Some("2h".to_string()));
}

#[tokio::test]
async fn db_quota_multiple_types_same_provider() {
    let db = test_db().await;
    create_test_provider(&db, "mt-prov").await;
    db.set_provider_quota("mt-prov", "request", "fixed", "day", None, None, None, 1000, true).await.unwrap();
    db.set_provider_quota("mt-prov", "token", "fixed", "day", None, None, None, 50000, true).await.unwrap();

    let quotas = db.list_provider_quotas("mt-prov").await.unwrap();
    assert_eq!(quotas.len(), 2);
}

#[tokio::test]
async fn db_quota_update_upsert() {
    let db = test_db().await;
    create_test_provider(&db, "upsert-prov").await;
    db.set_provider_quota("upsert-prov", "request", "fixed", "day", None, None, None, 1000, true).await.unwrap();
    // Upsert: same provider + quota_type, different limit
    db.set_provider_quota("upsert-prov", "request", "fixed", "day", None, None, None, 2000, true).await.unwrap();

    let quotas = db.list_provider_quotas("upsert-prov").await.unwrap();
    assert_eq!(quotas.len(), 1);
    assert_eq!(quotas[0].limit_count, 2000);
}

// ==================== Quota: calibration ====================

#[tokio::test]
async fn db_quota_calibration_set_and_get() {
    let db = test_db().await;
    create_test_provider(&db, "cal-prov").await;
    db.set_provider_quota("cal-prov", "request", "fixed", "day", None, None, None, 1000, true).await.unwrap();
    db.set_quota_calibration("cal-prov", "request", 500, Some("2024-01-15T10:00:00Z"), Some("2024-01-16T10:00:00Z"), None).await.unwrap();

    let cal = db.get_quota_calibration("cal-prov", "request").await.unwrap();
    assert!(cal.is_some());
    let c = cal.unwrap();
    assert_eq!(c.calibration_offset, 500);
}

#[tokio::test]
async fn db_quota_calibration_not_found() {
    let db = test_db().await;
    let cal = db.get_quota_calibration("nonexistent", "request").await.unwrap();
    assert!(cal.is_none());
}

#[tokio::test]
async fn db_quota_calibration_list() {
    let db = test_db().await;
    create_test_provider(&db, "callist-prov").await;
    db.set_provider_quota("callist-prov", "request", "fixed", "day", None, None, None, 1000, true).await.unwrap();
    db.set_quota_calibration("callist-prov", "request", 500, Some("2024-01-15T10:00:00Z"), Some("2024-01-16T10:00:00Z"), None).await.unwrap();

    let cals = db.list_provider_calibrations("callist-prov").await.unwrap();
    assert!(!cals.is_empty());
}

// ==================== Quota: usage ====================

#[tokio::test]
async fn db_quota_count_requests() {
    let db = test_db().await;
    create_test_provider(&db, "qcr-prov").await;
    db.insert_request_log("key1", "qcr-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key2", "qcr-prov", Some("model-b"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();

    let count = db.count_provider_requests("qcr-prov", "1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z").await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn db_quota_count_requests_no_match() {
    let db = test_db().await;
    create_test_provider(&db, "qcr-empty").await;
    let count = db.count_provider_requests("qcr-empty", "1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z").await.unwrap();
    assert_eq!(count, 0);
}

// ==================== Quota: all usage ====================

#[tokio::test]
async fn db_get_all_quota_usage() {
    let db = test_db().await;
    create_test_provider(&db, "gau-prov").await;
    db.set_provider_quota("gau-prov", "request", "fixed", "day", None, None, None, 1000, true).await.unwrap();

    let usage = db.get_all_quota_usage().await.unwrap();
    assert!(!usage.is_empty());
}

// ==================== Provider: list_active_model_mappings ====================

#[tokio::test]
async fn db_list_active_model_mappings() {
    let db = test_db().await;
    create_test_provider(&db, "lam-map-prov").await;
    db.create_model("lam-map-m", "LAM Map", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("lam-map-m", "lam-map-prov", "gpt-4", 1, 1.0).await.unwrap();
    db.add_model_mapping("lam-map-m", "lam-map-prov", "gpt-4o", 2, 0.5).await.unwrap();

    let active = db.list_active_model_mappings("lam-map-m").await.unwrap();
    assert_eq!(active.len(), 2);
}

#[tokio::test]
async fn db_list_active_model_mappings_filters_inactive() {
    let db = test_db().await;
    create_test_provider(&db, "lami-prov").await;
    db.create_model("lami-m", "LAMI", None, "chat", 1, None).await.unwrap();
    db.add_model_mapping("lami-m", "lami-prov", "gpt-4", 1, 1.0).await.unwrap();
    db.add_model_mapping("lami-m", "lami-prov", "gpt-4o", 2, 0.5).await.unwrap();
    // Deactivate one mapping
    db.update_model_mapping("lami-m", "lami-prov", "gpt-4o", "gpt-4o", false, 2, 0.5).await.unwrap();

    let active = db.list_active_model_mappings("lami-m").await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].provider_model_id, "gpt-4");
}

// ==================== Provider: query_request_logs_lightweight ====================

#[tokio::test]
async fn db_query_logs_lightweight() {
    let db = test_db().await;
    create_test_provider(&db, "lwl-prov").await;
    db.insert_request_log(
        "key1", "lwl-prov", Some("model-a"), "/v1/chat", "POST",
        None, Some("big request body"), Some(200), None, Some("big response body"),
        10, 20, 30, Some(100), false, false, None
    ).await.unwrap();

    let lightweight = db.query_request_logs_lightweight(None, None, None, None, None, None, None, 100, 0).await.unwrap();
    assert_eq!(lightweight.len(), 1);
    // Lightweight should still return records even without body fields
}

#[tokio::test]
async fn db_query_logs_lightweight_with_filter() {
    let db = test_db().await;
    create_test_provider(&db, "lwl-f-prov").await;
    db.insert_request_log("key1", "lwl-f-prov", Some("model-a"), "/v1/chat", "POST", None, None, Some(200), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();
    db.insert_request_log("key1", "lwl-f-prov", Some("model-b"), "/v1/chat", "POST", None, None, Some(500), None, None, 10, 20, 30, Some(100), false, false, None).await.unwrap();

    let filtered = db.query_request_logs_lightweight(None, Some("lwl-f-prov"), None, None, None, None, None, 100, 0).await.unwrap();
    assert_eq!(filtered.len(), 2);
}
