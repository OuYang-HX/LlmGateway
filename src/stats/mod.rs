use crate::db::Database;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct StatsCollector {
    db: Arc<Database>,
    pub windows: Arc<RwLock<HashMap<String, TokenWindow>>>,
}

#[derive(Debug)]
pub struct TokenWindow {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub request_count: i64,
    /// Time when the last snapshot was taken (or window creation time for the first interval).
    /// Used as the denominator for rate = tokens / time_since_last_snapshot.
    pub last_snapshot_time: Instant,
}

impl Default for TokenWindow {
    fn default() -> Self {
        Self { prompt_tokens: 0, completion_tokens: 0, request_count: 0, last_snapshot_time: Instant::now() }
    }
}

impl StatsCollector {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db, windows: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn record_usage(&self, provider_id: &str, prompt_tokens: i64, completion_tokens: i64) {
        let mut windows = self.windows.write().await;
        let window = windows.entry(provider_id.to_string()).or_default();
        window.prompt_tokens += prompt_tokens;
        window.completion_tokens += completion_tokens;
        window.request_count += 1;
    }

    pub async fn take_snapshot(&self, provider_id: &str) -> Result<f64, sqlx::Error> {
        let mut windows = self.windows.write().await;
        let window = windows.get_mut(provider_id).ok_or(sqlx::Error::RowNotFound)?;

        let elapsed = window.last_snapshot_time.elapsed().as_secs_f64();

        // No requests in this window — just reset the timer.
        if window.request_count == 0 {
            window.last_snapshot_time = Instant::now();
            return Ok(0.0);
        }

        // Skip snapshot if elapsed time is too short (< 1s) — the rate would be
        // unreliable (e.g. a request just completed and snapshot fired immediately,
        // yielding artificially high rates like 694 tokens/s from 39 tokens / 0.056s).
        if elapsed < 1.0 {
            return Ok(0.0);
        }

        let total = window.prompt_tokens + window.completion_tokens;
        let tps = total as f64 / elapsed;

        self.db.insert_token_rate_snapshot(Some(provider_id), tps, window.prompt_tokens, window.completion_tokens, window.request_count, elapsed).await?;

        // Reset counters but keep the timing chain — next snapshot measures from now.
        window.prompt_tokens = 0;
        window.completion_tokens = 0;
        window.request_count = 0;
        window.last_snapshot_time = Instant::now();

        Ok(tps)
    }

    pub async fn get_active_provider_ids(&self) -> Vec<String> {
        self.windows.read().await.keys().cloned().collect()
    }

    pub async fn get_current_rate(&self, provider_id: Option<&str>) -> Result<Vec<crate::db::TokenRateRow>, sqlx::Error> {
        let one_hour_ago = chrono::Utc::now() - chrono::Duration::hours(1);
        let start_time = one_hour_ago.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        self.db.get_token_rate_snapshots(provider_id, &start_time, 3600).await
    }

    pub fn start_snapshot_task(&self, interval_secs: u64) {
        let collector = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                for pid in collector.get_active_provider_ids().await {
                    if let Err(e) = collector.take_snapshot(&pid).await {
                        tracing::error!("Snapshot error for {}: {}", pid, e);
                    }
                }
            }
        });
    }

    pub fn start_cleanup_task(&self, interval_secs: u64) {
        let db = self.db.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                let before = (chrono::Utc::now() - chrono::Duration::hours(1)).format("%Y-%m-%dT%H:%M:%SZ").to_string();
                if let Err(e) = db.cleanup_old_snapshots(&before).await {
                    tracing::error!("Cleanup error: {}", e);
                }
            }
        });
    }
}
