//! Token rate and StatsCollector tests
//!
//! Covers issues found during development:
//! 1. record_usage was never called from proxy (silent data loss)
//! 2. elapsed_seconds was missing from database table (caused all inserts to fail)
//! 3. Rate calculation used hardcoded interval (inaccurate)
//! 4. Token counts were mixed together (prompt + completion = 0 visibility)
//! 5. Snapshots with very short elapsed (< 1s) produce unreliable rates and are skipped

use llm_gateway::db::Database;
use llm_gateway::stats::StatsCollector;
use std::sync::Arc;

/// Build an in-memory database for tests
async fn test_db() -> Arc<Database> {
    Arc::new(Database::new_in_memory().await.unwrap())
}

// ==================== StatsCollector: record_usage accumulation ====================

#[tokio::test]
async fn test_stats_collector_record_usage_accumulates() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // Record several usage events
    collector.record_usage("test-provider", 100, 200).await;
    collector.record_usage("test-provider", 50, 75).await;
    collector.record_usage("test-provider", 150, 300).await;

    // Wait >1s so elapsed >= 1.0 (snapshots with elapsed < 1s are skipped)
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    // Take a snapshot - the window should contain all accumulated tokens
    let result = collector.take_snapshot("test-provider").await;
    assert!(result.is_ok(), "Snapshot should succeed");

    // Verify the snapshot was persisted to the database
    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert!(!snapshots.is_empty(), "Snapshot should be persisted to DB");

    let snap = &snapshots[0];
    assert_eq!(snap.prompt_tokens, 300, "Total prompt tokens = 100 + 50 + 150");
    assert_eq!(snap.completion_tokens, 575, "Total completion tokens = 200 + 75 + 300");
    assert_eq!(snap.request_count, 3, "Three requests recorded");
}

#[tokio::test]
async fn test_stats_collector_window_resets_after_snapshot() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // Record usage
    collector.record_usage("test-provider", 100, 200).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    // Take first snapshot
    collector.take_snapshot("test-provider").await.unwrap();

    // Record more usage
    collector.record_usage("test-provider", 500, 600).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    // Take second snapshot
    collector.take_snapshot("test-provider").await.unwrap();

    // Should have 2 snapshots, each with different token counts
    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert_eq!(snapshots.len(), 2);

    // First snapshot: 100+200=300
    assert_eq!(snapshots[0].prompt_tokens, 100);
    assert_eq!(snapshots[0].completion_tokens, 200);

    // Second snapshot: 500+600=1100
    assert_eq!(snapshots[1].prompt_tokens, 500);
    assert_eq!(snapshots[1].completion_tokens, 600);
}

// ==================== StatsCollector: elapsed_seconds accuracy ====================

#[tokio::test]
async fn test_stats_collector_snapshot_contains_elapsed_seconds() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    collector.record_usage("test-provider", 100, 200).await;

    // Wait >1s so snapshot is not skipped
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    collector.take_snapshot("test-provider").await.unwrap();

    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert_eq!(snapshots.len(), 1);

    // elapsed_seconds should be >= 1.0 (we waited >1s)
    let elapsed = snapshots[0].elapsed_seconds;
    assert!(
        elapsed >= 1.0,
        "elapsed_seconds should be >= 1.0, got {}",
        elapsed
    );
}

