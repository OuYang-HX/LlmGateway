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

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub name: Option<String>,
    pub allowed_providers: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

/// Update an API key (name, allowed_providers, is_active)
pub async fn update_api_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateApiKeyRequest>,
) -> impl IntoResponse {
    let existing = match state.db.get_api_key_by_id(&id).await {
        Ok(Some(k)) => k,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "API key not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get API key: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let name = req.name.unwrap_or(existing.name);
    let allowed_providers = req.allowed_providers
        .as_ref()
        .map(|v| serde_json::to_string(v).ok())
        .unwrap_or(existing.allowed_providers.clone());
    let is_active = req.is_active.unwrap_or(existing.is_active);

    match state.db.update_api_key(&id, &name, allowed_providers.as_deref(), is_active).await {
        Ok(true) => {
            let response = ApiKeyResponse {
                id: id.clone(),
                name: name.clone(),
                key: existing.api_key,
                key_prefix: existing.key_prefix,
                allowed_providers: allowed_providers
                    .as_ref()
                    .and_then(|s| serde_json::from_str(s).ok()),
                is_active,
                created_at: existing.created_at,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "API key not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to update API key: {}", e);
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
    pub token_cookies: Option<String>,
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
    pub weight: i64,
    #[serde(default)]
    pub bypass_proxy: bool,
    #[serde(default = "default_response_content_path")]
    pub response_content_path: String,
    #[serde(default = "default_response_reasoning_path")]
    pub response_reasoning_path: String,
    pub subscription_start: Option<String>,
    #[serde(default)]
    pub mock_mode: bool,
    pub group_id: Option<String>,
}

fn default_api_type() -> String { "openai".to_string() }
fn default_auth_type() -> String { "api_key".to_string() }
fn default_token_field() -> String { "token".to_string() }
fn default_refresh_token_field() -> String { "refreshToken".to_string() }
fn default_token_header_field() -> String { "Authorization".to_string() }
fn default_token_header_prefix() -> String { "Bearer ".to_string() }
fn default_token_expiry() -> i64 { 86400 }
fn default_weight() -> i64 { 1 }
fn default_cost_multiplier() -> f64 { 1.0 }
fn default_post() -> String { "POST".to_string() }
fn default_json() -> String { "json".to_string() }
fn default_username_field() -> String { "username".to_string() }
fn default_password_field() -> String { "password".to_string() }
fn default_response_content_path() -> String { "choices.0.message.content".to_string() }
fn default_response_reasoning_path() -> String { "choices.0.delta.reasoning_content".to_string() }

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
        req.token_cookies.as_deref(),
        &req.token_field,
        &req.refresh_token_field,
        &req.token_header_field,
        &req.token_header_prefix,
        req.token_expiry_seconds,
        req.weight,
        req.bypass_proxy,
        req.mock_mode,
        &req.response_content_path,
        &req.response_reasoning_path,
    ).await {
        Ok(()) => {
            // Set group_id after creation if provided
            if let Some(ref gid) = req.group_id {
                let _ = state.db.update_provider_group_id(&req.id, Some(gid.as_str())).await;
            }
            (StatusCode::CREATED, Json(serde_json::json!({"id": req.id}))).into_response()
        }
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

#[derive(Deserialize)]
pub struct BatchUpdateApiKeyStatusRequest {
    pub ids: Vec<String>,
    pub enable: bool,
}

/// Batch enable or disable API keys
pub async fn batch_update_api_key_status(
    State(state): State<AppState>,
    Json(req): Json<BatchUpdateApiKeyStatusRequest>,
) -> impl IntoResponse {
    if req.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "ids cannot be empty"}))).into_response();
    }
    match state.db.batch_set_api_key_active_status(&req.ids, req.enable).await {
        Ok(count) => (StatusCode::OK, Json(serde_json::json!({"updated": count}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to batch update API key status: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct BatchUpdateProviderStatusRequest {
    pub ids: Vec<String>,
    pub enable: bool,
}

/// Batch enable or disable providers
pub async fn batch_update_provider_status(
    State(state): State<AppState>,
    Json(req): Json<BatchUpdateProviderStatusRequest>,
) -> impl IntoResponse {
    if req.ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "ids cannot be empty"}))).into_response();
    }
    match state.db.batch_set_provider_active_status(&req.ids, req.enable).await {
        Ok(count) => (StatusCode::OK, Json(serde_json::json!({"updated": count}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to batch update provider status: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Manually refresh a provider's token
pub async fn refresh_provider_token(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let provider = match state.db.get_provider(&id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get provider: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    match state.auth_manager.get_auth_header(&provider).await {
        Ok((header_name, header_value)) => {
            // Re-fetch to get updated token/cookies
            let updated = state.db.get_provider(&id).await.ok().flatten();
            let token = updated.as_ref().and_then(|p| p.current_token.as_deref()).unwrap_or("");
            let cookies = updated.as_ref().and_then(|p| p.token_cookies.as_deref()).unwrap_or("");
            let expires = updated.as_ref().and_then(|p| p.token_expires_at.as_deref()).unwrap_or("");
            (StatusCode::OK, Json(serde_json::json!({
                "message": "Token refreshed",
                "token_preview": format!("{}...{}", &token[..token.len().min(8)], if token.len() > 8 { &token[token.len()-4..] } else { "" }),
                "cookies_updated": !cookies.is_empty(),
                "expires_at": expires,
                "header_name": header_name,
            }))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to refresh token for provider {}: {}", id, e);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("Token refresh failed: {}", e)}))).into_response()
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
    pub token_cookies: Option<String>,
    pub token_field: Option<String>,
    pub refresh_token_field: Option<String>,
    pub token_header_field: Option<String>,
    pub token_header_prefix: Option<String>,
    pub token_expiry_seconds: Option<i64>,
    pub weight: Option<i64>,
    pub bypass_proxy: Option<bool>,
    pub is_active: Option<bool>,
    pub response_content_path: Option<String>,
    pub response_reasoning_path: Option<String>,
    pub chart_color: Option<String>,
    pub subscription_start: Option<String>,
    pub new_id: Option<String>,
    pub mock_mode: Option<bool>,
    pub group_id: Option<String>,
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
    let token_cookies = req.token_cookies.as_deref()
        .or(existing.token_cookies.as_deref());
    let token_field = req.token_field.as_deref().unwrap_or(&existing.token_field);
    let refresh_token_field = req.refresh_token_field.as_deref().unwrap_or(&existing.refresh_token_field);
    let token_header_field = req.token_header_field.as_deref().unwrap_or(&existing.token_header_field);
    let token_header_prefix = req.token_header_prefix.as_deref().unwrap_or(&existing.token_header_prefix);
    let token_expiry_seconds = req.token_expiry_seconds.unwrap_or(existing.token_expiry_seconds);
    let weight = req.weight.unwrap_or(existing.weight);
    let is_active = req.is_active.unwrap_or(existing.is_active);
    let bypass_proxy = req.bypass_proxy.unwrap_or(existing.bypass_proxy);
    let response_content_path = req.response_content_path.as_deref().unwrap_or(&existing.response_content_path);
    let response_reasoning_path = req.response_reasoning_path.as_deref().unwrap_or(&existing.response_reasoning_path);
    let chart_color = req.chart_color.as_deref().or(existing.chart_color.as_deref());
    let subscription_start = req.subscription_start.as_deref().or(existing.subscription_start.as_deref());
    let new_id = req.new_id.as_deref().unwrap_or(&existing.id);
    let mock_mode = req.mock_mode.unwrap_or(existing.mock_mode);

    // Check if new_id conflicts with an existing provider (when ID is changing)
    if new_id != id {
        if state.db.get_provider(new_id).await.ok().flatten().is_some() {
            return (StatusCode::CONFLICT, Json(serde_json::json!({"error": format!("Provider ID '{}' already exists", new_id)}))).into_response();
        }
    }

    match state.db.update_provider(
        &id, new_id, name, base_url, api_type, auth_type,
        api_key, token_url, token_username, token_password,
        Some(token_request_method), Some(token_content_type),
        Some(token_username_field), Some(token_password_field),
        token_body_template,
        token_extra_headers,
        token_cookies,
        token_field, refresh_token_field, token_header_field, token_header_prefix,
        token_expiry_seconds, weight, is_active, bypass_proxy,
        mock_mode,
        response_content_path,
        response_reasoning_path,
        chart_color,
        subscription_start,
        req.group_id.as_deref(),
    ).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"id": id}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to update provider: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

// ========== Provider Model handlers ==========

/// Add a model to a provider
pub async fn add_provider_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AddProviderModelRequest>,
) -> impl IntoResponse {
    // Verify provider exists
    if state.db.get_provider(&id).await.ok().flatten().is_none() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response();
    }

    match state.db.add_provider_model(&id, &req.model_id).await {
        Ok(()) => {
            let model = state.db.get_provider_model(&id, &req.model_id).await.ok().flatten();
            (StatusCode::CREATED, Json(serde_json::json!({"provider_id": id, "model_id": req.model_id, "model": model}))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to add provider model: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Remove a model from a provider
pub async fn remove_provider_model(
    State(state): State<AppState>,
    Path((id, model_id)): Path<(String, String)>,
) -> impl IntoResponse {
    // First, clean up any model_mappings that reference this provider+model
    // A provider_model can be mapped as provider_model_id in model_mappings.
    // We need to remove mappings where provider_id = this provider AND provider_model_id = this model.
    if let Err(e) = state.db.cleanup_mappings_for_provider_model(&id, &model_id).await {
        tracing::warn!("Failed to cleanup mappings for provider model {}/{}, error: {}", id, model_id, e);
        // Non-fatal: continue with model removal even if cleanup fails
    }

    match state.db.remove_provider_model(&id, &model_id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"message": "Model removed"}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Model not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to remove provider model: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// List all models for a provider
pub async fn list_provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.list_provider_models(&id).await {
        Ok(models) => Json(models).into_response(),
        Err(e) => {
            tracing::error!("Failed to list provider models: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Test connection for a specific model
pub async fn test_provider_model(
    State(state): State<AppState>,
    Path((id, model_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let provider = match state.db.get_provider(&id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get provider: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    // Verify model exists in provider's list
    if state.db.get_provider_model(&id, &model_id).await.ok().flatten().is_none() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Model not found in provider"}))).into_response();
    }

    // Build a minimal chat completion request to test the model
    let test_result = test_model_connection(&state, &provider, &model_id).await;

    let (status, message, is_active) = match &test_result {
        Ok(resp) => {
            if resp.status_code >= 200 && resp.status_code < 300 {
                ("success".to_string(), format!("连接成功 (HTTP {})", resp.status_code), true)
            } else {
                let body_str = String::from_utf8_lossy(&resp.body);
                let error_msg = serde_json::from_slice::<serde_json::Value>(&resp.body)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).map(|s| s.to_string()))
                    .unwrap_or_else(|| body_str.chars().take(200).collect());
                ("failed".to_string(), format!("HTTP {}: {}", resp.status_code, error_msg), false)
            }
        }
        Err(e) => ("failed".to_string(), e.to_string(), false),
    };

    // Update test status in database
    let _ = state.db.update_provider_model_test_status(
        &id, &model_id, &status, Some(&message), is_active,
    ).await;

    let response = serde_json::json!({
        "provider_id": id,
        "model_id": model_id,
        "status": status,
        "message": message,
        "is_active": is_active,
    });

    if status == "success" {
        (StatusCode::OK, Json(response)).into_response()
    } else {
        (StatusCode::BAD_REQUEST, Json(response)).into_response()
    }
}

/// Test all models for a provider
pub async fn test_all_provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let provider = match state.db.get_provider(&id).await {
        Ok(Some(p)) => p,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get provider: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let models = match state.db.list_provider_models(&id).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to list provider models: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let mut results = Vec::new();
    for model in &models {
        let test_result = test_model_connection(&state, &provider, &model.model_id).await;

        let (status, message, is_active) = match &test_result {
            Ok(resp) => {
                if resp.status_code >= 200 && resp.status_code < 300 {
                    ("success".to_string(), format!("连接成功 (HTTP {})", resp.status_code), true)
                } else {
                    let body_str = String::from_utf8_lossy(&resp.body);
                    let error_msg = serde_json::from_slice::<serde_json::Value>(&resp.body)
                        .ok()
                        .and_then(|v| v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).map(|s| s.to_string()))
                        .unwrap_or_else(|| body_str.chars().take(200).collect());
                    ("failed".to_string(), format!("HTTP {}: {}", resp.status_code, error_msg), false)
                }
            }
            Err(e) => ("failed".to_string(), e.to_string(), false),
        };

        let _ = state.db.update_provider_model_test_status(
            &id, &model.model_id, &status, Some(&message), is_active,
        ).await;

        results.push(serde_json::json!({
            "model_id": model.model_id,
            "status": status,
            "message": message,
            "is_active": is_active,
        }));
    }

    (StatusCode::OK, Json(serde_json::json!({
        "provider_id": id,
        "results": results,
    }))).into_response()
}

/// Helper: test model connection by sending a minimal chat completion request
async fn test_model_connection(
    state: &AppState,
    provider: &crate::db::ProviderRow,
    model_id: &str,
) -> Result<crate::proxy::ProxyResponse, String> {
    // Get auth header
    let (auth_header_name, auth_header_value) = state.auth_manager.get_auth_header(provider).await
        .map_err(|e| format!("Auth error: {}", e))?;

    // Build test request body
    let test_body = serde_json::json!({
        "model": model_id,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1
    });

    let base_url = provider.base_url.trim_end_matches('/');
    let target_url = format!("{}/chat/completions", base_url);

    let client = if provider.bypass_proxy {
        &state.proxy.no_proxy_client()
    } else {
        state.proxy.http_client()
    };

    let mut req_builder = client
        .post(&target_url)
        .header(&auth_header_name, &auth_header_value)
        .header("Content-Type", "application/json")
        .json(&test_body);

    // Add stored cookies from auth response
    if let Some(cookies) = &provider.token_cookies {
        if !cookies.is_empty() {
            req_builder = req_builder.header("Cookie", cookies.as_str());
        }
    }

    let response = req_builder
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Connection error: {}", e))?;

    let status_code = response.status().as_u16() as i32;
    let body = response.bytes().await.unwrap_or_default();

    Ok(crate::proxy::ProxyResponse {
        status_code,
        content_type: None,
        body,
    })
}

#[derive(Debug, Deserialize)]
pub struct AddProviderModelRequest {
    pub model_id: String,
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
    pub model: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub search: Option<String>,
    pub status_filter: Option<String>,  // "success", "error", or empty for all
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

    match state.db.query_request_logs_lightweight(
        params.api_key_id.as_deref(),
        params.provider_id.as_deref(),
        params.start_time.as_deref(),
        params.end_time.as_deref(),
        params.search.as_deref(),
        params.model.as_deref(),
        params.status_filter.as_deref(),
        page_size as i64,
        offset,
    ).await {
        Ok(logs) => {
            let total = state.db.count_request_logs(
                params.api_key_id.as_deref(),
                params.provider_id.as_deref(),
                params.start_time.as_deref(),
                params.end_time.as_deref(),
                params.search.as_deref(),
                params.model.as_deref(),
                params.status_filter.as_deref(),
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

/// Get a single request log by ID (for QA detail view)
pub async fn get_request_log_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_request_log(id).await {
        Ok(Some(log)) => Json(log).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Log not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get request log: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Delete a single request log by ID
pub async fn delete_request_log(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.delete_request_log(id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"message": "Log deleted"}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Log not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete request log: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteLogsRequest {
    pub provider_id: Option<String>,
    pub api_key_id: Option<String>,
}

/// Delete request logs by provider or API key (batch)
pub async fn delete_request_logs_batch(
    State(state): State<AppState>,
    Json(req): Json<DeleteLogsRequest>,
) -> impl IntoResponse {
    let result = match (req.provider_id, req.api_key_id) {
        (Some(provider_id), None) => state.db.delete_request_logs_by_provider(&provider_id).await,
        (None, Some(api_key_id)) => state.db.delete_request_logs_by_api_key(&api_key_id).await,
        _ => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Must specify exactly one of: provider_id or api_key_id"}))).into_response(),
    };

    match result {
        Ok(count) => (StatusCode::OK, Json(serde_json::json!({"deleted": count, "message": format!("Deleted {} log entries", count)}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete request logs: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteAllLogsParams {
    pub api_key_id: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub search: Option<String>,
    pub status_filter: Option<String>,
}

/// Delete request logs matching current search/filter criteria
/// If no filters provided, deletes all logs (same as before)
pub async fn delete_all_request_logs(
    State(state): State<AppState>,
    Query(params): Query<DeleteAllLogsParams>,
) -> impl IntoResponse {
    // If no filters are specified, delete everything
    let has_filters = params.provider_id.is_some()
        || params.api_key_id.is_some()
        || params.start_time.is_some()
        || params.end_time.is_some()
        || params.search.is_some()
        || params.model.is_some()
        || params.status_filter.is_some();

    let result = if has_filters {
        state.db.delete_request_logs_filtered(
            params.api_key_id.as_deref(),
            params.provider_id.as_deref(),
            params.start_time.as_deref(),
            params.end_time.as_deref(),
            params.search.as_deref(),
            params.model.as_deref(),
            params.status_filter.as_deref(),
        ).await
    } else {
        state.db.delete_all_request_logs().await
    };

    match result {
        Ok(count) => (StatusCode::OK, Json(serde_json::json!({"deleted": count, "message": format!("Deleted {} log entries", count)}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete request logs: {}", e);
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

/// Get provider health statistics (last 24h)
pub async fn get_provider_health(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.get_provider_health_stats().await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => {
            tracing::error!("Failed to get provider health: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get stats grouped by API key (for usage bar chart)
pub async fn get_stats_by_api_key(
    State(state): State<AppState>,
    Query(params): Query<StatsByApiKeyParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(10);
    match state.db.get_stats_by_api_key(
        limit,
        params.start_time.as_deref(),
        params.end_time.as_deref(),
    ).await {
        Ok(stats) => Json(stats).into_response(),
        Err(e) => {
            tracing::error!("Failed to get stats by API key: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get usage trend data (daily totals for the last 30 days)
pub async fn get_usage_trend(
    State(state): State<AppState>,
    Query(params): Query<UsageTrendParams>,
) -> impl IntoResponse {
    let days = params.days.unwrap_or(30).min(90);
    let end_time = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let start_time = (chrono::Utc::now() - chrono::Duration::days(days as i64))
        .format("%Y-%m-%dT%H:%M:%SZ").to_string();

    match state.db.get_time_bucketed_stats(
        params.api_key_id.as_deref(),
        params.provider_id.as_deref(),
        &start_time,
        &end_time,
        "day",
    ).await {
        Ok(stats) => {
            // Return just the fields needed for a trend chart
            let trend: Vec<serde_json::Value> = stats.into_iter().map(|s| {
                serde_json::json!({
                    "date": s.period,
                    "total_tokens": s.total_tokens,
                    "request_count": s.request_count,
                    "prompt_tokens": s.prompt_tokens,
                    "completion_tokens": s.completion_tokens,
                })
            }).collect();
            Json(trend).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get usage trend: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get token rate data
pub async fn get_token_rate(
    State(state): State<AppState>,
    Query(params): Query<TokenRateParams>,
) -> impl IntoResponse {
    let range = params.range.as_deref().unwrap_or("1h");
    let (start_time, limit) = match range {
        "5h" => {
            let t = chrono::Utc::now() - chrono::Duration::hours(5);
            (t.format("%Y-%m-%dT%H:%M:%SZ").to_string(), 18000)
        }
        "1d" => {
            let t = chrono::Utc::now() - chrono::Duration::days(1);
            (t.format("%Y-%m-%dT%H:%M:%SZ").to_string(), 86400)
        }
        "1w" => {
            let t = chrono::Utc::now() - chrono::Duration::weeks(1);
            (t.format("%Y-%m-%dT%H:%M:%SZ").to_string(), 604800)
        }
        "1m" => {
            let t = chrono::Utc::now() - chrono::Duration::days(30);
            (t.format("%Y-%m-%dT%H:%M:%SZ").to_string(), 2592000)
        }
        "all" => {
            ("2000-01-01T00:00:00Z".to_string(), 99999999)
        }
        _ => {  // "1h" default
            let t = chrono::Utc::now() - chrono::Duration::hours(1);
            (t.format("%Y-%m-%dT%H:%M:%SZ").to_string(), 3600)
        }
    };

    match state.db.get_token_rate_snapshots(
        params.provider_id.as_deref(),
        &start_time,
        limit,
    ).await {
        Ok(snapshots) => Json(snapshots).into_response(),
        Err(e) => {
            tracing::error!("Failed to get token rate: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StatsByApiKeyParams {
    pub limit: Option<i64>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenRateParams {
    pub provider_id: Option<String>,
    pub range: Option<String>,  // "1h", "5h", "1d", "1w", "1m", "all"
}

pub async fn get_top_provider(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.get_top_provider_by_usage().await {
        Ok(Some(pid)) => Json(serde_json::json!({ "provider_id": pid })).into_response(),
        Ok(None) => Json(serde_json::json!({ "provider_id": null })).into_response(),
        Err(e) => {
            tracing::error!("Failed to get top provider: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Update provider chart color
#[derive(Debug, Deserialize)]
pub struct UpdateChartColorRequest {
    pub chart_color: Option<String>,
}

pub async fn update_provider_chart_color(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateChartColorRequest>,
) -> impl IntoResponse {
    if state.db.get_provider(&id).await.ok().flatten().is_none() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response();
    }
    match state.db.update_provider_chart_color(&id, req.chart_color.as_deref()).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"id": id}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to update chart color: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UsageTrendParams {
    pub api_key_id: Option<String>,
    pub provider_id: Option<String>,
    pub days: Option<i64>,
}

// ========== Unified Model handlers ==========

#[derive(Debug, Deserialize)]
pub struct CreateModelRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_model_type")]
    pub model_type: String,
    #[serde(default)]
    pub priority: i64,
    pub config: Option<serde_json::Value>,
}

fn default_model_type() -> String { "chat".to_string() }

/// Create a new unified model
pub async fn create_model(
    State(state): State<AppState>,
    Json(req): Json<CreateModelRequest>,
) -> impl IntoResponse {
    let config_json = req.config.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default());
    
    match state.db.create_model(
        &req.id,
        &req.name,
        req.description.as_deref(),
        &req.model_type,
        req.priority,
        config_json.as_deref(),
    ).await {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({
            "id": req.id,
            "name": req.name,
            "message": "Model created successfully"
        }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to create model: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// List all unified models
pub async fn list_models(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.list_models_with_mappings().await {
        Ok(models) => Json(models).into_response(),
        Err(e) => {
            tracing::error!("Failed to list models: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get a specific unified model
pub async fn get_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_model_with_mappings(&id).await {
        Ok(Some(model)) => Json(model).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Model not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get model: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Update a unified model
#[derive(Debug, Deserialize)]
pub struct UpdateModelRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub model_type: Option<String>,
    pub is_active: Option<bool>,
    pub priority: Option<i64>,
    pub config: Option<serde_json::Value>,
}

pub async fn update_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateModelRequest>,
) -> impl IntoResponse {
    // Get existing model
    let existing = match state.db.get_model(&id).await {
        Ok(Some(m)) => m,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Model not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to get model: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let name = req.name.as_deref().unwrap_or(&existing.name);
    let description = req.description.as_ref().map(|s| s.as_str()).or(existing.description.as_deref());
    let model_type = req.model_type.as_deref().unwrap_or(&existing.model_type);
    let is_active = req.is_active.unwrap_or(existing.is_active);
    let priority = req.priority.unwrap_or(existing.priority);
    let config_json = req.config.as_ref()
        .map(|c| serde_json::to_string(c).ok())
        .unwrap_or(existing.config.clone());

    match state.db.update_model(
        &id,
        name,
        description,
        model_type,
        is_active,
        priority,
        config_json.as_deref(),
    ).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"id": id, "message": "Model updated"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to update model: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Delete a unified model
pub async fn delete_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_model(&id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"message": "Model deleted"}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Model not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete model: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// List mappings for a model
pub async fn list_model_mappings(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.list_model_mappings(&id).await {
        Ok(mappings) => Json(mappings).into_response(),
        Err(e) => {
            tracing::error!("Failed to list model mappings: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Add a provider mapping to a model
#[derive(Debug, Deserialize)]
pub struct AddModelMappingRequest {
    pub provider_id: String,
    pub provider_model_id: String,
    #[serde(default = "default_weight")]
    pub weight: i64,
    #[serde(default = "default_cost_multiplier")]
    pub cost_multiplier: f64,
}

pub async fn add_model_mapping(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AddModelMappingRequest>,
) -> impl IntoResponse {
    // Verify model exists
    if state.db.get_model(&id).await.ok().flatten().is_none() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Model not found"}))).into_response();
    }
    
    // Verify provider exists
    if state.db.get_provider(&req.provider_id).await.ok().flatten().is_none() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response();
    }

    match state.db.add_model_mapping(
        &id,
        &req.provider_id,
        &req.provider_model_id,
        req.weight,
        req.cost_multiplier,
    ).await {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({
            "model_id": id,
            "provider_id": req.provider_id,
            "provider_model_id": req.provider_model_id,
            "message": "Mapping added successfully"
        }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to add model mapping: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Update a model mapping
#[derive(Debug, Deserialize)]
pub struct UpdateModelMappingRequest {
    pub provider_model_id: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default = "default_weight")]
    pub weight: i64,
    #[serde(default = "default_cost_multiplier")]
    pub cost_multiplier: f64,
}

pub async fn update_model_mapping(
    State(state): State<AppState>,
    Path((model_id, provider_id, provider_model_id)): Path<(String, String, String)>,
    Json(req): Json<UpdateModelMappingRequest>,
) -> impl IntoResponse {
    match state.db.update_model_mapping(
        &model_id,
        &provider_id,
        &provider_model_id,
        &req.provider_model_id,
        req.is_active,
        req.weight,
        req.cost_multiplier,
    ).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({
            "model_id": model_id,
            "provider_id": provider_id,
            "message": "Mapping updated"
        }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to update model mapping: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Remove a provider mapping from a model
pub async fn remove_model_mapping(
    State(state): State<AppState>,
    Path((model_id, provider_id, provider_model_id)): Path<(String, String, String)>,
) -> impl IntoResponse {
    match state.db.remove_model_mapping(&model_id, &provider_id, &provider_model_id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"message": "Mapping removed"}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Mapping not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to remove model mapping: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

// ========== Provider Quota handlers ==========

#[derive(Debug, Deserialize)]
pub struct SetQuotaRequest {
    pub quota_type: Option<String>,
    pub window_mode: Option<String>,
    pub window_size: Option<String>,
    pub window_start_override: Option<String>,
    pub rolling_step: Option<String>,
    pub rolling_step_tz: Option<String>,
    pub limit_count: i64,
    #[serde(default = "default_quota_enabled")]
    pub is_enabled: bool,
}

fn default_quota_enabled() -> bool { true }
fn default_window_mode() -> String { "fixed".to_string() }
fn default_window_size() -> String { "5h".to_string() }

/// Validate window_size format (e.g. "5h", "7d", "30d")
fn is_valid_window_size(s: &str) -> bool {
    if let Some(hours) = s.strip_suffix('h') {
        return hours.parse::<i64>().map(|h| h > 0).unwrap_or(false);
    }
    if let Some(days) = s.strip_suffix('d') {
        return days.parse::<i64>().map(|d| d > 0).unwrap_or(false);
    }
    false
}

/// Set a quota limit for a provider
pub async fn set_provider_quota(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Json(req): Json<SetQuotaRequest>,
) -> impl IntoResponse {
    // Verify provider exists
    if state.db.get_provider(&provider_id).await.ok().flatten().is_none() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response();
    }

    // Determine window_mode and window_size
    let (quota_type, window_mode, window_size) = if let Some(ref qt) = req.quota_type {
        // Legacy quota_type: convert to new format
        match qt.as_str() {
            "5h" => ("sliding:5h".to_string(), "sliding".to_string(), "5h".to_string()),
            "weekly" => ("fixed:7d".to_string(), "fixed".to_string(), "7d".to_string()),
            "monthly" => ("fixed:30d".to_string(), "fixed".to_string(), "30d".to_string()),
            other => {
                // Could be new format like "fixed:5h" or "rolling:5h:1h"
                let parts: Vec<&str> = other.split(':').collect();
                if parts.len() >= 2 {
                    let mode = parts[0];
                    if !["fixed", "sliding", "rolling"].contains(&mode) {
                        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                            "error": "Invalid window_mode. Must be 'fixed', 'sliding', or 'rolling'"
                        }))).into_response();
                    }
                    let size = parts[1];
                    if !is_valid_window_size(size) {
                        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                            "error": format!("Invalid window_size '{}'. Use format like 5h, 12h, 7d, 30d", size)
                        }))).into_response();
                    }
                    (other.to_string(), mode.to_string(), size.to_string())
                } else {
                    return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                        "error": format!("Invalid quota_type '{}'. Use window_mode + window_size, or legacy: 5h, weekly, monthly", other)
                    }))).into_response();
                }
            }
        }
    } else {
        // New format: window_mode + window_size
        let mode = req.window_mode.as_deref().unwrap_or("fixed");
        if !["fixed", "sliding", "rolling"].contains(&mode) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Invalid window_mode. Must be 'fixed', 'sliding', or 'rolling'"
            }))).into_response();
        }
        let size = req.window_size.as_deref().unwrap_or("5h");
        if !is_valid_window_size(size) {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Invalid window_size '{}'. Use format like 5h, 12h, 7d, 30d", size)
            }))).into_response();
        }
        // For rolling mode, quota_type includes step: rolling:size:step
        let qt = if mode == "rolling" {
            let step = req.rolling_step.as_deref().unwrap_or("1h");
            format!("{}:{}:{}", mode, size, step)
        } else {
            format!("{}:{}", mode, size)
        };
        (qt, mode.to_string(), size.to_string())
    };

    if req.limit_count <= 0 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "limit_count must be positive"
        }))).into_response();
    }

    match state.db.set_provider_quota(
        &provider_id,
        &quota_type,
        &window_mode,
        &window_size,
        req.window_start_override.as_deref(),
        req.rolling_step.as_deref(),
        req.rolling_step_tz.as_deref(),
        req.limit_count,
        req.is_enabled,
    ).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({
            "provider_id": provider_id,
            "quota_type": quota_type,
            "window_mode": window_mode,
            "window_size": window_size,
            "window_start_override": req.window_start_override,
            "rolling_step": req.rolling_step,
            "rolling_step_tz": req.rolling_step_tz,
            "limit_count": req.limit_count,
            "is_enabled": req.is_enabled,
            "message": "Quota set successfully"
        }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to set provider quota: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get quota usage for a specific provider
pub async fn get_provider_quota_usage(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_provider_quota_usage(&provider_id).await {
        Ok(usage) => Json(usage).into_response(),
        Err(e) => {
            tracing::error!("Failed to get provider quota usage: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Delete a quota for a provider
pub async fn delete_provider_quota(
    State(state): State<AppState>,
    Path((provider_id, quota_type)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.db.delete_provider_quota(&provider_id, &quota_type).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"message": "Quota deleted"}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Quota not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete provider quota: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get all quota usage across all providers
pub async fn get_all_quota_usage(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.get_all_quota_usage().await {
        Ok(usage) => Json(usage).into_response(),
        Err(e) => {
            tracing::error!("Failed to get all quota usage: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SetCalibrationRequest {
    pub quota_type: String,
    /// The total number of requests already used OUTSIDE the gateway
    /// in the current period. This is an offset added to the gateway count.
    pub calibration_offset: i64,
    pub note: Option<String>,
}

/// Set calibration offset for a provider quota
pub async fn set_quota_calibration(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
    Json(req): Json<SetCalibrationRequest>,
) -> impl IntoResponse {
    // Verify provider exists
    if state.db.get_provider(&provider_id).await.ok().flatten().is_none() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Provider not found"}))).into_response();
    }

    // Verify quota exists for this type
    let quotas = state.db.list_provider_quotas(&provider_id).await;
    let quota = quotas.unwrap_or_default().into_iter().find(|q| q.quota_type == req.quota_type);
    if quota.is_none() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": format!("No quota of type '{}' configured for this provider. Please set the quota first.", req.quota_type)
        }))).into_response();
    }

    // Calculate current window period for this quota and record it
    let quota = quota.unwrap();
    let now = chrono::Utc::now();
    let (window_mode, window_size) = if quota.window_mode.is_empty() {
        crate::db::Database::normalize_quota_type(&quota.quota_type)
    } else {
        (quota.window_mode.clone(), quota.window_size.clone())
    };
    let (period_start, period_end) = crate::db::Database::compute_quota_period(&quota, None, &now);
    let period_start_str = period_start.format("%Y-%m-%d %H:%M:%S").to_string();
    let period_end_str = period_end.format("%Y-%m-%d %H:%M:%S").to_string();

    match state.db.set_quota_calibration(
        &provider_id,
        &req.quota_type,
        req.calibration_offset,
        Some(&period_start_str),
        Some(&period_end_str),
        req.note.as_deref(),
    ).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({
            "provider_id": provider_id,
            "quota_type": req.quota_type,
            "calibration_offset": req.calibration_offset,
            "calibration_window_start": period_start_str,
            "calibration_window_end": period_end_str,
            "message": "Calibration set successfully"
        }))).into_response(),
        Err(e) => {
            tracing::error!("Failed to set quota calibration: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Get calibration info for a provider
pub async fn get_provider_calibrations(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> impl IntoResponse {
    match state.db.list_provider_calibrations(&provider_id).await {
        Ok(calibrations) => Json(calibrations).into_response(),
        Err(e) => {
            tracing::error!("Failed to get provider calibrations: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
