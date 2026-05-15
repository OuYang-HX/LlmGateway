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
    let allowed_providers_json = req.allowed_providers.as_ref()
        .map(|p| serde_json::to_string(p).unwrap_or_default());

    match state.db.create_api_key(
        &id,
        &req.name,
        &raw_key,
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
                    key: k.api_key,
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
                key: k.api_key,
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

/// Regenerate an API key - old key becomes invalid, new key is returned
pub async fn regenerate_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Check the key exists
    let existing = match state.db.get_api_key_by_id(&id).await {
        Ok(Some(k)) => k,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "API key not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get API key: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let raw_key = format!("lgk-{}", Uuid::new_v4().to_string().replace('-', ""));
    let key_prefix = raw_key[..12].to_string();

    match state.db.regenerate_api_key(&id, &raw_key, &key_prefix).await {
        Ok(true) => {
            let response = ApiKeyResponse {
                id: id.clone(),
                name: existing.name,
                key: raw_key,
                key_prefix,
                allowed_providers: existing.allowed_providers
                    .map(|s| serde_json::from_str(&s).unwrap_or_default()),
                is_active: existing.is_active,
                created_at: existing.created_at,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "API key not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to regenerate API key: {}", e);
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
    #[serde(default = "default_post")]
    pub token_request_method: String,
    #[serde(default = "default_json")]
    pub token_content_type: String,
    #[serde(default = "default_username_field")]
    pub token_username_field: String,
    #[serde(default = "default_password_field")]
    pub token_password_field: String,
    pub token_body_template: Option<String>,
    #[serde(default)]
    pub token_extra_headers: Option<String>,
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
fn default_post() -> String { "POST".to_string() }
fn default_json() -> String { "json".to_string() }
fn default_username_field() -> String { "username".to_string() }
fn default_password_field() -> String { "password".to_string() }

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
        Some(req.token_request_method.as_str()),
        Some(req.token_content_type.as_str()),
        Some(req.token_username_field.as_str()),
        Some(req.token_password_field.as_str()),
        req.token_body_template.as_deref(),
        req.token_extra_headers.as_deref(),
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

/// Update a provider
#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub api_type: Option<String>,
    pub auth_type: Option<String>,
    pub api_key: Option<String>,
    pub token_url: Option<String>,
    pub token_username: Option<String>,
    pub token_password: Option<String>,
    pub token_request_method: Option<String>,
    pub token_content_type: Option<String>,
    pub token_username_field: Option<String>,
    pub token_password_field: Option<String>,
    pub token_body_template: Option<String>,
    pub token_extra_headers: Option<String>,
    pub token_field: Option<String>,
    pub refresh_token_field: Option<String>,
    pub token_header_field: Option<String>,
    pub token_header_prefix: Option<String>,
    pub token_expiry_seconds: Option<i64>,
    pub weight: Option<i32>,
    pub is_active: Option<bool>,
}

pub async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProviderRequest>,
) -> impl IntoResponse {
    // First get the existing provider
    let existing = match state.db.get_provider(&id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get provider: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let base_url = req.base_url.as_deref().unwrap_or(&existing.base_url);
    let api_type = req.api_type.as_deref().unwrap_or(&existing.api_type);
    let auth_type = req.auth_type.as_deref().unwrap_or(&existing.auth_type);
    let api_key = req.api_key.as_deref().or(existing.api_key.as_deref());
    let token_url = req.token_url.as_deref().or(existing.token_url.as_deref());
    let token_username = req.token_username.as_deref().or(existing.token_username.as_deref());
    let token_password = req.token_password.as_deref().or(existing.token_password.as_deref());
    let token_request_method = req.token_request_method.as_deref()
        .or(existing.token_request_method.as_deref())
        .unwrap_or("POST");
    let token_content_type = req.token_content_type.as_deref()
        .or(existing.token_content_type.as_deref())
        .unwrap_or("json");
    let token_username_field = req.token_username_field.as_deref()
        .or(existing.token_username_field.as_deref())
        .unwrap_or("username");
    let token_password_field = req.token_password_field.as_deref()
        .or(existing.token_password_field.as_deref())
        .unwrap_or("password");
    let token_body_template = req.token_body_template.as_deref()
        .or(existing.token_body_template.as_deref());
    let token_extra_headers = req.token_extra_headers.as_deref()
        .or(existing.token_extra_headers.as_deref());
    let token_field = req.token_field.as_deref().unwrap_or(&existing.token_field);
    let refresh_token_field = req.refresh_token_field.as_deref().unwrap_or(&existing.refresh_token_field);
    let token_header_field = req.token_header_field.as_deref().unwrap_or(&existing.token_header_field);
    let token_header_prefix = req.token_header_prefix.as_deref().unwrap_or(&existing.token_header_prefix);
    let token_expiry_seconds = req.token_expiry_seconds.unwrap_or(existing.token_expiry_seconds);
    let weight = req.weight.unwrap_or(existing.weight);
    let is_active = req.is_active.unwrap_or(existing.is_active);

    match state.db.update_provider(
        &id, name, base_url, api_type, auth_type,
        api_key, token_url, token_username, token_password,
        Some(token_request_method), Some(token_content_type),
        Some(token_username_field), Some(token_password_field),
        token_body_template,
        token_extra_headers,
        token_field, refresh_token_field, token_header_field, token_header_prefix,
        token_expiry_seconds, weight, is_active,
    ).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"id": id}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to update provider: {}", e);
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
