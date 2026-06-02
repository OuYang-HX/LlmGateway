use llm_gateway::db::Database;

async fn test_db() -> Database {
    Database::new_in_memory().await.unwrap()
}

async fn create_test_provider(db: &Database, id: &str) {
    db.create_provider(
        id, id, "https://api.example.com/v1", "openai", "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false, false,
        "", "", false,
    ).await.unwrap();
}

async fn create_test_provider_with_group(db: &Database, id: &str, api_type: &str, group_id: &str) {
    db.create_provider(
        id, id, "https://api.example.com/v1", api_type, "api_key",
        Some("test-key"), None, None, None, None, None, None, None, None, None, None,
        "token", "refreshToken", "Authorization", "Bearer ", 86400, 1, false, false,
        "", "", false,
    ).await.unwrap();
    db.update_provider_group_id(id, Some(group_id)).await.unwrap();
}

// ==================== group_id CRUD ====================

#[tokio::test]
async fn db_provider_group_id_set() {
    let db = test_db().await;
    create_test_provider(&db, "grp-prov").await;
    db.update_provider_group_id("grp-prov", Some("my-group")).await.unwrap();
    let provider = db.get_provider("grp-prov").await.unwrap().unwrap();
    assert_eq!(provider.group_id, Some("my-group".to_string()));
}

#[tokio::test]
async fn db_provider_group_id_remove() {
    let db = test_db().await;
    create_test_provider(&db, "grp-rem").await;
    db.update_provider_group_id("grp-rem", Some("my-group")).await.unwrap();
    db.update_provider_group_id("grp-rem", None).await.unwrap();
    let provider = db.get_provider("grp-rem").await.unwrap().unwrap();
    assert!(provider.group_id.is_none());
}

#[tokio::test]
async fn db_provider_group_id_default_none() {
    let db = test_db().await;
    create_test_provider(&db, "grp-default").await;
    let provider = db.get_provider("grp-default").await.unwrap().unwrap();
    assert!(provider.group_id.is_none());
}

// ==================== request_logs with group_id ====================

