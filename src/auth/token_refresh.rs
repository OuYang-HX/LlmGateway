use crate::db::Database;
use crate::auth::AuthManager;
use std::sync::Arc;

/// Background task that periodically refreshes dynamic tokens for providers
pub struct TokenRefreshTask {
    db: Arc<Database>,
    auth_manager: Arc<AuthManager>,
}

impl TokenRefreshTask {
    pub fn new(db: Arc<Database>, auth_manager: Arc<AuthManager>) -> Self {
        Self { db, auth_manager }
    }

    /// Start the background token refresh task
    pub fn start(self: Arc<Self>, interval_secs: u64) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(interval_secs)
            );
            loop {
                interval.tick().await;
                if let Err(e) = self.refresh_all_tokens().await {
                    tracing::error!("Token refresh task error: {}", e);
                }
            }
        });
    }

    /// Refresh tokens for all providers that need it
    pub async fn refresh_all_tokens(&self) -> Result<(), String> {
        let providers = self.db.list_active_providers().await
            .map_err(|e| e.to_string())?;

        for provider in providers {
            if provider.auth_type != "dynamic_token" {
                continue;
            }

            // Check if token is about to expire
            let needs_refresh = match &provider.token_expires_at {
                Some(expires_at) => {
                    // Refresh 10 minutes before expiry
                    let refresh_before = chrono::Duration::minutes(10);
                    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(
                        &format!("{}Z", expires_at.replace(' ', "T"))
                    ) {
                        expires < chrono::Utc::now() + refresh_before
                    } else {
                        true // Can't parse, try to refresh
                    }
                }
                None => true, // No expiry set, needs refresh
            };

            if needs_refresh {
                match self.auth_manager.refresh_token(&provider).await {
                    Ok(_) => {
                        tracing::info!("Refreshed token for provider {}", provider.id);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to refresh token for provider {}: {}", provider.id, e);
                    }
                }
            }
        }

        Ok(())
    }
}
