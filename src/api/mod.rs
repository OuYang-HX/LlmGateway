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
        &req.response_content_path,
        &req.response_reasoning_path,
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
    pub new_id: Option<String>,
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
    let new_id = req.new_id.as_deref().unwrap_or(&existing.id);

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
        response_content_path,
        response_reasoning_path,
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
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub search: Option<String>,
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
        params.search.as_deref(),
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
    Path((model_id, provider_id)): Path<(String, String)>,
    Json(req): Json<UpdateModelMappingRequest>,
) -> impl IntoResponse {
    match state.db.update_model_mapping(
        &model_id,
        &provider_id,
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
    Path((model_id, provider_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.db.remove_model_mapping(&model_id, &provider_id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"message": "Mapping removed"}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Mapping not found"}))).into_response(),
        Err(e) => {
            tracing::error!("Failed to remove model mapping: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
