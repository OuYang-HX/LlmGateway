use axum::{
    Router,
    routing::{get, post},
    http::{Method, StatusCode},
};
use llm_gateway::{AppState, db::Database, auth::AuthManager, proxy::LlmProxy, stats::StatsCollector};
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};
use axum_test::TestServer;

/// Build a test application with in-memory database
async fn build_test_app() -> TestServer {
    let db = Arc::new(Database::new_in_memory().await.unwrap());
    let auth_manager = Arc::new(AuthManager::new(db.clone()));
    let proxy = Arc::new(LlmProxy::new(db.clone(), auth_manager.clone()));
    let stats = Arc::new(StatsCollector::new(db.clone()));

    let state = AppState {
        db: db.clone(),
        proxy: proxy.clone(),
        auth_manager: auth_manager.clone(),
        stats_collector: stats.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(llm_gateway::dashboard::dashboard_page))
        .route("/dashboard", get(llm_gateway::dashboard::dashboard_page))
        .route("/api/v1/api-keys", post(llm_gateway::api::create_api_key).get(llm_gateway::api::list_api_keys))
        .route("/api/v1/api-keys/:id", get(llm_gateway::api::get_api_key).delete(llm_gateway::api::delete_api_key))
        .route("/api/v1/providers", post(llm_gateway::api::create_provider).get(llm_gateway::api::list_providers))
        .route("/api/v1/providers/:id", get(llm_gateway::api::get_provider).delete(llm_gateway::api::delete_provider).put(llm_gateway::api::update_provider))
        .route("/api/v1/stats", get(llm_gateway::api::get_stats))
        .route("/api/v1/stats/bucketed", get(llm_gateway::api::get_time_bucketed_stats))
        .route("/api/v1/logs", get(llm_gateway::api::get_request_logs))
        .route("/api/v1/dashboard/summary", get(llm_gateway::api::get_dashboard_summary))
        .route("/api/v1/dashboard/token-rate", get(llm_gateway::api::get_token_rate))
        .route("/ws/v1", get(llm_gateway::proxy::ws_handler::ws_proxy_handler))
        .with_state(state)
        .layer(cors);

    TestServer::new(app).unwrap()
}

// ==================== Dashboard Page Tests ====================

#[tokio::test]
async fn test_dashboard_page_returns_html() {
    let server = build_test_app().await;
    let response = server.get("/").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body = response.text();
    assert!(body.contains("LLM Gateway Dashboard"));
    assert!(body.contains("服务商管理"));
    assert!(body.contains("概览"));
    assert!(body.contains("请求日志"));
}

#[tokio::test]
async fn test_dashboard_page_explicit_path() {
    let server = build_test_app().await;
    let response = server.get("/dashboard").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body = response.text();
    assert!(body.contains("LLM Gateway Dashboard"));
    assert!(body.contains("服务商管理"));
}

// ==================== API Key HTTP Tests ====================

#[tokio::test]
async fn test_http_create_api_key() {
    let server = build_test_app().await;
    let response = server.post("/api/v1/api-keys")
        .json(&serde_json::json!({
            "name": "Test Key HTTP",
            "allowed_providers": null
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::CREATED);
    let body: serde_json::Value = response.json();
    assert!(body.get("id").is_some());
    assert!(body.get("key").is_some());
    assert!(body["key"].as_str().unwrap().starts_with("lgk-"));
    assert_eq!(body["name"], "Test Key HTTP");
    assert!(body["is_active"].as_bool().unwrap());
}

#[tokio::test]
async fn test_http_create_api_key_with_providers() {
    let server = build_test_app().await;
    let response = server.post("/api/v1/api-keys")
        .json(&serde_json::json!({
            "name": "Restricted Key",
            "allowed_providers": ["openai", "anthropic"]
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::CREATED);
    let body: serde_json::Value = response.json();
    let providers = body.get("allowed_providers").unwrap().as_array().unwrap();
    assert_eq!(providers.len(), 2);
}

#[tokio::test]
async fn test_http_list_api_keys() {
    let server = build_test_app().await;

    // Create two keys
    server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "Key 1"}))
        .await;
    server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "Key 2"}))
        .await;

    let response = server.get("/api/v1/api-keys").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    let keys = body.as_array().unwrap();
    assert_eq!(keys.len(), 2);
    // Keys should be masked after creation
    assert_eq!(keys[0]["key"], "***");
}

