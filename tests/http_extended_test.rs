use axum::{
    Router,
    routing::{get, post, delete, put},
    http::{Method, StatusCode},
};
use llm_gateway::{AppState, db::Database, auth::AuthManager, proxy::LlmProxy, stats::StatsCollector};
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};
use axum_test::TestServer;

async fn build_test_app() -> TestServer {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let stats = Arc::new(StatsCollector::new(db.clone()));
    let proxy = Arc::new(LlmProxy::new(db.clone(), auth_manager.clone(), stats.clone()));
    let state = AppState { db: db.clone(), proxy: proxy.clone(), auth_manager: auth_manager.clone(), stats_collector: stats.clone() };
    let cors = CorsLayer::new().allow_origin(Any).allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH]).allow_headers(Any);
    let app = Router::new()
        .route("/api/v1/api-keys", post(llm_gateway::api::create_api_key).get(llm_gateway::api::list_api_keys))
        .route("/api/v1/api-keys/batch-update-status", post(llm_gateway::api::batch_update_api_key_status))
        .route("/api/v1/api-keys/:id", get(llm_gateway::api::get_api_key).delete(llm_gateway::api::delete_api_key).put(llm_gateway::api::update_api_key))
        .route("/api/v1/api-keys/:id/regenerate", post(llm_gateway::api::regenerate_api_key))
        .route("/api/v1/providers", post(llm_gateway::api::create_provider).get(llm_gateway::api::list_providers))
        .route("/api/v1/providers/batch-update-status", post(llm_gateway::api::batch_update_provider_status))
        .route("/api/v1/providers/:id", get(llm_gateway::api::get_provider).delete(llm_gateway::api::delete_provider).put(llm_gateway::api::update_provider))
        .route("/api/v1/providers/:id/refresh-token", post(llm_gateway::api::refresh_provider_token))
        .route("/api/v1/providers/:id/chart-color", put(llm_gateway::api::update_provider_chart_color))
        .route("/api/v1/providers/:id/models", post(llm_gateway::api::add_provider_model).get(llm_gateway::api::list_provider_models))
        .route("/api/v1/providers/:id/models/:model_id", delete(llm_gateway::api::remove_provider_model))
        .route("/api/v1/providers/:id/models/:model_id/test", post(llm_gateway::api::test_provider_model))
        .route("/api/v1/providers/:id/models/test-all", post(llm_gateway::api::test_all_provider_models))
        .route("/api/v1/models", post(llm_gateway::api::create_model).get(llm_gateway::api::list_models))
        .route("/api/v1/models/:id", get(llm_gateway::api::get_model).delete(llm_gateway::api::delete_model).put(llm_gateway::api::update_model))
        .route("/api/v1/models/:id/mappings", get(llm_gateway::api::list_model_mappings).post(llm_gateway::api::add_model_mapping))
        .route("/api/v1/models/:id/mappings/:provider_id/:provider_model_id", put(llm_gateway::api::update_model_mapping).delete(llm_gateway::api::remove_model_mapping))
        .route("/api/v1/quotas/usage", get(llm_gateway::api::get_all_quota_usage))
        .route("/api/v1/quotas/:provider_id", post(llm_gateway::api::set_provider_quota).get(llm_gateway::api::get_provider_quota_usage))
        .route("/api/v1/quotas/:provider_id/:quota_type", delete(llm_gateway::api::delete_provider_quota))
        .route("/api/v1/quotas/:provider_id/calibration", post(llm_gateway::api::set_quota_calibration).get(llm_gateway::api::get_provider_calibrations))
        .route("/api/v1/stats", get(llm_gateway::api::get_stats))
        .route("/api/v1/stats/bucketed", get(llm_gateway::api::get_time_bucketed_stats))
        .route("/api/v1/logs", get(llm_gateway::api::get_request_logs))
        .route("/api/v1/logs/:id", get(llm_gateway::api::get_request_log_detail).delete(llm_gateway::api::delete_request_log))
        .route("/api/v1/logs/batch-delete", post(llm_gateway::api::delete_request_logs_batch))
        .route("/api/v1/logs/delete-all", post(llm_gateway::api::delete_all_request_logs))
        .route("/api/v1/dashboard/summary", get(llm_gateway::api::get_dashboard_summary))
        .route("/api/v1/dashboard/health", get(llm_gateway::api::get_provider_health))
        .route("/api/v1/dashboard/top-provider", get(llm_gateway::api::get_top_provider))
        .route("/api/v1/stats/by-api-key", get(llm_gateway::api::get_stats_by_api_key))
        .route("/api/v1/stats/usage-trend", get(llm_gateway::api::get_usage_trend))
        .route("/api/v1/dashboard/token-rate", get(llm_gateway::api::get_token_rate))
        .with_state(state)
        .layer(cors);
    TestServer::new(app).unwrap()
}