#[tokio::test]
async fn test_stats_collector_elapsed_not_hardcoded_10_seconds() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // Record and wait ~1.1s
    collector.record_usage("test-provider", 1000, 2000).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    collector.take_snapshot("test-provider").await.unwrap();

    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    let first_elapsed = snapshots[0].elapsed_seconds;

    // first_elapsed should be ~1.1s (from last_snapshot_time to now), NOT hardcoded 10s
    assert!(
        first_elapsed < 5.0,
        "First snapshot elapsed should be ~1.1s, not hardcoded 10s, got {}",
        first_elapsed
    );
    assert!(
        first_elapsed >= 1.0,
        "First snapshot elapsed should be >= 1.0, got {}",
        first_elapsed
    );

    // Wait ~3.1s before recording and snapshotting again
    // The elapsed for the second snapshot should be ~3.1s because
    // last_snapshot_time was set when the first snapshot was taken
    tokio::time::sleep(std::time::Duration::from_millis(3100)).await;
    collector.record_usage("test-provider", 1000, 2000).await;
    // Small extra wait to ensure elapsed is captured correctly
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    collector.take_snapshot("test-provider").await.unwrap();

    let snapshots2 = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    let second_elapsed = snapshots2[1].elapsed_seconds;

    // Second elapsed should be ~3.2s (from last_snapshot_time of first snapshot)
    // This proves elapsed is based on actual interval timing, not hardcoded
    assert!(
        second_elapsed > 2.5,
        "Second snapshot elapsed should be ~3.2s, got {}",
        second_elapsed
    );
    assert!(
        second_elapsed < 5.0,
        "Second snapshot elapsed should not exceed 5s, got {}",
        second_elapsed
    );
}

// ==================== StatsCollector: prompt vs completion separation ====================

#[tokio::test]
async fn test_stats_collector_separate_prompt_and_completion() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // Only prompt tokens
    collector.record_usage("test-provider", 1000, 0).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    collector.take_snapshot("test-provider").await.unwrap();

    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert_eq!(snapshots[0].prompt_tokens, 1000);
    assert_eq!(snapshots[0].completion_tokens, 0);
}

#[tokio::test]
async fn test_stats_collector_separate_completion_only() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // Only completion tokens (e.g. from cached context)
    collector.record_usage("test-provider", 0, 500).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    collector.take_snapshot("test-provider").await.unwrap();

    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert_eq!(snapshots[0].prompt_tokens, 0);
    assert_eq!(snapshots[0].completion_tokens, 500);
}

// ==================== StatsCollector: zero tokens ====================

#[tokio::test]
async fn test_stats_collector_zero_tokens_skipped() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // Record zero tokens with request_count=1 but wait >1s
    // Since prompt+completion=0, the rate is 0 but snapshot should still be written
    // because request_count > 0 and elapsed >= 1.0
    collector.record_usage("test-provider", 0, 0).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let result = collector.take_snapshot("test-provider").await;
    assert!(result.is_ok());

    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].prompt_tokens, 0);
    assert_eq!(snapshots[0].completion_tokens, 0);
}

// ==================== StatsCollector: short elapsed snapshots are skipped ====================

#[tokio::test]
async fn test_stats_collector_short_elapsed_snapshot_skipped() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // Record usage and immediately take snapshot (elapsed < 1s)
    // This should be skipped — no snapshot written to DB
    collector.record_usage("test-provider", 1000, 2000).await;
    let rate = collector.take_snapshot("test-provider").await.unwrap();
    assert_eq!(rate, 0.0, "Short-elapsed snapshot should return 0.0");

    // Verify no snapshot was written to DB
    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert!(snapshots.is_empty(), "Short-elapsed snapshot should not be persisted");
}

// ==================== StatsCollector: no-request snapshots are skipped ====================

#[tokio::test]
async fn test_stats_collector_no_request_snapshot_skipped() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // A provider with no record_usage calls has no window entry
    // take_snapshot should return RowNotFound
    let result = collector.take_snapshot("nonexistent-provider").await;
    assert!(result.is_err(), "Snapshot for nonexistent provider should fail");
}

// ==================== StatsCollector: rate calculation accuracy ====================

#[tokio::test]
async fn test_stats_collector_rate_calculation_accurate() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    // Simulate: 1000 tokens in 10 seconds = 100 tokens/s
    collector.record_usage("test-provider", 600, 400).await;

    // Wait approximately 10 seconds
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let rate = collector.take_snapshot("test-provider").await.unwrap();

    // Rate should be approximately 100 (600+400=1000, elapsed≈10s)
    // Allow some tolerance due to timing variation
    assert!(
        (rate - 100.0).abs() < 20.0,
        "Rate should be approximately 100, got {}",
        rate
    );
}

// ==================== Database: elapsed_seconds column exists ====================