#[tokio::test]
async fn db_request_log_uses_group_id() {
    let db = test_db().await;
    create_test_provider_with_group(&db, "rl-grp-openai", "openai", "rl-group").await;
    
    // When writing a request log, the effective_provider_id should be the group_id
    let effective_id = db.get_effective_provider_id("rl-grp-openai").await.unwrap();
    assert_eq!(effective_id, "rl-group");
    
    db.insert_request_log(
        "key1", &effective_id, Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(50), false, false, None
    ).await.unwrap();
    
    let count = db.count_provider_requests("rl-group", "1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z").await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn db_request_log_no_group_uses_provider_id() {
    let db = test_db().await;
    create_test_provider(&db, "rl-no-grp").await;
    
    let effective_id = db.get_effective_provider_id("rl-no-grp").await.unwrap();
    assert_eq!(effective_id, "rl-no-grp");
}

// ==================== Quota shared by group ====================

#[tokio::test]
async fn db_quota_shared_by_group() {
    let db = test_db().await;
    create_test_provider_with_group(&db, "qs-openai", "openai", "qs-group").await;
    create_test_provider_with_group(&db, "qs-anthropic", "anthropic", "qs-group").await;
    
    // Set quota on the group_id, not on individual providers
    db.set_provider_quota("qs-group", "request", "fixed", "day", None, None, None, 1000, true).await.unwrap();
    
    // Both providers should find the same quota through get_effective_provider_id
    let effective1 = db.get_effective_provider_id("qs-openai").await.unwrap();
    let effective2 = db.get_effective_provider_id("qs-anthropic").await.unwrap();
    assert_eq!(effective1, "qs-group");
    assert_eq!(effective2, "qs-group");
    
    // Both should see the same quota
    let quotas1 = db.list_provider_quotas(&effective1).await.unwrap();
    let quotas2 = db.list_provider_quotas(&effective2).await.unwrap();
    assert_eq!(quotas1.len(), 1);
    assert_eq!(quotas2.len(), 1);
    assert_eq!(quotas1[0].limit_count, quotas2[0].limit_count);
}

#[tokio::test]
async fn db_quota_no_group_uses_own_id() {
    let db = test_db().await;
    create_test_provider(&db, "qs-own").await;
    db.set_provider_quota("qs-own", "request", "fixed", "day", None, None, None, 500, true).await.unwrap();
    
    let effective = db.get_effective_provider_id("qs-own").await.unwrap();
    assert_eq!(effective, "qs-own");
    let quotas = db.list_provider_quotas(&effective).await.unwrap();
    assert_eq!(quotas.len(), 1);
}

// ==================== Stats aggregation by group ====================

#[tokio::test]
async fn db_stats_aggregated_by_group() {
    let db = test_db().await;
    create_test_provider_with_group(&db, "sa-openai", "openai", "sa-group").await;
    create_test_provider_with_group(&db, "sa-anthropic", "anthropic", "sa-group").await;
    
    // Both providers route requests to the same group_id in request_logs
    let effective = db.get_effective_provider_id("sa-openai").await.unwrap();
    assert_eq!(effective, "sa-group");
    
    db.insert_request_log(
        "key1", "sa-group", Some("model-a"), "/v1/chat/completions", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(50), false, false, None
    ).await.unwrap();
    db.insert_request_log(
        "key1", "sa-group", Some("model-a"), "/v1/messages", "POST",
        None, None, Some(200), None, None,
        50, 100, 150, Some(30), false, false, None
    ).await.unwrap();
    
    // Stats should show aggregated totals for the group
    let stats = db.get_stats(None, Some("sa-group"), None, None).await.unwrap();
    assert_eq!(stats.total_requests, 2);
    assert_eq!(stats.total_tokens, 450);
}

#[tokio::test]
async fn db_stats_no_group_separate() {
    let db = test_db().await;
    create_test_provider(&db, "ss-1").await;
    create_test_provider(&db, "ss-2").await;
    
    // Without group_id, each provider's stats are separate
    db.insert_request_log(
        "key1", "ss-1", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        100, 200, 300, Some(50), false, false, None
    ).await.unwrap();
    db.insert_request_log(
        "key1", "ss-2", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        50, 100, 150, Some(30), false, false, None
    ).await.unwrap();
    
    let stats1 = db.get_stats(None, Some("ss-1"), None, None).await.unwrap();
    assert_eq!(stats1.total_requests, 1);
    let stats2 = db.get_stats(None, Some("ss-2"), None, None).await.unwrap();
    assert_eq!(stats2.total_requests, 1);
}

// ==================== Backward compatibility ====================

#[tokio::test]
async fn db_backward_compat_no_group_id() {
    let db = test_db().await;
    create_test_provider(&db, "bc-prov").await;
    db.set_provider_quota("bc-prov", "request", "fixed", "day", None, None, None, 100, true).await.unwrap();
    db.insert_request_log(
        "key1", "bc-prov", Some("model-a"), "/v1/chat", "POST",
        None, None, Some(200), None, None,
        10, 20, 30, Some(5), false, false, None
    ).await.unwrap();
    
    // Everything should work as before without group_id
    let count = db.count_provider_requests("bc-prov", "1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z").await.unwrap();
    assert_eq!(count, 1);
    let stats = db.get_stats(None, Some("bc-prov"), None, None).await.unwrap();
    assert_eq!(stats.total_requests, 1);
    let quotas = db.list_provider_quotas("bc-prov").await.unwrap();
    assert_eq!(quotas.len(), 1);
}

// ==================== Mixed: some grouped, some not ====================

#[tokio::test]
async fn db_mixed_grouped_and_ungrouped() {
    let db = test_db().await;
    create_test_provider_with_group(&db, "mix-grp-1", "openai", "mix-group").await;
    create_test_provider_with_group(&db, "mix-grp-2", "anthropic", "mix-group").await;
    create_test_provider(&db, "mix-standalone").await;
    
    // Grouped providers use group_id
    assert_eq!(db.get_effective_provider_id("mix-grp-1").await.unwrap(), "mix-group");
    assert_eq!(db.get_effective_provider_id("mix-grp-2").await.unwrap(), "mix-group");
    // Standalone uses own id
    assert_eq!(db.get_effective_provider_id("mix-standalone").await.unwrap(), "mix-standalone");
}

// ==================== Dashboard: list providers by group ====================

#[tokio::test]
async fn db_list_providers_in_same_group() {
    let db = test_db().await;
    create_test_provider_with_group(&db, "lsg-1", "openai", "lsg-group").await;
    create_test_provider_with_group(&db, "lsg-2", "anthropic", "lsg-group").await;
    
    let providers = db.list_providers_by_group("lsg-group").await.unwrap();
    assert_eq!(providers.len(), 2);
}
