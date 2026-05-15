mod handler;

use crate::db::{Database, ProviderRow};
use crate::auth::AuthManager;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

pub use handler::proxy_request;

/// LLM Proxy that forwards requests to providers with load balancing
#[derive(Debug, Clone)]
pub struct LlmProxy {
    db: Arc<Database>,
    auth_manager: Arc<AuthManager>,
    http_client: reqwest::Client,
    /// Round-robin counter for load balancing
    rr_counter: Arc<RwLock<HashMap<String, usize>>>,
}

impl LlmProxy {
    pub fn new(db: Arc<Database>, auth_manager: Arc<AuthManager>) -> Self {
        Self {
            db,
            auth_manager,
            http_client: reqwest::Client::new(),
            rr_counter: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a reference to the HTTP client for WebSocket proxy
    pub fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }

    /// Select a provider using weighted round-robin
    pub async fn select_provider(&self, allowed_provider_ids: Option<&[String]>) -> Result<ProviderRow, ProxyError> {
        let providers = self.db.list_active_providers().await
            .map_err(|e| ProxyError::DatabaseError(e.to_string()))?;

        if providers.is_empty() {
            return Err(ProxyError::NoProvidersAvailable);
        }

        // Filter by allowed providers if specified
        let filtered: Vec<ProviderRow> = if let Some(allowed) = allowed_provider_ids {
            providers.into_iter()
                .filter(|p| allowed.contains(&p.id))
                .collect()
        } else {
            providers
        };

        if filtered.is_empty() {
            return Err(ProxyError::NoMatchingProvider);
        }

        // Weighted round-robin selection
        let total_weight: i32 = filtered.iter().map(|p| p.weight).sum();
        if total_weight == 0 {
            return Err(ProxyError::NoProvidersAvailable);
        }

        let key = filtered.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join(",");
        let mut counter = self.rr_counter.write().await;
        let current = counter.entry(key).or_insert(0);
        let mut pos = *current % total_weight as usize;

        for provider in &filtered {
            if pos < provider.weight as usize {
                *current = (*current + 1) % total_weight as usize;
                return Ok(provider.clone());
            }
            pos -= provider.weight as usize;
        }

        // Fallback to first
        *current = (*current + 1) % total_weight as usize;
        Ok(filtered[0].clone())
    }

    /// Forward a request to the selected provider
    pub async fn forward_request(
        &self,
        api_key_id: &str,
        provider: &ProviderRow,
        path: &str,
        method: &str,
        headers: axum::http::HeaderMap,
        body: axum::body::Bytes,
    ) -> Result<reqwest::Response, ProxyError> {
        let start = std::time::Instant::now();

        // Get auth header
        let (auth_header_name, auth_header_value) = self.auth_manager.get_auth_header(provider).await
            .map_err(|e| ProxyError::AuthError(e.to_string()))?;

        // Build the target URL
        let base_url = provider.base_url.trim_end_matches('/');
        let target_url = format!("{}{}", base_url, path);

        // Build the forwarded request
        let mut req_builder = match method {
            "GET" => self.http_client.get(&target_url),
            "POST" => self.http_client.post(&target_url),
            "PUT" => self.http_client.put(&target_url),
            "DELETE" => self.http_client.delete(&target_url),
            "PATCH" => self.http_client.patch(&target_url),
            _ => self.http_client.post(&target_url),
        };

        // Copy headers, replacing auth
        for (name, value) in headers.iter() {
            if name.as_str() != "authorization" && name.as_str() != "host" {
                if let Ok(v) = value.to_str() {
                    req_builder = req_builder.header(name.as_str(), v);
                }
            }
        }
        req_builder = req_builder.header(&auth_header_name, &auth_header_value);

        // Set body
        if !body.is_empty() {
            req_builder = req_builder.body(body.clone());
        }

        // Send request
        let response = req_builder.send().await
            .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;

        let duration_ms = start.elapsed().as_millis() as i64;

        // Check if throttled (429 status)
        let is_throttled = response.status().as_u16() == 429;

        // Parse token usage from response (best effort)
        let (prompt_tokens, completion_tokens, total_tokens) = (0i64, 0i64, 0i64);

        // Log the request
        let model = self.extract_model_from_body(&body);
        let _ = self.db.insert_request_log(
            api_key_id,
            &provider.id,
            model.as_deref(),
            path,
            method,
            None,
            None,
            Some(response.status().as_u16() as i32),
            None,
            None,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            Some(duration_ms),
            false,
            is_throttled,
            None,
        ).await;

        Ok(response)
    }

    /// Extract model name from request body
    pub fn extract_model_from_body(&self, body: &[u8]) -> Option<String> {
        serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("model")?.as_str().map(|s| s.to_string()))
    }

    /// Log a completed streaming request with token counts
    pub async fn log_streaming_request(
        &self,
        api_key_id: &str,
        provider_id: &str,
        path: &str,
        model: Option<&str>,
        response_status: i32,
        prompt_tokens: i64,
        completion_tokens: i64,
        total_tokens: i64,
        duration_ms: i64,
        is_throttled: bool,
        error_message: Option<&str>,
    ) -> Result<i64, ProxyError> {
        let id = self.db.insert_request_log(
            api_key_id,
            provider_id,
            model,
            path,
            "POST",
            None,
            None,
            Some(response_status),
            None,
            None,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            Some(duration_ms),
            true,
            is_throttled,
            error_message,
        ).await.map_err(|e| ProxyError::DatabaseError(e.to_string()))?;

        Ok(id)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("No providers available")]
    NoProvidersAvailable,
    #[error("No matching provider for the given constraints")]
    NoMatchingProvider,
    #[error("Auth error: {0}")]
    AuthError(String),
    #[error("Upstream error: {0}")]
    UpstreamError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
}
