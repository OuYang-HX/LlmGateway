/// Dashboard module - serves the web UI and provides dashboard API endpoints
use axum::response::{Html, IntoResponse};

/// Returns the dashboard HTML page
pub async fn dashboard_page() -> impl IntoResponse {
    Html(DASHBOARD_HTML)
}

const DASHBOARD_HTML: &str = include_str!("../dashboard.html");
