use llm_gateway::{config, db, auth, proxy, stats, api, dashboard, AppState};

use axum::{
    Router,
    routing::{get, post, delete},
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
        .route("/api/v1/api-keys", post(api::create_api_key))
        .route("/api/v1/api-keys", get(api::list_api_keys))
        .route("/api/v1/api-keys/{id}", get(api::get_api_key))
        .route("/api/v1/api-keys/{id}", delete(api::delete_api_key))

        // Provider management
        .route("/api/v1/providers", post(api::create_provider))
        .route("/api/v1/providers", get(api::list_providers))
        .route("/api/v1/providers/{id}", get(api::get_provider))
        .route("/api/v1/providers/{id}", delete(api::delete_provider))

        // Statistics
        .route("/api/v1/stats", get(api::get_stats))
        .route("/api/v1/stats/bucketed", get(api::get_time_bucketed_stats))

        // Request logs
        .route("/api/v1/logs", get(api::get_request_logs))

        // Dashboard API
        .route("/api/v1/dashboard/summary", get(api::get_dashboard_summary))
        .route("/api/v1/dashboard/token-rate", get(api::get_token_rate))

        // LLM Proxy (catch-all for /v1/* paths)
        .route("/v1/{*path}", post(proxy::proxy_request))
        .route("/v1/{*path}", get(proxy::proxy_request))

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
        .set_default("server.port", 3000)?
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