async fn create_provider_via_api(server: &TestServer, id: &str) {
    let body = serde_json::json!({
        "id": id, "name": id, "base_url": "https://api.example.com/v1",
        "api_type": "openai", "auth_type": "api_key", "api_key": "test-key"
    });
    let resp = server.post("/api/v1/providers").json(&body).await;
    assert_eq!(resp.status_code(), StatusCode::CREATED);
}

async fn create_api_key_via_api(server: &TestServer, name: &str) -> String {
    let body = serde_json::json!({"name": name});
    let resp = server.post("/api/v1/api-keys").json(&body).await;
    assert_eq!(resp.status_code(), StatusCode::CREATED);
    resp.json::<serde_json::Value>()["id"].as_str().unwrap().to_string()
}

// ==================== Model CRUD HTTP Tests ====================

#[tokio::test]
async fn http_model_get_by_id() {
    let server = build_test_app().await;
    let body = serde_json::json!({"id": "get-mod", "name": "GPT-4", "model_type": "chat"});
    let create_resp = server.post("/api/v1/models").json(&body).await;
    assert!(create_resp.status_code() == StatusCode::CREATED || create_resp.status_code() == StatusCode::OK);
    let resp = server.get("/api/v1/models/get-mod").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert_eq!(data["model"]["id"], "get-mod");
    assert_eq!(data["model"]["name"], "GPT-4");
}

