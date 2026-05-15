pub mod token_refresh;

use crate::db::Database;
use chrono::Utc;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Manages dynamic token authentication for providers
#[derive(Debug, Clone)]
pub struct AuthManager {
    db: Arc<Database>,
    http_client: reqwest::Client,
    /// HTTP client that bypasses system proxy (for internal networks)
    no_proxy_client: reqwest::Client,
}

/// Token response from a dynamic token endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

impl AuthManager {
    pub fn new(db: Arc<Database>) -> Self {
        let http_client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("Failed to build HTTP client");
        let no_proxy_client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("Failed to build no_proxy client");
        Self { db, http_client, no_proxy_client }
    }

    /// Get a valid auth header value for a provider
    /// If the provider uses dynamic tokens, refresh if needed
    pub async fn get_auth_header(&self, provider: &crate::db::ProviderRow) -> Result<(String, String), AuthError> {
        match provider.auth_type.as_str() {
            "api_key" => {
                let key = provider.api_key.as_deref().ok_or(AuthError::NoApiKey)?;
                Ok((provider.token_header_field.clone(), format!("{}{}", provider.token_header_prefix, key)))
            }
            "dynamic_token" => {
                let token = self.get_valid_token(provider).await?;
                Ok((provider.token_header_field.clone(), format!("{}{}", provider.token_header_prefix, token)))
            }
            _ => Err(AuthError::UnknownAuthType(provider.auth_type.clone())),
        }
    }

    /// Get a valid token, refreshing if expired
    async fn get_valid_token(&self, provider: &crate::db::ProviderRow) -> Result<String, AuthError> {
        // Check if current token is still valid
        if let Some(current_token) = &provider.current_token {
            if let Some(expires_at) = &provider.token_expires_at {
                // Refresh 5 minutes before expiry
                let refresh_before = chrono::Duration::minutes(5);
                if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&format!("{}Z", expires_at.replace(' ', "T"))) {
                    if expires > Utc::now() + refresh_before {
                        return Ok(current_token.clone());
                    }
                }
            }
        }

