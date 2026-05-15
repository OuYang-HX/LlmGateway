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
        .route("/api/v1/providers/:id", get(llm_gateway::api::get_provider).delete(llm_gateway::api::delete_provider))
        .route("/api/v1/stats", get(llm_gateway::api::get_stats))
        .route("/api/v1/stats/bucketed", get(llm_gateway::api::get_time_bucketed_stats))
        .route("/api/v1/logs", get(llm_gateway::api::get_request_logs))
        .route("/api/v1/dashboard/summary", get(llm_gateway::api::get_dashboard_summary))
        .route("/api/v1/dashboard/token-rate", get(llm_gateway::api::get_token_rate))
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
    assert!(body.contains("Overview"));
    assert!(body.contains("Real-time Rate"));
    assert!(body.contains("Request Logs"));
}

#[tokio::test]
async fn test_dashboard_page_explicit_path() {
    let server = build_test_app().await;
    let response = server.get("/dashboard").await;
    assert_eq!(response.status_code(), StatusCode::OK);
    let body = response.text();
    assert!(body.contains("LLM Gateway Dashboard"));
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