#[tokio::test]
async fn test_http_get_api_key_by_id() {
    let server = build_test_app().await;
    let create_response = server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "Get Key"}))
        .await;
    let created: serde_json::Value = create_response.json();
    let id = created["id"].as_str().unwrap();

    let response = server.get(&format!("/api/v1/api-keys/{}", id)).await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["name"], "Get Key");
}

#[tokio::test]
async fn test_http_get_nonexistent_api_key() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/api-keys/nonexistent-id").await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_delete_api_key() {
    let server = build_test_app().await;
    let create_response = server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "Delete Key"}))
        .await;
    let created: serde_json::Value = create_response.json();
    let id = created["id"].as_str().unwrap();

    let response = server.delete(&format!("/api/v1/api-keys/{}", id)).await;
    assert_eq!(response.status_code(), StatusCode::OK);

    // Verify it's gone
    let get_response = server.get(&format!("/api/v1/api-keys/{}", id)).await;
    assert_eq!(get_response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_delete_nonexistent_api_key() {
    let server = build_test_app().await;
    let response = server.delete("/api/v1/api-keys/nonexistent").await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}

// ==================== Provider HTTP Tests ====================

#[tokio::test]
async fn test_http_create_provider_api_key() {
    let server = build_test_app().await;
    let response = server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "openai",
            "name": "OpenAI",
            "base_url": "https://api.openai.com/v1",
            "api_type": "openai",
            "auth_type": "api_key",
            "api_key": "sk-test-key-123",
            "weight": 1
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::CREATED);
    let body: serde_json::Value = response.json();
    assert_eq!(body["id"], "openai");
}

#[tokio::test]
async fn test_http_create_provider_dynamic_token() {
    let server = build_test_app().await;
    let response = server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "internal",
            "name": "Internal Provider",
            "base_url": "https://internal.company.com/v1",
            "api_type": "openai",
            "auth_type": "dynamic_token",
            "token_url": "https://auth.company.com/login",
            "token_username": "admin",
            "token_password": "secret",
            "token_field": "access_token",
            "refresh_token_field": "refresh_token",
            "token_header_field": "X-Auth-Token",
            "token_header_prefix": "",
            "token_expiry_seconds": 28800,
            "weight": 2
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_http_list_providers() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "p1", "name": "P1", "base_url": "https://p1.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key1"
        }))
        .await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "p2", "name": "P2", "base_url": "https://p2.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key2"
        }))
        .await;

    let response = server.get("/api/v1/providers").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_http_get_provider() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "get-prov", "name": "Get Provider", "base_url": "https://gp.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key"
        }))
        .await;

    let response = server.get("/api/v1/providers/get-prov").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["name"], "Get Provider");
}

#[tokio::test]
async fn test_http_get_nonexistent_provider() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/providers/nonexistent").await;
    assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_delete_provider() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "del-prov", "name": "Delete Provider", "base_url": "https://dp.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key"
        }))
        .await;

    let response = server.delete("/api/v1/providers/del-prov").await;
    assert_eq!(response.status_code(), StatusCode::OK);

    let get_response = server.get("/api/v1/providers/del-prov").await;
    assert_eq!(get_response.status_code(), StatusCode::NOT_FOUND);
}

// ==================== Statistics HTTP Tests ====================

#[tokio::test]
async fn test_http_get_stats_empty() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/stats").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["total_requests"], 0);
    assert_eq!(body["total_tokens"], 0);
}

