use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 49127,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:llm_gateway.db".to_string(),
        }
    }
}

/// Provider configuration - supports both static API key and dynamic token auth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default = "default_api_type")]
    pub api_type: String, // "openai", "anthropic", "custom"
    #[serde(default = "default_auth_type")]
    pub auth_type: String, // "api_key", "dynamic_token"
    pub api_key: Option<String>,
    // Dynamic token fields
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
    pub token_expiry_seconds: u64,
    #[serde(default = "default_weight")]
    pub weight: u32,
}

fn default_api_type() -> String { "openai".to_string() }
fn default_auth_type() -> String { "api_key".to_string() }
fn default_token_field() -> String { "token".to_string() }
fn default_refresh_token_field() -> String { "refreshToken".to_string() }
fn default_token_header_field() -> String { "Authorization".to_string() }
fn default_token_header_prefix() -> String { "Bearer ".to_string() }
fn default_token_expiry() -> u64 { 86400 }
fn default_weight() -> u32 { 1 }

/// API Key configuration for creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub allowed_providers: Option<Vec<String>>,
}

/// API Key response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key: String, // Only returned on creation
    pub key_prefix: String,
    pub allowed_providers: Option<Vec<String>>,
    pub is_active: bool,
    pub created_at: String,
}

/// Statistics query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsQuery {
    pub api_key_id: Option<String>,
    pub provider_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub granularity: Option<String>, // "5h", "day", "week", "month"
}

/// Statistics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub throttle_count: i64,
    pub error_count: i64,
    pub data_points: Vec<StatsDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsDataPoint {
    pub period_start: String,
    pub period_end: String,
    pub request_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub throttle_count: i64,
    pub error_count: i64,
}

/// Real-time token rate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRateResponse {
    pub provider_id: Option<String>,
    pub tokens_per_second: f64,
    pub prompt_tokens_per_second: f64,
    pub completion_tokens_per_second: f64,
    pub timestamp: String,
}

/// Request log query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLogQuery {
    pub api_key_id: Option<String>,
    pub provider_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// Request log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLogEntry {
    pub id: i64,
    pub api_key_id: String,
    pub provider_id: String,
    pub model: Option<String>,
    pub request_path: String,
    pub request_method: String,
    pub request_body: Option<String>,
    pub response_status: Option<i32>,
    pub response_body: Option<String>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: Option<i64>,
    pub is_streaming: bool,
    pub is_throttled: bool,
    pub error_message: Option<String>,
    pub created_at: String,
}

/// Dashboard summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub total_api_keys: i64,
    pub active_api_keys: i64,
    pub total_providers: i64,
    pub active_providers: i64,
    pub total_requests_24h: i64,
    pub total_tokens_24h: i64,
    pub avg_tokens_per_second: f64,
    pub throttle_count_24h: i64,
}

/// Provider with current token status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub auth_type: String,
    pub has_valid_token: bool,
    pub token_expires_at: Option<String>,
    pub total_requests: i64,
    pub total_tokens: i64,
    pub throttle_count: i64,
}

/// LLM Proxy request - OpenAI compatible
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    #[serde(default)]
    pub messages: Vec<LlmMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// LLM Proxy response (non-streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<LlmChoice>,
    pub usage: LlmUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChoice {
    pub index: i32,
    pub message: Option<LlmMessage>,
    pub delta: Option<LlmMessage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}