        // Need to refresh token
        self.refresh_token(provider).await
    }

    /// Refresh the dynamic token for a provider
    pub async fn refresh_token(&self, provider: &crate::db::ProviderRow) -> Result<String, AuthError> {
        let _token_url = provider.token_url.as_deref().ok_or(AuthError::NoTokenUrl)?;
        let username = provider.token_username.as_deref().ok_or(AuthError::NoCredentials)?;
        let password = provider.token_password.as_deref().ok_or(AuthError::NoCredentials)?;

        // Try refresh token first if available
        if let Some(refresh_token) = &provider.current_refresh_token {
            if let Ok(token) = self.try_refresh_with_token(provider, refresh_token).await {
                return Ok(token);
            }
        }

        // Fall back to username/password login
        self.login_with_credentials(provider, username, password).await
    }

    /// Try to refresh using existing refresh token
    async fn try_refresh_with_token(&self, provider: &crate::db::ProviderRow, refresh_token: &str) -> Result<String, AuthError> {
        let token_url = provider.token_url.as_deref().ok_or(AuthError::NoTokenUrl)?;
        let method = provider.token_request_method.as_deref().unwrap_or("POST");
        let content_type = provider.token_content_type.as_deref().unwrap_or("json");

        let refresh_field = &provider.refresh_token_field;
        let body = if content_type == "form" {
            format!("{}={}", urlencoding::encode(refresh_field), urlencoding::encode(refresh_token))
        } else {
            serde_json::json!({ refresh_field: refresh_token }).to_string()
        };

        let response = self.send_token_request(token_url, method, content_type, &body, provider.token_extra_headers.as_deref(), provider.bypass_proxy).await?;

        if !response.status().is_success() {
            return Err(AuthError::RefreshFailed(response.status().to_string()));
        }

        // Extract Set-Cookie headers and store them
        let cookies: Vec<String> = response.headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .collect();
        if !cookies.is_empty() {
            let cookie_str = cookies.join("; ");
            tracing::info!("Storing cookies for provider {}: {}", provider.id, cookie_str);
            let _ = self.db.update_provider_cookies(&provider.id, &cookie_str).await;
        }

        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AuthError::ParseError(e.into()))?;

        self.extract_and_store_token(provider, &token_response).await
    }

    /// Login with username and password to get a new token
    /// Supports configurable request format:
    ///   - token_content_type: "json" or "form"
    ///   - token_username_field: field name for username (default "username")
    ///   - token_password_field: field name for password (default "password")
    ///   - token_body_template: optional JSON template with {{username}}/{{password}} placeholders
    ///   - token_request_method: "POST" or "GET"
    async fn login_with_credentials(&self, provider: &crate::db::ProviderRow, username: &str, password: &str) -> Result<String, AuthError> {
        let token_url = provider.token_url.as_deref().ok_or(AuthError::NoTokenUrl)?;
        let method = provider.token_request_method.as_deref().unwrap_or("POST");
        let content_type = provider.token_content_type.as_deref().unwrap_or("json");
        let username_field = provider.token_username_field.as_deref().unwrap_or("username");
        let password_field = provider.token_password_field.as_deref().unwrap_or("password");

        let response = if let Some(template) = &provider.token_body_template {
            // User provided a custom body template with {{username}}/{{password}} placeholders
            let body = template
                .replace("{{username}}", username)
                .replace("{{password}}", password);
            self.send_token_request(token_url, method, content_type, &body, provider.token_extra_headers.as_deref(), provider.bypass_proxy).await?
        } else {
            // Build body from field names
            if content_type == "form" {
                let body = format!("{}={}&{}={}",
                    urlencoding::encode(username_field),
                    urlencoding::encode(username),
                    urlencoding::encode(password_field),
                    urlencoding::encode(password));
                self.send_token_request(token_url, method, "form", &body, provider.token_extra_headers.as_deref(), provider.bypass_proxy).await?
            } else {
                let mut map = serde_json::Map::new();
                map.insert(username_field.to_string(), serde_json::Value::String(username.to_string()));
                map.insert(password_field.to_string(), serde_json::Value::String(password.to_string()));
                let body = serde_json::Value::Object(map);
                let body_str = serde_json::to_string(&body).map_err(|e| AuthError::ParseError(e.into()))?;
                self.send_token_request(token_url, method, "json", &body_str, provider.token_extra_headers.as_deref(), provider.bypass_proxy).await?
            }
        };

        if !response.status().is_success() {
            let status = response.status().to_string();
            let body = response.text().await.unwrap_or_default();
            tracing::error!("Login failed: status={}, body={}", status, body);
            return Err(AuthError::LoginFailed(status));
        }

        // Extract Set-Cookie headers and store them
        let cookies: Vec<String> = response.headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .collect();
        if !cookies.is_empty() {
            let cookie_str = cookies.join("; ");
            tracing::info!("Storing cookies for provider {}: {}", provider.id, cookie_str);
            let _ = self.db.update_provider_cookies(&provider.id, &cookie_str).await;
        }

        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AuthError::ParseError(e.into()))?;

        self.extract_and_store_token(provider, &token_response).await
    }

    /// Send a token request with the given body and optional extra headers
    async fn send_token_request(&self, url: &str, method: &str, content_type: &str, body: &str, extra_headers: Option<&str>, bypass_proxy: bool) -> Result<reqwest::Response, AuthError> {
        let method = match method {
            "GET" => reqwest::Method::GET,
            _ => reqwest::Method::POST,
        };

        let client = if bypass_proxy { &self.no_proxy_client } else { &self.http_client };
        let mut req = client.request(method, url);

        // Add custom extra headers from provider config
        // Skip Content-Type to avoid duplicate header conflict
        if let Some(headers_json) = extra_headers {
            if let Ok(headers) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(headers_json) {
                for (key, value) in headers {
                    if key.eq_ignore_ascii_case("content-type") { continue; }
                    if let Some(v) = value.as_str() {
                        req = req.header(&key, v);
                    }
                }
            }
        }

        if content_type == "form" {
            req = req.header("Content-Type", "application/x-www-form-urlencoded")
                     .body(body.to_string());
        } else {
            // For JSON, use reqwest's .json() method which properly encodes the body
            // Parse the body string as JSON value first
            let json_body: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| AuthError::ParseError(e.into()))?;
            req = req.json(&json_body);
        }

        req.send().await.map_err(AuthError::RequestFailed)
    }

    /// Extract token from response and store in database
    /// Supports dot-notation paths like "result.token", "result.newToken"
    async fn extract_and_store_token(&self, provider: &crate::db::ProviderRow, response: &serde_json::Value) -> Result<String, AuthError> {
        let token = extract_json_path(response, &provider.token_field)
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::TokenFieldNotFound(provider.token_field.clone()))?
            .to_string();

        let refresh_token = extract_json_path(response, &provider.refresh_token_field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let expires_at = Utc::now() + chrono::Duration::seconds(provider.token_expiry_seconds);
        let expires_at_str = expires_at.format("%Y-%m-%d %H:%M:%S").to_string();

        self.db.update_provider_token(
            &provider.id,
            &token,
            refresh_token.as_deref(),
            &expires_at_str,
        ).await.map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        Ok(token)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("No API key configured")]
    NoApiKey,
    #[error("No token URL configured")]
    NoTokenUrl,
    #[error("No credentials configured")]
    NoCredentials,
    #[error("Token field '{0}' not found in response")]
    TokenFieldNotFound(String),
    #[error("Unknown auth type: {0}")]
    UnknownAuthType(String),
    #[error("Request failed: {0}")]
    RequestFailed(reqwest::Error),
    #[error("Login failed: {0}")]
    LoginFailed(String),
    #[error("Refresh failed: {0}")]
    RefreshFailed(String),
    #[error("Parse error: {0}")]
    ParseError(Box<dyn std::error::Error + Send + Sync>),
    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Extract a value from a JSON document using a dot-notation path.
/// Examples:
///   "token"        → response.token
///   "result.token" → response.result.token
///   "result.newToken" → response.result.newToken
///   "data.access_token" → response.data.access_token
///
/// If the path has no dots, it works like a simple key lookup.
pub fn extract_json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;
    for part in parts {
        current = current.get(part)?;
    }
    Some(current)
}
