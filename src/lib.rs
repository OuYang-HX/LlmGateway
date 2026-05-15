pub mod config;
pub mod db;
pub mod auth;
pub mod proxy;
pub mod stats;
pub mod api;
pub mod dashboard;
pub mod utils;
pub mod usage;

use std::sync::Arc;

/// Combined application state that includes all sub-states
#[derive(Debug, Clone)]
pub struct AppState {
    pub db: Arc<db::Database>,
    pub proxy: Arc<proxy::LlmProxy>,
    pub auth_manager: Arc<auth::AuthManager>,
    pub stats_collector: Arc<stats::StatsCollector>,
}