#[tokio::test]
async fn http_model_get_not_found() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/models/nonexistent").await;
    assert_eq!(resp.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_model_update() {
    let server = build_test_app().await;
    let body = serde_json::json!({"id": "upd-mod", "name": "Original", "model_type": "chat"});
    server.post("/api/v1/models").json(&body).await;
    let update = serde_json::json!({"name": "Updated", "description": "New desc", "model_type": "chat", "is_active": true, "priority": 2});
    let resp = server.put("/api/v1/models/upd-mod").json(&update).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert_eq!(data["message"], "Model updated");
    // Verify by getting the model
    let get_resp = server.get("/api/v1/models/upd-mod").await;
    let get_data = get_resp.json::<serde_json::Value>();
    assert_eq!(get_data["model"]["name"], "Updated");
}

#[tokio::test]
async fn http_model_update_not_found() {
    let server = build_test_app().await;
    let update = serde_json::json!({"name": "Updated", "model_type": "chat", "is_active": true, "priority": 1});
    let resp = server.put("/api/v1/models/nonexistent").json(&update).await;
    assert_eq!(resp.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_model_delete() {
    let server = build_test_app().await;
    let body = serde_json::json!({"id": "del-mod", "name": "ToDelete", "model_type": "chat"});
    server.post("/api/v1/models").json(&body).await;
    let resp = server.delete("/api/v1/models/del-mod").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    // Verify deleted
    let get_resp = server.get("/api/v1/models/del-mod").await;
    assert_eq!(get_resp.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_model_delete_not_found() {
    let server = build_test_app().await;
    let resp = server.delete("/api/v1/models/nonexistent").await;
    assert_eq!(resp.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_model_list() {
    let server = build_test_app().await;
    let b1 = serde_json::json!({"id": "lm-1", "name": "Model 1", "model_type": "chat"});
    let b2 = serde_json::json!({"id": "lm-2", "name": "Model 2", "model_type": "chat"});
    server.post("/api/v1/models").json(&b1).await;
    server.post("/api/v1/models").json(&b2).await;
    let resp = server.get("/api/v1/models").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert!(data.as_array().unwrap().len() >= 2);
}

// ==================== Model Mapping HTTP Tests ====================

#[tokio::test]
async fn http_model_mapping_add() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "map-prov").await;
    let model = serde_json::json!({"id": "map-model", "name": "Map Model", "model_type": "chat"});
    server.post("/api/v1/models").json(&model).await;
    let mapping = serde_json::json!({"provider_id": "map-prov", "provider_model_id": "gpt-4", "weight": 1, "cost_multiplier": 1.0});
    let resp = server.post("/api/v1/models/map-model/mappings").json(&mapping).await;
    assert!(resp.status_code() == StatusCode::CREATED || resp.status_code() == StatusCode::OK);
}

#[tokio::test]
async fn http_model_mapping_list() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "ml-prov").await;
    let model = serde_json::json!({"id": "ml-model", "name": "ML Model", "model_type": "chat"});
    server.post("/api/v1/models").json(&model).await;
    let mapping = serde_json::json!({"provider_id": "ml-prov", "provider_model_id": "gpt-4", "weight": 1, "cost_multiplier": 1.0});
    server.post("/api/v1/models/ml-model/mappings").json(&mapping).await;
    let resp = server.get("/api/v1/models/ml-model/mappings").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert!(data.as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn http_model_mapping_update() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "um-prov").await;
    let model = serde_json::json!({"id": "um-model", "name": "UM Model", "model_type": "chat"});
    server.post("/api/v1/models").json(&model).await;
    let mapping = serde_json::json!({"provider_id": "um-prov", "provider_model_id": "gpt-4", "weight": 1, "cost_multiplier": 1.0});
    server.post("/api/v1/models/um-model/mappings").json(&mapping).await;
    let update = serde_json::json!({"provider_model_id": "gpt-4o", "is_active": true, "weight": 2, "cost_multiplier": 0.5});
    let resp = server.put("/api/v1/models/um-model/mappings/um-prov/gpt-4").json(&update).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_model_mapping_delete() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "dm-prov").await;
    let model = serde_json::json!({"id": "dm-model", "name": "DM Model", "model_type": "chat"});
    server.post("/api/v1/models").json(&model).await;
    let mapping = serde_json::json!({"provider_id": "dm-prov", "provider_model_id": "gpt-4", "weight": 1, "cost_multiplier": 1.0});
    server.post("/api/v1/models/dm-model/mappings").json(&mapping).await;
    let resp = server.delete("/api/v1/models/dm-model/mappings/dm-prov/gpt-4").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

// ==================== Quota Usage HTTP Tests ====================

#[tokio::test]
async fn http_quota_all_usage() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "qa-prov").await;
    let quota = serde_json::json!({"quota_type": "fixed:5h", "window_mode": "fixed", "window_size": "5h", "limit_count": 1000, "is_enabled": true});
    server.post("/api/v1/quotas/qa-prov").json(&quota).await;
    let resp = server.get("/api/v1/quotas/usage").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert!(data.as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn http_quota_provider_usage() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "pu-prov").await;
    let quota = serde_json::json!({"quota_type": "fixed:5h", "window_mode": "fixed", "window_size": "5h", "limit_count": 1000, "is_enabled": true});
    server.post("/api/v1/quotas/pu-prov").json(&quota).await;
    let resp = server.get("/api/v1/quotas/pu-prov").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}


#[tokio::test]
async fn http_quota_delete() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "qd-prov").await;
    let quota = serde_json::json!({"quota_type": "fixed:5h", "window_mode": "fixed", "window_size": "5h", "limit_count": 1000, "is_enabled": true});
    server.post("/api/v1/quotas/qd-prov").json(&quota).await;
    let resp = server.delete("/api/v1/quotas/qd-prov/fixed:5h").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_quota_calibrations_list() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "cl-prov").await;
    let quota = serde_json::json!({"quota_type": "fixed:5h", "window_mode": "fixed", "window_size": "5h", "limit_count": 1000, "is_enabled": true});
    server.post("/api/v1/quotas/cl-prov").json(&quota).await;
    let cal = serde_json::json!({"quota_type": "fixed:5h", "calibration_offset": 50, "note": "test"});
    server.post("/api/v1/quotas/cl-prov/calibration").json(&cal).await;
    let resp = server.get("/api/v1/quotas/cl-prov/calibration").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_log_detail_not_found() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/logs/99999").await;
    assert_eq!(resp.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_log_delete_single_not_found() {
    let server = build_test_app().await;
    let resp = server.delete("/api/v1/logs/99999").await;
    assert_eq!(resp.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_provider_chart_color() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "cc-prov").await;
    let body = serde_json::json!({"color": "#ff0000"});
    let resp = server.put("/api/v1/providers/cc-prov/chart-color").json(&body).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_provider_refresh_token_no_dynamic() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "rt-prov").await;
    let resp = server.post("/api/v1/providers/rt-prov/refresh-token").await;
    // Refresh token endpoint succeeds even for api_key providers (returns current auth)
    assert!(resp.status_code() == StatusCode::OK || resp.status_code() == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_api_key_update_via_put() {
    let server = build_test_app().await;
    let key_id = create_api_key_via_api(&server, "put-key").await;
    let update = serde_json::json!({"name": "Updated Name", "is_active": true});
    let resp = server.put(&format!("/api/v1/api-keys/{}", key_id)).json(&update).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert_eq!(data["name"], "Updated Name");
}

#[tokio::test]
async fn http_api_key_update_deactivate() {
    let server = build_test_app().await;
    let key_id = create_api_key_via_api(&server, "deact-key").await;
    let update = serde_json::json!({"name": "deact-key", "is_active": false});
    let resp = server.put(&format!("/api/v1/api-keys/{}", key_id)).json(&update).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_api_key_regenerate_via_post() {
    let server = build_test_app().await;
    let key_id = create_api_key_via_api(&server, "regen-key").await;
    let resp = server.post(&format!("/api/v1/api-keys/{}/regenerate", key_id)).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert!(data["key"].is_string());
}

#[tokio::test]
async fn http_provider_update_multiple_fields() {
    let server = build_test_app().await;
    create_provider_via_api(&server, "mf-prov").await;
    let update = serde_json::json!({"name": "Updated Provider", "base_url": "https://new-api.example.com/v1", "weight": 5});
    let resp = server.put("/api/v1/providers/mf-prov").json(&update).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert_eq!(data["id"], "mf-prov");
}

#[tokio::test]
async fn http_stats_empty_database() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/stats").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert_eq!(data["total_requests"], 0);
}

#[tokio::test]
async fn http_usage_trend_default() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/stats/usage-trend").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_top_provider_empty() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/dashboard/top-provider").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_token_rate_empty() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/dashboard/token-rate").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_health_empty() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/dashboard/health").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_bucketed_stats_empty() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/stats/bucketed").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_stats_by_api_key_empty() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/stats/by-api-key").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_logs_empty() {
    let server = build_test_app().await;
    let resp = server.get("/api/v1/logs").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_logs_delete_all_empty() {
    let server = build_test_app().await;
    let resp = server.post("/api/v1/logs/delete-all").await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_logs_batch_delete_empty() {
    let server = build_test_app().await;
    let body = serde_json::json!({"provider_id": "batch-del-prov"});
    let resp = server.post("/api/v1/logs/batch-delete").json(&body).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn http_model_create_and_get() {
    let server = build_test_app().await;
    let body = serde_json::json!({"id": "cg-model", "name": "CreateGet Model", "model_type": "chat", "description": "Test model"});
    let create_resp = server.post("/api/v1/models").json(&body).await;
    assert!(create_resp.status_code() == StatusCode::CREATED || create_resp.status_code() == StatusCode::OK);
    let get_resp = server.get("/api/v1/models/cg-model").await;
    assert_eq!(get_resp.status_code(), StatusCode::OK);
    let data = get_resp.json::<serde_json::Value>();
    assert_eq!(data["model"]["description"], "Test model");
}

#[tokio::test]
async fn http_model_deactivate_via_update() {
    let server = build_test_app().await;
    let body = serde_json::json!({"id": "deact-mod", "name": "Deactivate Model", "model_type": "chat"});
    server.post("/api/v1/models").json(&body).await;
    let update = serde_json::json!({"name": "Deactivate Model", "model_type": "chat", "is_active": false, "priority": 1});
    let resp = server.put("/api/v1/models/deact-mod").json(&update).await;
    assert_eq!(resp.status_code(), StatusCode::OK);
    let data = resp.json::<serde_json::Value>();
    assert_eq!(data["message"], "Model updated");
}
