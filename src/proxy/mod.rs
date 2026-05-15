pub mod handler;
pub mod ws_handler;

use crate::db::{Database, ProviderRow};
use crate::auth::AuthManager;
use crate::usage;
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
    /// HTTP client that bypasses system proxy (for internal networks)
    no_proxy_client: reqwest::Client,
    /// Round-robin counter for load balancing
    rr_counter: Arc<RwLock<HashMap<String, usize>>>,
}

impl LlmProxy {
    pub fn new(db: Arc<Database>, auth_manager: Arc<AuthManager>) -> Self {
        // Build a client that bypasses proxy
        let no_proxy_client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("Failed to build no_proxy client");

        Self {
            db,
            auth_manager,
            http_client: reqwest::Client::new(),
            no_proxy_client,
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
        let total_weight: i64 = filtered.iter().map(|p| p.weight).sum();
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

    /// Forward a non-streaming request and extract usage from the response
    pub async fn forward_and_collect(
        &self,
        api_key_id: &str,
        provider: &ProviderRow,
        path: &str,
        method: &str,
        headers: axum::http::HeaderMap,
        body: axum::body::Bytes,
    ) -> Result<ProxyResponse, ProxyError> {
        let start = std::time::Instant::now();

        // Get auth header
        let (auth_header_name, auth_header_value) = self.auth_manager.get_auth_header(provider).await
            .map_err(|e| ProxyError::AuthError(e.to_string()))?;

        // Build the target URL
        let base_url = provider.base_url.trim_end_matches('/');
        let target_url = format!("{}{}", base_url, path);

        // Select HTTP client based on bypass_proxy setting
        let client = if provider.bypass_proxy {
            &self.no_proxy_client
        } else {
            &self.http_client
        };

        // Build the forwarded request
        let mut req_builder = match method {
            "GET" => client.get(&target_url),
            "POST" => client.post(&target_url),
            "PUT" => client.put(&target_url),
            "DELETE" => client.delete(&target_url),
            "PATCH" => client.patch(&target_url),
            _ => client.post(&target_url),
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

        // Add stored cookies from auth response
        if let Some(cookies) = &provider.token_cookies {
            if !cookies.is_empty() {
                req_builder = req_builder.header("Cookie", cookies.as_str());
            }
        }

        // Set body
        if !body.is_empty() {
            req_builder = req_builder.body(body.clone());
        }

        // Send request
        let response = req_builder.send().await
            .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;

        let status_code = response.status().as_u16() as i32;
        let is_throttled = status_code == 429;
        let content_type = response.headers().get("content-type").cloned();
        let response_body = response.bytes().await.unwrap_or_default();

        let duration_ms = start.elapsed().as_millis() as i64;

        // Extract usage from response body
        let (prompt_tokens, completion_tokens, total_tokens) = serde_json::from_slice::<serde_json::Value>(&response_body)
            .map(|v| usage::extract_openai_usage(&v))
            .unwrap_or((0, 0, 0));

        // Extract model from response
        let response_model = serde_json::from_slice::<serde_json::Value>(&response_body)
            .ok()
            .and_then(|v| usage::extract_model_from_response(&v));

        // Extract model from request body as fallback
        let model = response_model.or_else(|| self.extract_model_from_body(&body));

        // Log the request
        let _ = self.db.insert_request_log(
            api_key_id,
            &provider.id,
            model.as_deref(),
            path,
            method,
            None,
            None,
            Some(status_code),
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

        Ok(ProxyResponse {
            status_code,
            content_type,
            body: response_body,
        })
    }

    /// Forward a streaming request, returning the upstream response for streaming
    /// The caller is responsible for calling log_streaming_request after the stream completes
    pub async fn forward_streaming(
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

        // Select HTTP client based on bypass_proxy setting
        let client = if provider.bypass_proxy {
            &self.no_proxy_client
        } else {
            &self.http_client
        };

        // Build the forwarded request
        let mut req_builder = match method {
            "GET" => client.get(&target_url),
            "POST" => client.post(&target_url),
            "PUT" => client.put(&target_url),
            "DELETE" => client.delete(&target_url),
            "PATCH" => client.patch(&target_url),
            _ => client.post(&target_url),
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

        // Add stored cookies from auth response
        if let Some(cookies) = &provider.token_cookies {
            if !cookies.is_empty() {
                req_builder = req_builder.header("Cookie", cookies.as_str());
            }
        }

        // Set body
        if !body.is_empty() {
            req_builder = req_builder.body(body.clone());
        }

        // Send request
        let response = req_builder.send().await
            .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;

        let duration_ms = start.elapsed().as_millis() as i64;
        let is_throttled = response.status().as_u16() == 429;
        let model = self.extract_model_from_body(&body);

        // Log a preliminary entry (token counts will be updated later)
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
            0, // Will be updated when stream completes
            0,
            0,
            Some(duration_ms),
            true,
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

/// Response from a non-streaming proxy request
pub struct ProxyResponse {
    pub status_code: i32,
    pub content_type: Option<axum::http::HeaderValue>,
    pub body: bytes::Bytes,
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
