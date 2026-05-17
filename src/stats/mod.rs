use crate::db::Database;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

/// Statistics collector that tracks real-time token rates
#[derive(Debug, Clone)]
pub struct StatsCollector {
    db: Arc<Database>,
    /// In-memory counters for the current snapshot window
    pub current_window: Arc<RwLock<TokenWindow>>,
}

#[derive(Debug)]
pub struct TokenWindow {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub request_count: i64,
    pub start_time: Instant,
}

impl Default for TokenWindow {
    fn default() -> Self {
        Self {
            prompt_tokens: 0,
            completion_tokens: 0,
            request_count: 0,
            start_time: Instant::now(),
        }
    }
}

impl StatsCollector {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            current_window: Arc::new(RwLock::new(TokenWindow {
                start_time: Instant::now(),
                ..Default::default()
            })),
        }
    }

    /// Record token usage from a completed request
    pub async fn record_usage(&self, prompt_tokens: i64, completion_tokens: i64) {
        let mut window = self.current_window.write().await;
        window.prompt_tokens += prompt_tokens;
        window.completion_tokens += completion_tokens;
        window.request_count += 1;
    }

    /// Take a snapshot of the current window and persist it
    pub async fn take_snapshot(&self, provider_id: Option<&str>) -> Result<f64, sqlx::Error> {
        let mut window = self.current_window.write().await;
        let elapsed = window.start_time.elapsed().as_secs_f64();
        let total_tokens = window.prompt_tokens + window.completion_tokens;
        let tokens_per_second = if elapsed > 0.0 { total_tokens as f64 / elapsed } else { 0.0 };

        // Persist snapshot
        self.db.insert_token_rate_snapshot(
            provider_id,
            tokens_per_second,
            window.prompt_tokens,
            window.completion_tokens,
            window.request_count,
            elapsed,
        ).await?;

        // Reset window
        *window = TokenWindow {
            start_time: Instant::now(),
            ..Default::default()
        };

        Ok(tokens_per_second)
    }

    /// Get real-time token rate for the last N seconds
    pub async fn get_current_rate(&self, provider_id: Option<&str>) -> Result<Vec<crate::db::TokenRateRow>, sqlx::Error> {
        let one_hour_ago = chrono::Utc::now() - chrono::Duration::hours(1);
        let start_time = one_hour_ago.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        self.db.get_token_rate_snapshots(provider_id, &start_time, 3600).await
    }

    /// Start a background task that takes snapshots periodically
    pub fn start_snapshot_task(&self, interval_secs: u64) {
        let collector = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(interval_secs)
            );
            loop {
                interval.tick().await;
                if let Err(e) = collector.take_snapshot(None).await {
                    tracing::error!("Failed to take token rate snapshot: {}", e);
                }
            }
        });
    }

    /// Start a background cleanup task for old snapshots
    pub fn start_cleanup_task(&self, interval_secs: u64) {
        let db = self.db.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(interval_secs)
            );
            loop {
                interval.tick().await;
                let one_hour_ago = chrono::Utc::now() - chrono::Duration::hours(1);
                let before_time = one_hour_ago.format("%Y-%m-%dT%H:%M:%SZ").to_string();
                if let Err(e) = db.cleanup_old_snapshots(&before_time).await {
                    tracing::error!("Failed to cleanup old snapshots: {}", e);
                }
            }
        });
    }
}