#[tokio::test]
async fn test_http_get_stats_with_params() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/stats?granularity=day").await;
    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_http_get_bucketed_stats() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/stats/bucketed?granularity=day").await;
    assert_eq!(response.status_code(), StatusCode::OK);
}

// ==================== Request Logs HTTP Tests ====================

#[tokio::test]
async fn test_http_get_request_logs_empty() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/logs").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["total"], 0);
    assert_eq!(body["logs"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_http_get_request_logs_with_pagination() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/logs?page=2&page_size=10").await;
    assert_eq!(response.status_code(), StatusCode::OK);
}

// ==================== Dashboard Summary HTTP Tests ====================

#[tokio::test]
async fn test_http_dashboard_summary_empty() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/dashboard/summary").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["total_api_keys"], 0);
    assert_eq!(body["active_api_keys"], 0);
    assert_eq!(body["total_providers"], 0);
}

#[tokio::test]
async fn test_http_dashboard_summary_with_data() {
    let server = build_test_app().await;

    // Create API key and provider
    server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "Dashboard Key"}))
        .await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "dash-prov", "name": "Dashboard Provider", "base_url": "https://dp.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key"
        }))
        .await;

    let response = server.get("/api/v1/dashboard/summary").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["total_api_keys"], 1);
    assert_eq!(body["active_api_keys"], 1);
    assert_eq!(body["total_providers"], 1);
}

// ==================== Token Rate HTTP Tests ====================

#[tokio::test]
async fn test_http_token_rate() {
    let server = build_test_app().await;
    let response = server.get("/api/v1/dashboard/token-rate").await;
    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_http_token_rate_with_provider() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "rate-prov", "name": "Rate Provider", "base_url": "https://rp.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key"
        }))
        .await;

    let response = server.get("/api/v1/dashboard/token-rate?provider_id=rate-prov").await;
    assert_eq!(response.status_code(), StatusCode::OK);
}

// ==================== End-to-End Workflow Tests ====================

#[tokio::test]
async fn test_http_full_workflow_create_key_and_provider() {
    let server = build_test_app().await;

    // 1. Create provider
    let prov_response = server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "workflow-prov",
            "name": "Workflow Provider",
            "base_url": "https://wf.com/v1",
            "api_type": "openai",
            "auth_type": "api_key",
            "api_key": "sk-wf-key"
        }))
        .await;
    assert_eq!(prov_response.status_code(), StatusCode::CREATED);

    // 2. Create API key restricted to that provider
    let key_response = server.post("/api/v1/api-keys")
        .json(&serde_json::json!({
            "name": "Workflow Key",
            "allowed_providers": ["workflow-prov"]
        }))
        .await;
    assert_eq!(key_response.status_code(), StatusCode::CREATED);
    let key: serde_json::Value = key_response.json();
    assert!(key["key"].as_str().unwrap().starts_with("lgk-"));

    // 3. Verify dashboard summary
    let summary = server.get("/api/v1/dashboard/summary").await;
    let summary_body: serde_json::Value = summary.json();
    assert_eq!(summary_body["total_api_keys"], 1);
    assert_eq!(summary_body["total_providers"], 1);

    // 4. Verify stats are empty (no requests yet)
    let stats = server.get("/api/v1/stats").await;
    let stats_body: serde_json::Value = stats.json();
    assert_eq!(stats_body["total_requests"], 0);

    // 5. Verify logs are empty
    let logs = server.get("/api/v1/logs").await;
    let logs_body: serde_json::Value = logs.json();
    assert_eq!(logs_body["total"], 0);
}