#[tokio::test]
async fn test_database_token_rate_snapshot_has_elapsed_column() {
    let db = test_db().await;

    // Insert directly with known elapsed_seconds
    db.insert_token_rate_snapshot(Some("test-prov"), 100.0, 50, 100, 5, 5.0).await.unwrap();

    // Query directly to verify the column exists and has a value
    let rows = sqlx::query_as::<_, (i64, Option<String>, f64, i64, i64, i64, f64, String)>(
        "SELECT id, provider_id, tokens_per_second, prompt_tokens, completion_tokens, request_count, elapsed_seconds, snapshot_time FROM token_rate_snapshots"
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.3, 50, "prompt_tokens");
    assert_eq!(row.4, 100, "completion_tokens");
    assert!((row.6 - 5.0).abs() < 0.01, "elapsed_seconds should be 5.0, got {}", row.6);
}

// ==================== Database: migration adds elapsed_seconds column ====================

#[tokio::test]
async fn test_database_migration_adds_elapsed_seconds_column() {
    let db: Arc<Database> = Arc::new(Database::new_in_memory().await.unwrap());

    // Verify the column exists in the current schema (migration already ran at DB init)
    let rows: Vec<(i64, f64, i64, i64, i64, f64)> = sqlx::query_as(
        "SELECT id, tokens_per_second, prompt_tokens, completion_tokens, request_count, elapsed_seconds FROM token_rate_snapshots LIMIT 0"
    )
    .fetch_all(&db.pool)
    .await
    .unwrap(); // Column exists - query succeeds

    // Also verify we can INSERT with elapsed_seconds (proves the column works)
    db.insert_token_rate_snapshot(Some("test-prov"), 100.0, 50, 100, 5, 5.0).await.unwrap();
    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].elapsed_seconds, 5.0);
}

// ==================== StatsCollector: snapshot_time uses UTC ISO format ====================

#[tokio::test]
async fn test_stats_collector_snapshot_time_is_utc_iso_format() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    collector.record_usage("test-provider", 10, 20).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    collector.take_snapshot("test-provider").await.unwrap();

    let snapshots = db.get_token_rate_snapshots(None, "1970-01-01T00:00:00Z", 100).await.unwrap();
    assert_eq!(snapshots.len(), 1);

    let time_str = &snapshots[0].snapshot_time;
    // Should be UTC ISO format: "2024-01-01T12:00:00Z"
    assert!(
        time_str.ends_with('Z'),
        "snapshot_time should end with Z (UTC), got: {}",
        time_str
    );
    assert!(
        time_str.contains('T'),
        "snapshot_time should contain T separator, got: {}",
        time_str
    );
}

// ==================== StatsCollector: get_current_rate ====================

#[tokio::test]
async fn test_stats_collector_get_current_rate_returns_data() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    collector.record_usage("test-provider", 100, 200).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    collector.take_snapshot("test-provider").await.unwrap();

    let rates = collector.get_current_rate(None).await.unwrap();
    assert!(!rates.is_empty());
    assert_eq!(rates[0].prompt_tokens, 100);
    assert_eq!(rates[0].completion_tokens, 200);
}

// ==================== StatsCollector: provider_id filtering ====================

#[tokio::test]
async fn test_stats_collector_snapshot_provider_id() {
    let db = test_db().await;
    let collector = StatsCollector::new(db.clone());

    collector.record_usage("openai", 100, 200).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    collector.take_snapshot("openai").await.unwrap();

    collector.record_usage("anthropic", 50, 75).await;
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    collector.take_snapshot("anthropic").await.unwrap();

    let openai_rates = collector.get_current_rate(Some("openai")).await.unwrap();
    assert_eq!(openai_rates.len(), 1);
    assert_eq!(openai_rates[0].provider_id.as_deref(), Some("openai"));

    let anthropic_rates = collector.get_current_rate(Some("anthropic")).await.unwrap();
    assert_eq!(anthropic_rates.len(), 1);
    assert_eq!(anthropic_rates[0].provider_id.as_deref(), Some("anthropic"));

    let all_rates = collector.get_current_rate(None).await.unwrap();
    assert_eq!(all_rates.len(), 2);
}
