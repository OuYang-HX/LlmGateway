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
}

/// Token response from a dynamic token endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

impl AuthManager {
    pub fn new(db: Arc<Database>) -> Self {
        let http_client = reqwest::Client::new();
        Self { db, http_client }
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

        let refresh_field = provider.refresh_token_field.clone();
        let body = serde_json::json!({
            refresh_field: refresh_token,
        });

        let response = self.http_client
            .post(token_url)
            .json(&body)
            .send()
            .await
            .map_err(AuthError::RequestFailed)?;

        if !response.status().is_success() {
            return Err(AuthError::RefreshFailed(response.status().to_string()));
        }

        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(AuthError::ParseError)?;

        self.extract_and_store_token(provider, &token_response).await
    }

    /// Login with username and password to get a new token
    async fn login_with_credentials(&self, provider: &crate::db::ProviderRow, username: &str, password: &str) -> Result<String, AuthError> {
        let token_url = provider.token_url.as_deref().ok_or(AuthError::NoTokenUrl)?;

        let body = serde_json::json!({
            "username": username,
            "password": password,
        });

        let response = self.http_client
            .post(token_url)
            .json(&body)
            .send()
            .await
            .map_err(AuthError::RequestFailed)?;

        if !response.status().is_success() {
            return Err(AuthError::LoginFailed(response.status().to_string()));
        }

        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(AuthError::ParseError)?;

        self.extract_and_store_token(provider, &token_response).await
    }

    /// Extract token from response and store in database
    async fn extract_and_store_token(&self, provider: &crate::db::ProviderRow, response: &serde_json::Value) -> Result<String, AuthError> {
        let token = response.get(&provider.token_field)
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::TokenFieldNotFound(provider.token_field.clone()))?
            .to_string();

        let refresh_token = response.get(&provider.refresh_token_field)
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
    ParseError(reqwest::Error),
    #[error("Database error: {0}")]
    DatabaseError(String),
}