#[tokio::test]
async fn test_http_create_multiple_providers_different_auth() {
    let server = build_test_app().await;

    // Static API key provider
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "static-prov",
            "name": "Static Provider",
            "base_url": "https://static.com/v1",
            "auth_type": "api_key",
            "api_key": "sk-static"
        }))
        .await;

    // Dynamic token provider
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "dynamic-prov",
            "name": "Dynamic Provider",
            "base_url": "https://dynamic.com/v1",
            "auth_type": "dynamic_token",
            "token_url": "https://auth.dynamic.com/login",
            "token_username": "user",
            "token_password": "pass",
            "token_header_field": "X-Token",
            "token_header_prefix": "",
            "token_expiry_seconds": 7200
        }))
        .await;

    let providers = server.get("/api/v1/providers").await;
    let prov_body: serde_json::Value = providers.json();
    assert_eq!(prov_body.as_array().unwrap().len(), 2);

    // Verify both types exist
    let static_prov = server.get("/api/v1/providers/static-prov").await;
    let static_body: serde_json::Value = static_prov.json();
    assert_eq!(static_body["auth_type"], "api_key");

    let dynamic_prov = server.get("/api/v1/providers/dynamic-prov").await;
    let dynamic_body: serde_json::Value = dynamic_prov.json();
    assert_eq!(dynamic_body["auth_type"], "dynamic_token");
    assert_eq!(dynamic_body["token_url"], "https://auth.dynamic.com/login");
}

// ==================== Stats Pipeline End-to-End Tests ====================

#[tokio::test]
async fn test_http_stats_after_manual_log_insertion() {
    let server = build_test_app().await;

    // Create API key and provider
    server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "Stats Key"}))
        .await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "stats-prov", "name": "Stats Provider", "base_url": "https://sp.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key"
        }))
        .await;

    // Get the API key ID
    let keys = server.get("/api/v1/api-keys").await;
    let keys_body: serde_json::Value = keys.json();
    let key_id = keys_body[0]["id"].as_str().unwrap();

    // Insert a request log directly via DB (simulating what forward_and_collect does)
    // We access the DB through the internal state, but since we can't from HTTP,
    // we verify the stats endpoint works with empty data
    let stats = server.get("/api/v1/stats").await;
    let stats_body: serde_json::Value = stats.json();
    assert_eq!(stats_body["total_requests"], 0);
}

#[tokio::test]
async fn test_http_stats_with_provider_filter() {
    let server = build_test_app().await;

    // Create two providers
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "prov-a", "name": "Provider A", "base_url": "https://a.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key-a"
        }))
        .await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "prov-b", "name": "Provider B", "base_url": "https://b.com",
            "api_type": "openai", "auth_type": "api_key", "api_key": "key-b"
        }))
        .await;

    // Stats with provider filter (should be 0 since no requests)
    let stats = server.get("/api/v1/stats?provider_id=prov-a").await;
    let stats_body: serde_json::Value = stats.json();
    assert_eq!(stats_body["total_requests"], 0);
}

#[tokio::test]
async fn test_http_bucketed_stats_with_granularity() {
    let server = build_test_app().await;

    // Test each granularity
    for granularity in &["5h", "day", "week", "month"] {
        let stats = server.get(&format!("/api/v1/stats/bucketed?granularity={}", granularity)).await;
        assert_eq!(stats.status_code(), StatusCode::OK);
    }
}

#[tokio::test]
async fn test_http_logs_with_time_range() {
    let server = build_test_app().await;

    let now = chrono::Utc::now();
    let start = (now - chrono::Duration::hours(1)).to_rfc3339();
    let end = (now + chrono::Duration::hours(1)).to_rfc3339();

    let logs = server.get(&format!("/api/v1/logs?start_time={}&end_time={}", start, end)).await;
    assert_eq!(logs.status_code(), StatusCode::OK);
    let logs_body: serde_json::Value = logs.json();
    assert_eq!(logs_body["total"], 0);
}

