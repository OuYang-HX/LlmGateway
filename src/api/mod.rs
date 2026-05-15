use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
    response::IntoResponse,
};
use crate::config::*;
use crate::AppState;
use uuid::Uuid;
use serde::Deserialize;

// ========== API Key handlers ==========

/// Create a new API key
pub async fn create_api_key(
    State(state): State<AppState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4().to_string();
    let raw_key = format!("lgk-{}", Uuid::new_v4().to_string().replace('-', ""));
    let key_prefix = raw_key[..12].to_string();
    let key_hash = crate::utils::sha256_hash(&raw_key);
    let allowed_providers_json = req.allowed_providers.as_ref()
        .map(|p| serde_json::to_string(p).unwrap_or_default());

    match state.db.create_api_key(
        &id,
        &req.name,
        &key_hash,
        &key_prefix,
        allowed_providers_json.as_deref(),
    ).await {
        Ok(()) => {
            let response = ApiKeyResponse {
                id,
                name: req.name,
                key: raw_key,
                key_prefix,
                allowed_providers: req.allowed_providers,
                is_active: true,
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create API key: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// List all API keys
pub async fn list_api_keys(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.list_api_keys().await {
        Ok(keys) => {
            let responses: Vec<ApiKeyResponse> = keys.into_iter().map(|k| {
                let allowed_providers = k.allowed_providers.as_ref()
                    .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok());
                ApiKeyResponse {
                    id: k.id,
                    name: k.name,
                    key: "***".to_string(),
                    key_prefix: k.key_prefix,
                    allowed_providers,
                    is_active: k.is_active,
                    created_at: k.created_at,
                }
            }).collect();
            Json(responses).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list API keys: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get a specific API key
pub async fn get_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_api_key_by_id(&id).await {
        Ok(Some(k)) => {
            let allowed_providers = k.allowed_providers.as_ref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok());
            let response = ApiKeyResponse {
                id: k.id,
                name: k.name,
                key: "***".to_string(),
                key_prefix: k.key_prefix,
                allowed_providers,
                is_active: k.is_active,
                created_at: k.created_at,
            };
            Json(response).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "API key not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get API key: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Delete an API key
pub async fn delete_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_api_key(&id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"message": "API key deleted"}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "API key not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete API key: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

// ========== Provider handlers ==========

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default = "default_api_type")]
    pub api_type: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    pub api_key: Option<String>,
    pub token_url: Option<String>,
    pub token_username: Option<String>,
    pub token_password: Option<String>,
    #[serde(default = "default_token_field")]
    pub token_field: String,
    #[serde(default = "default_refresh_token_field")]
    pub refresh_token_field: String,
    #[serde(default = "default_token_header_field")]
    pub token_header_field: String,
    #[serde(default = "default_token_header_prefix")]
    pub token_header_prefix: String,
    #[serde(default = "default_token_expiry")]
    pub token_expiry_seconds: i64,
    #[serde(default = "default_weight")]
    pub weight: i32,
}

fn default_api_type() -> String { "openai".to_string() }
fn default_auth_type() -> String { "api_key".to_string() }
fn default_token_field() -> String { "token".to_string() }
fn default_refresh_token_field() -> String { "refreshToken".to_string() }
fn default_token_header_field() -> String { "Authorization".to_string() }
fn default_token_header_prefix() -> String { "Bearer ".to_string() }
fn default_token_expiry() -> i64 { 86400 }
fn default_weight() -> i32 { 1 }

/// Create a new provider
pub async fn create_provider(
    State(state): State<AppState>,
    Json(req): Json<CreateProviderRequest>,
) -> impl IntoResponse {
    match state.db.create_provider(
        &req.id,
        &req.name,
        &req.base_url,
        &req.api_type,
        &req.auth_type,
        req.api_key.as_deref(),
        req.token_url.as_deref(),
        req.token_username.as_deref(),
        req.token_password.as_deref(),
        &req.token_field,
        &req.refresh_token_field,
        &req.token_header_field,
        &req.token_header_prefix,
        req.token_expiry_seconds,
        req.weight,
    ).await {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({"id": req.id}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to create provider: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// List all providers
pub async fn list_providers(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.list_providers().await {
        Ok(providers) => Json(providers).into_response(),
        Err(e) => {
            tracing::error!("Failed to list providers: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get a specific provider
pub async fn get_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_provider(&id).await {
        Ok(Some(p)) => Json(p).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get provider: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Delete a provider
pub async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_provider(&id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"message": "Provider deleted"}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete provider: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

// ========== Statistics handlers ==========

#[derive(Debug, Deserialize)]
pub struct StatsParams {
    pub api_key_id: Option<String>,
    pub provider_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub granularity: Option<String>,
}

/// Get statistics
pub async fn get_stats(
    State(state): State<AppState>,
    Query(params): Query<StatsParams>,
) -> impl IntoResponse {
    match state.db.get_stats(
        params.api_key_id.as_deref(),
        params.provider_id.as_deref(),
        params.start_time.as_deref(),
        params.end_time.as_deref(),
    ).await {
        Ok(stats) => Json(serde_json::json!({
            "total_requests": stats.total_requests,
            "total_prompt_tokens": stats.total_prompt_tokens,
            "total_completion_tokens": stats.total_completion_tokens,
            "total_tokens": stats.total_tokens,
            "avg_duration_ms": stats.avg_duration_ms,
            "throttle_count": stats.throttle_count,
            "error_count": stats.error_count,
        })).into_response(),
        Err(e) => {
            tracing::error!("Failed to get stats: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get time-bucketed statistics
pub async fn get_time_bucketed_stats(
    State(state): State<AppState>,
    Query(params): Query<StatsParams>,
) -> impl IntoResponse {
    let start_time = params.start_time.as_deref().unwrap_or("2000-01-01 00:00:00");
    let end_time = params.end_time.as_deref().unwrap_or("2099-12-31 23:59:59");
    let granularity = params.granularity.as_deref().unwrap_or("day");

    match state.db.get_time_bucketed_stats(
        params.api_key_id.as_deref(),
        params.provider_id.as_deref(),
        start_time,
        end_time,
        granularity,
    ).await {
        Ok(buckets) => Json(buckets).into_response(),
        Err(e) => {
            tracing::error!("Failed to get time-bucketed stats: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

// ========== Request log handlers ==========

#[derive(Debug, Deserialize)]
pub struct RequestLogParams {
    pub api_key_id: Option<String>,
    pub provider_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// Query request logs
pub async fn get_request_logs(
    State(state): State<AppState>,
    Query(params): Query<RequestLogParams>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);
    let offset = ((page - 1) * page_size) as i64;

    match state.db.query_request_logs(
        params.api_key_id.as_deref(),
        params.provider_id.as_deref(),
        params.start_time.as_deref(),
        params.end_time.as_deref(),
        page_size as i64,
        offset,
    ).await {
        Ok(logs) => {
            let total = state.db.count_request_logs(
                params.api_key_id.as_deref(),
                params.provider_id.as_deref(),
                params.start_time.as_deref(),
                params.end_time.as_deref(),
            ).await.unwrap_or(0);

            Json(serde_json::json!({
                "logs": logs,
                "total": total,
                "page": page,
                "page_size": page_size,
            })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get request logs: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

// ========== Dashboard handlers ==========

/// Get dashboard summary
pub async fn get_dashboard_summary(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.get_dashboard_summary().await {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => {
            tracing::error!("Failed to get dashboard summary: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get token rate data
pub async fn get_token_rate(
    State(state): State<AppState>,
    Query(params): Query<TokenRateParams>,
) -> impl IntoResponse {
    let one_hour_ago = chrono::Utc::now() - chrono::Duration::hours(1);
    let start_time = one_hour_ago.format("%Y-%m-%d %H:%M:%S").to_string();

    match state.db.get_token_rate_snapshots(
        params.provider_id.as_deref(),
        &start_time,
        3600,
    ).await {
        Ok(snapshots) => Json(snapshots).into_response(),
        Err(e) => {
            tracing::error!("Failed to get token rate: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenRateParams {
    pub provider_id: Option<String>,
}
