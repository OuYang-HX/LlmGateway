use llm_gateway::{config, db, auth, proxy, stats, api, dashboard, AppState};
use llm_gateway::auth::token_refresh::TokenRefreshTask;

use axum::{
    Router,
    routing::{get, post, delete, put},
    http::Method,
};
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "llm_gateway=info,tower_http=info".into())
        )
        .init();

    // Load configuration
    let config = load_config()?;

    // Initialize database
    let database = db::Database::new(&config.database.url).await?;
    let db = Arc::new(database);

    // Initialize components
    let auth_manager = Arc::new(auth::AuthManager::new(db.clone()));
    let llm_proxy = Arc::new(proxy::LlmProxy::new(db.clone(), auth_manager.clone()));
    let stats_collector = Arc::new(stats::StatsCollector::new(db.clone()));

    // Start background tasks
    stats_collector.start_snapshot_task(10); // Snapshot every 10 seconds
    stats_collector.start_cleanup_task(300);  // Cleanup every 5 minutes

    // Start token refresh task
    let token_refresh = Arc::new(TokenRefreshTask::new(db.clone(), auth_manager.clone()));
    token_refresh.clone().start(60); // Check every 60 seconds

    // Create combined app state
    let state = AppState {
        db: db.clone(),
        proxy: llm_proxy.clone(),
        auth_manager: auth_manager.clone(),
        stats_collector: stats_collector.clone(),
    };

    // CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        // Dashboard
        .route("/", get(dashboard::dashboard_page))
        .route("/dashboard", get(dashboard::dashboard_page))

        // API Key management
        .route("/api/v1/api-keys", post(api::create_api_key).get(api::list_api_keys))
        .route("/api/v1/api-keys/:id", get(api::get_api_key).delete(api::delete_api_key).post(api::regenerate_api_key))

        // Provider management
        .route("/api/v1/providers", post(api::create_provider).get(api::list_providers))
        .route("/api/v1/providers/:id", get(api::get_provider).delete(api::delete_provider).put(api::update_provider))
        .route("/api/v1/providers/:id/refresh-token", post(api::refresh_provider_token))
        .route("/api/v1/providers/:id/models", post(api::add_provider_model).get(api::list_provider_models))
        .route("/api/v1/providers/:id/models/:model_id", delete(api::remove_provider_model))
        .route("/api/v1/providers/:id/models/:model_id/test", post(api::test_provider_model))
        .route("/api/v1/providers/:id/models/test-all", post(api::test_all_provider_models))

        // Unified Model management
        .route("/api/v1/models", post(api::create_model).get(api::list_models))
        .route("/api/v1/models/:id", get(api::get_model).delete(api::delete_model).put(api::update_model))
        .route("/api/v1/models/:id/mappings", get(api::list_model_mappings))
        .route("/api/v1/models/:id/mappings", post(api::add_model_mapping))
        .route("/api/v1/models/:id/mappings/:provider_id", put(api::update_model_mapping).delete(api::remove_model_mapping))

        // Statistics
        .route("/api/v1/stats", get(api::get_stats))
        .route("/api/v1/stats/bucketed", get(api::get_time_bucketed_stats))

        // Request logs
        .route("/api/v1/logs", get(api::get_request_logs))
        .route("/api/v1/logs/:id", get(api::get_request_log_detail))

        // Dashboard API
        .route("/api/v1/dashboard/summary", get(api::get_dashboard_summary))
        .route("/api/v1/dashboard/health", get(api::get_provider_health))
        .route("/api/v1/dashboard/token-rate", get(api::get_token_rate))

        // WebSocket proxy
        .route("/ws/v1", get(proxy::ws_handler::ws_proxy_handler))

        // LLM Proxy (catch-all for /v1/* paths)
        .route("/v1/*path", post(proxy::proxy_request))
        .route("/v1/*path", get(proxy::proxy_request))

        // Apply state and middleware
        .with_state(state)
        .layer(cors);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("🚀 LLM Gateway starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn load_config() -> anyhow::Result<config::AppConfig> {
    let mut cfg = config_crate::Config::builder()
        .set_default("server.host", "0.0.0.0")?
        .set_default("server.port", 49127)?
        .set_default("database.url", "sqlite:llm_gateway.db")?;

    if std::path::Path::new("config.toml").exists() {
        cfg = cfg.add_source(config_crate::File::with_name("config"));
    }

    cfg = cfg.add_source(
        config_crate::Environment::with_prefix("LLM_GW")
            .separator("_")
    );

    let app_config: config::AppConfig = cfg.build()?.try_deserialize()?;
    Ok(app_config)
}