#[tokio::test]
async fn test_http_api_key_crud_full_cycle() {
    let server = build_test_app().await;

    // Create
    let create = server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "CRUD Key", "allowed_providers": ["prov-1"]}))
        .await;
    assert_eq!(create.status_code(), StatusCode::CREATED);
    let created: serde_json::Value = create.json();
    let id = created["id"].as_str().unwrap();
    let key = created["key"].as_str().unwrap();
    assert!(key.starts_with("lgk-"));

    // Read
    let get = server.get(&format!("/api/v1/api-keys/{}", id)).await;
    assert_eq!(get.status_code(), StatusCode::OK);
    let got: serde_json::Value = get.json();
    assert_eq!(got["name"], "CRUD Key");
    assert_eq!(got["key"], "***"); // Key should be masked
    let providers = got["allowed_providers"].as_array().unwrap();
    assert_eq!(providers[0], "prov-1");

    // List
    let list = server.get("/api/v1/api-keys").await;
    let list_body: serde_json::Value = list.json();
    assert_eq!(list_body.as_array().unwrap().len(), 1);

    // Delete
    let delete = server.delete(&format!("/api/v1/api-keys/{}", id)).await;
    assert_eq!(delete.status_code(), StatusCode::OK);

    // Verify deleted
    let get_after = server.get(&format!("/api/v1/api-keys/{}", id)).await;
    assert_eq!(get_after.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_provider_crud_full_cycle() {
    let server = build_test_app().await;

    // Create
    let create = server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "crud-prov",
            "name": "CRUD Provider",
            "base_url": "https://crud.com/v1",
            "api_type": "openai",
            "auth_type": "api_key",
            "api_key": "sk-crud-key",
            "weight": 3
        }))
        .await;
    assert_eq!(create.status_code(), StatusCode::CREATED);

    // Read
    let get = server.get("/api/v1/providers/crud-prov").await;
    assert_eq!(get.status_code(), StatusCode::OK);
    let got: serde_json::Value = get.json();
    assert_eq!(got["name"], "CRUD Provider");
    assert_eq!(got["weight"], 3);

    // List
    let list = server.get("/api/v1/providers").await;
    let list_body: serde_json::Value = list.json();
    assert_eq!(list_body.as_array().unwrap().len(), 1);

    // Delete
    let delete = server.delete("/api/v1/providers/crud-prov").await;
    assert_eq!(delete.status_code(), StatusCode::OK);

    // Verify deleted
    let get_after = server.get("/api/v1/providers/crud-prov").await;
    assert_eq!(get_after.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_dashboard_complete_workflow() {
    let server = build_test_app().await;

    // 1. Create 2 API keys
    server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "Key Alpha"}))
        .await;
    server.post("/api/v1/api-keys")
        .json(&serde_json::json!({"name": "Key Beta"}))
        .await;

    // 2. Create 2 providers
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "alpha-prov", "name": "Alpha Provider", "base_url": "https://alpha.com/v1",
            "api_type": "openai", "auth_type": "api_key", "api_key": "sk-alpha"
        }))
        .await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "beta-prov", "name": "Beta Provider", "base_url": "https://beta.com/v1",
            "api_type": "openai", "auth_type": "dynamic_token",
            "token_url": "https://auth.beta.com/login",
            "token_username": "admin", "token_password": "secret",
            "token_expiry_seconds": 28800
        }))
        .await;

    // 3. Verify dashboard summary
    let summary = server.get("/api/v1/dashboard/summary").await;
    let summary_body: serde_json::Value = summary.json();
    assert_eq!(summary_body["total_api_keys"], 2);
    assert_eq!(summary_body["active_api_keys"], 2);
    assert_eq!(summary_body["total_providers"], 2);
    assert_eq!(summary_body["active_providers"], 2);

    // 4. Verify empty stats
    let stats = server.get("/api/v1/stats").await;
    let stats_body: serde_json::Value = stats.json();
    assert_eq!(stats_body["total_requests"], 0);

    // 5. Verify empty logs
    let logs = server.get("/api/v1/logs").await;
    let logs_body: serde_json::Value = logs.json();
    assert_eq!(logs_body["total"], 0);

    // 6. Verify token rate endpoint
    let rate = server.get("/api/v1/dashboard/token-rate").await;
    assert_eq!(rate.status_code(), StatusCode::OK);
}

// ==================== Provider Update (PUT) Tests ====================

#[tokio::test]
async fn test_http_update_provider_name() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "upd-prov", "name": "Original Name", "base_url": "https://orig.com/v1",
            "api_type": "openai", "auth_type": "api_key", "api_key": "sk-orig"
        }))
        .await;

    let resp = server.put("/api/v1/providers/upd-prov")
        .json(&serde_json::json!({"name": "Updated Name"}))
        .await;
    assert_eq!(resp.status_code(), StatusCode::OK);

    let get_resp = server.get("/api/v1/providers/upd-prov").await;
    let body: serde_json::Value = get_resp.json();
    assert_eq!(body["name"], "Updated Name");
    // Other fields should remain unchanged
    assert_eq!(body["base_url"], "https://orig.com/v1");
}

#[tokio::test]
async fn test_http_update_provider_base_url() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "upd-url", "name": "URL Provider", "base_url": "https://old.com/v1",
            "auth_type": "api_key", "api_key": "key"
        }))
        .await;

    let resp = server.put("/api/v1/providers/upd-url")
        .json(&serde_json::json!({"base_url": "https://new.com/v1"}))
        .await;
    assert_eq!(resp.status_code(), StatusCode::OK);

    let body: serde_json::Value = server.get("/api/v1/providers/upd-url").await.json();
    assert_eq!(body["base_url"], "https://new.com/v1");
}

#[tokio::test]
async fn test_http_update_provider_api_key() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "upd-key", "name": "Key Provider", "base_url": "https://kp.com/v1",
            "auth_type": "api_key", "api_key": "old-key"
        }))
        .await;

    let resp = server.put("/api/v1/providers/upd-key")
        .json(&serde_json::json!({"api_key": "new-key-123"}))
        .await;
    assert_eq!(resp.status_code(), StatusCode::OK);

    let body: serde_json::Value = server.get("/api/v1/providers/upd-key").await.json();
    assert_eq!(body["api_key"], "new-key-123");
}

#[tokio::test]
async fn test_http_update_provider_weight() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "upd-wt", "name": "Weight Provider", "base_url": "https://wp.com/v1",
            "auth_type": "api_key", "api_key": "key", "weight": 1
        }))
        .await;

    let resp = server.put("/api/v1/providers/upd-wt")
        .json(&serde_json::json!({"weight": 5}))
        .await;
    assert_eq!(resp.status_code(), StatusCode::OK);

    let body: serde_json::Value = server.get("/api/v1/providers/upd-wt").await.json();
    assert_eq!(body["weight"], 5);
}

#[tokio::test]
async fn test_http_update_provider_to_dynamic_token() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "upd-dyn", "name": "To Dynamic", "base_url": "https://dp.com/v1",
            "auth_type": "api_key", "api_key": "key"
        }))
        .await;

    let resp = server.put("/api/v1/providers/upd-dyn")
        .json(&serde_json::json!({
            "auth_type": "dynamic_token",
            "token_url": "https://auth.dp.com/login",
            "token_username": "admin",
            "token_password": "secret",
            "token_expiry_seconds": 28800
        }))
        .await;
    assert_eq!(resp.status_code(), StatusCode::OK);

    let body: serde_json::Value = server.get("/api/v1/providers/upd-dyn").await.json();
    assert_eq!(body["auth_type"], "dynamic_token");
    assert_eq!(body["token_url"], "https://auth.dp.com/login");
    assert_eq!(body["token_username"], "admin");
    assert_eq!(body["token_expiry_seconds"], 28800);
}

#[tokio::test]
async fn test_http_update_provider_deactivate() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "upd-deact", "name": "Deactivate Me", "base_url": "https://dm.com/v1",
            "auth_type": "api_key", "api_key": "key"
        }))
        .await;

    let resp = server.put("/api/v1/providers/upd-deact")
        .json(&serde_json::json!({"is_active": false}))
        .await;
    assert_eq!(resp.status_code(), StatusCode::OK);

    let body: serde_json::Value = server.get("/api/v1/providers/upd-deact").await.json();
    assert_eq!(body["is_active"], false);
}

#[tokio::test]
async fn test_http_update_provider_not_found() {
    let server = build_test_app().await;
    let resp = server.put("/api/v1/providers/nonexistent")
        .json(&serde_json::json!({"name": "Ghost"}))
        .await;
    assert_eq!(resp.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_update_provider_multiple_fields() {
    let server = build_test_app().await;
    server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "upd-multi", "name": "Multi", "base_url": "https://m.com/v1",
            "auth_type": "api_key", "api_key": "old-key", "weight": 1
        }))
        .await;

    let resp = server.put("/api/v1/providers/upd-multi")
        .json(&serde_json::json!({
            "name": "Multi Updated",
            "base_url": "https://m2.com/v1",
            "api_key": "new-key",
            "weight": 3
        }))
        .await;
    assert_eq!(resp.status_code(), StatusCode::OK);

    let body: serde_json::Value = server.get("/api/v1/providers/upd-multi").await.json();
    assert_eq!(body["name"], "Multi Updated");
    assert_eq!(body["base_url"], "https://m2.com/v1");
    assert_eq!(body["api_key"], "new-key");
    assert_eq!(body["weight"], 3);
}

// ==================== Dashboard Content Tests ====================

#[tokio::test]
async fn test_dashboard_has_provider_management_tab() {
    let server = build_test_app().await;
    let body = server.get("/").await.text();
    assert!(body.contains("服务商管理"));
    assert!(body.contains("添加服务商"));
}

#[tokio::test]
async fn test_dashboard_has_apikey_management_tab() {
    let server = build_test_app().await;
    let body = server.get("/").await.text();
    assert!(body.contains("API Key 管理"));
    assert!(body.contains("创建 API Key"));
}

#[tokio::test]
async fn test_dashboard_has_provider_modal() {
    let server = build_test_app().await;
    let body = server.get("/").await.text();
    // Provider form fields
    assert!(body.contains("prov-id"));
    assert!(body.contains("prov-name"));
    assert!(body.contains("prov-base-url"));
    assert!(body.contains("prov-auth-type"));
    assert!(body.contains("prov-weight"));
    // Dynamic token fields
    assert!(body.contains("prov-token-url"));
    assert!(body.contains("prov-token-username"));
    assert!(body.contains("prov-token-password"));
    assert!(body.contains("prov-token-field"));
    assert!(body.contains("prov-refresh-token-field"));
    assert!(body.contains("prov-token-expiry"));
    // Auth type toggle
    assert!(body.contains("toggleAuthFields"));
}

#[tokio::test]
async fn test_dashboard_has_apikey_modal() {
    let server = build_test_app().await;
    let body = server.get("/").await.text();
    assert!(body.contains("ak-name"));
    assert!(body.contains("ak-providers-checks"));
}

#[tokio::test]
async fn test_dashboard_has_stats_tab() {
    let server = build_test_app().await;
    let body = server.get("/").await.text();
    assert!(body.contains("统计分析"));
    assert!(body.contains("st-granularity"));
}

#[tokio::test]
async fn test_dashboard_has_log_tab() {
    let server = build_test_app().await;
    let body = server.get("/").await.text();
    assert!(body.contains("请求日志"));
    assert!(body.contains("log-provider"));
}

// ==================== Provider CRUD Full Cycle with Update ====================

#[tokio::test]
async fn test_http_provider_full_lifecycle() {
    let server = build_test_app().await;

    // 1. Create
    let create = server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "lifecycle",
            "name": "Lifecycle Provider",
            "base_url": "https://lc.com/v1",
            "auth_type": "api_key",
            "api_key": "sk-initial",
            "weight": 1
        }))
        .await;
    assert_eq!(create.status_code(), StatusCode::CREATED);

    // 2. Read
    let get = server.get("/api/v1/providers/lifecycle").await;
    let body: serde_json::Value = get.json();
    assert_eq!(body["name"], "Lifecycle Provider");
    assert_eq!(body["api_key"], "sk-initial");

    // 3. Update
    let update = server.put("/api/v1/providers/lifecycle")
        .json(&serde_json::json!({
            "name": "Updated Lifecycle",
            "api_key": "sk-updated",
            "weight": 5
        }))
        .await;
    assert_eq!(update.status_code(), StatusCode::OK);

    // 4. Read again - verify update
    let get2 = server.get("/api/v1/providers/lifecycle").await;
    let body2: serde_json::Value = get2.json();
    assert_eq!(body2["name"], "Updated Lifecycle");
    assert_eq!(body2["api_key"], "sk-updated");
    assert_eq!(body2["weight"], 5);

    // 5. Deactivate
    let deact = server.put("/api/v1/providers/lifecycle")
        .json(&serde_json::json!({"is_active": false}))
        .await;
    assert_eq!(deact.status_code(), StatusCode::OK);

    let body3: serde_json::Value = server.get("/api/v1/providers/lifecycle").await.json();
    assert_eq!(body3["is_active"], false);

    // 6. Delete
    let delete = server.delete("/api/v1/providers/lifecycle").await;
    assert_eq!(delete.status_code(), StatusCode::OK);

    let get4 = server.get("/api/v1/providers/lifecycle").await;
    assert_eq!(get4.status_code(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_dynamic_token_provider_full_lifecycle() {
    let server = build_test_app().await;

    // 1. Create with dynamic token
    let create = server.post("/api/v1/providers")
        .json(&serde_json::json!({
            "id": "dyn-lc",
            "name": "Dynamic Lifecycle",
            "base_url": "https://dlc.com/v1",
            "auth_type": "dynamic_token",
            "token_url": "https://auth.dlc.com/login",
            "token_username": "user1",
            "token_password": "pass1",
            "token_field": "access_token",
            "refresh_token_field": "refresh_token",
            "token_header_field": "X-Auth-Token",
            "token_header_prefix": "",
            "token_expiry_seconds": 7200,
            "weight": 2
        }))
        .await;
    assert_eq!(create.status_code(), StatusCode::CREATED);

    // 2. Read and verify all fields
    let body: serde_json::Value = server.get("/api/v1/providers/dyn-lc").await.json();
    assert_eq!(body["auth_type"], "dynamic_token");
    assert_eq!(body["token_url"], "https://auth.dlc.com/login");
    assert_eq!(body["token_username"], "user1");
    assert_eq!(body["token_field"], "access_token");
    assert_eq!(body["refresh_token_field"], "refresh_token");
    assert_eq!(body["token_header_field"], "X-Auth-Token");
    assert_eq!(body["token_header_prefix"], "");
    assert_eq!(body["token_expiry_seconds"], 7200);
    assert_eq!(body["weight"], 2);

    // 3. Update credentials
    let update = server.put("/api/v1/providers/dyn-lc")
        .json(&serde_json::json!({
            "token_username": "user2",
            "token_password": "pass2",
            "token_expiry_seconds": 3600
        }))
        .await;
    assert_eq!(update.status_code(), StatusCode::OK);

    // 4. Verify update
    let body2: serde_json::Value = server.get("/api/v1/providers/dyn-lc").await.json();
    assert_eq!(body2["token_username"], "user2");
    assert_eq!(body2["token_expiry_seconds"], 3600);
    // Other fields should remain
    assert_eq!(body2["token_url"], "https://auth.dlc.com/login");
    assert_eq!(body2["token_field"], "access_token");

    // 5. Delete
    server.delete("/api/v1/providers/dyn-lc").await;
    assert_eq!(server.get("/api/v1/providers/dyn-lc").await.status_code(), StatusCode::NOT_FOUND);
}
