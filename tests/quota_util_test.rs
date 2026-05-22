use llm_gateway::db::Database;

// ==================== normalize_quota_type Tests ====================

#[test]
fn test_normalize_legacy_5h() {
    let (mode, size) = Database::normalize_quota_type("5h");
    assert_eq!(mode, "sliding");
    assert_eq!(size, "5h");
}

#[test]
fn test_normalize_legacy_weekly() {
    let (mode, size) = Database::normalize_quota_type("weekly");
    assert_eq!(mode, "fixed");
    assert_eq!(size, "7d");
}

#[test]
fn test_normalize_legacy_monthly() {
    let (mode, size) = Database::normalize_quota_type("monthly");
    assert_eq!(mode, "fixed");
    assert_eq!(size, "30d");
}

#[test]
fn test_normalize_new_format_fixed() {
    let (mode, size) = Database::normalize_quota_type("fixed:5h");
    assert_eq!(mode, "fixed");
    assert_eq!(size, "5h");
}

#[test]
fn test_normalize_new_format_sliding() {
    let (mode, size) = Database::normalize_quota_type("sliding:30d");
    assert_eq!(mode, "sliding");
    assert_eq!(size, "30d");
}

#[test]
fn test_normalize_new_format_rolling() {
    let (mode, size) = Database::normalize_quota_type("rolling:5h");
    assert_eq!(mode, "rolling");
    assert_eq!(size, "5h");
}

#[test]
fn test_normalize_unknown_defaults_to_fixed() {
    let (mode, size) = Database::normalize_quota_type("10h");
    assert_eq!(mode, "fixed");
    assert_eq!(size, "10h");
}

// ==================== get_quota_period Tests ====================

#[test]
fn test_quota_period_fixed_5h() {
    let now = chrono::Utc::now();
    let (start, end) = Database::get_quota_period("fixed", "5h", &now);
    let duration = end - start;
    assert_eq!(duration.num_hours(), 5);
}

#[test]
fn test_quota_period_fixed_30d() {
    let now = chrono::Utc::now();
    let (start, end) = Database::get_quota_period("fixed", "30d", &now);
    let duration = end - start;
    assert!(duration.num_days() == 30 || duration.num_days() == 31, "30d window should be ~30 days, got {}", duration.num_days());
}

#[test]
fn test_quota_period_sliding_5h() {
    let now = chrono::Utc::now();
    let (start, end) = Database::get_quota_period("sliding", "5h", &now);
    let duration = end - start;
    assert_eq!(duration.num_hours(), 5);
    // Sliding window: end should be close to now
    let diff = (end - now).num_seconds().abs();
    assert!(diff < 2, "End should be close to now, diff={}s", diff);
}

#[test]
fn test_quota_period_sliding_1h() {
    let now = chrono::Utc::now();
    let (start, end) = Database::get_quota_period("sliding", "1h", &now);
    let duration = end - start;
    assert_eq!(duration.num_hours(), 1);
}

// ==================== compute_rolling_next_refresh Tests ====================

#[test]
fn test_rolling_next_refresh_1h() {
    let now = chrono::Utc::now();
    let result = Database::compute_rolling_next_refresh("1h", "UTC", &now);
    assert!(result.is_some());
    // Should be a future time
    let next = result.unwrap();
    assert!(!next.is_empty());
}

#[test]
fn test_rolling_next_refresh_2h() {
    let now = chrono::Utc::now();
    let result = Database::compute_rolling_next_refresh("2h", "UTC", &now);
    assert!(result.is_some());
}

#[test]
fn test_rolling_next_refresh_6h() {
    let now = chrono::Utc::now();
    let result = Database::compute_rolling_next_refresh("6h", "UTC", &now);
    assert!(result.is_some());
}

#[test]
fn test_rolling_next_refresh_1d() {
    let now = chrono::Utc::now();
    let result = Database::compute_rolling_next_refresh("1d", "UTC", &now);
    assert!(result.is_some());
}

#[test]
fn test_rolling_next_refresh_shanghai_tz() {
    let now = chrono::Utc::now();
    let result = Database::compute_rolling_next_refresh("1h", "Asia/Shanghai", &now);
    assert!(result.is_some());
}

// ==================== is_calibration_valid Tests ====================

#[test]
fn test_calibration_valid_fixed_matching_window() {
    let now = chrono::Utc::now();
    let (start, end) = Database::get_quota_period("fixed", "5h", &now);
    let cal = llm_gateway::db::ProviderQuotaCalibrationRow {
        id: 1,
        provider_id: "test".to_string(),
        quota_type: "fixed:5h".to_string(),
        calibration_offset: 50,
        calibration_window_start: Some(start.format("%Y-%m-%d %H:%M:%S").to_string()),
        calibration_window_end: Some(end.format("%Y-%m-%d %H:%M:%S").to_string()),
        note: None,
        calibrated_at: String::new(),
    };
    assert!(Database::is_calibration_valid(&cal, "fixed", &start, &end));
}

#[test]
fn test_calibration_invalid_fixed_mismatched_window() {
    let now = chrono::Utc::now();
    let (start, end) = Database::get_quota_period("fixed", "5h", &now);
    let cal = llm_gateway::db::ProviderQuotaCalibrationRow {
        id: 1,
        provider_id: "test".to_string(),
        quota_type: "fixed:5h".to_string(),
        calibration_offset: 50,
        calibration_window_start: Some("2000-01-01 00:00:00".to_string()),
        calibration_window_end: Some("2000-01-01 05:00:00".to_string()),
        note: None,
        calibrated_at: String::new(),
    };
    assert!(!Database::is_calibration_valid(&cal, "fixed", &start, &end));
}

#[test]
fn test_calibration_invalid_fixed_no_window_bounds() {
    let now = chrono::Utc::now();
    let (start, end) = Database::get_quota_period("fixed", "5h", &now);
    let cal = llm_gateway::db::ProviderQuotaCalibrationRow {
        id: 1,
        provider_id: "test".to_string(),
        quota_type: "fixed:5h".to_string(),
        calibration_offset: 50,
        calibration_window_start: None,
        calibration_window_end: None,
        note: None,
        calibrated_at: String::new(),
    };
    assert!(!Database::is_calibration_valid(&cal, "fixed", &start, &end));
}

#[test]
fn test_calibration_valid_sliding_always() {
    let now = chrono::Utc::now();
    let (start, end) = Database::get_quota_period("sliding", "5h", &now);
    // Use a time that is definitely within the sliding window (1 minute ago)
    let cal_time = chrono::Utc::now() - chrono::Duration::minutes(1);
    let now_str = cal_time.format("%Y-%m-%d %H:%M:%S").to_string();
    let cal = llm_gateway::db::ProviderQuotaCalibrationRow {
        id: 1,
        provider_id: "test".to_string(),
        quota_type: "sliding:5h".to_string(),
        calibration_offset: 50,
        calibration_window_start: None,
        calibration_window_end: None,
        note: None,
        calibrated_at: now_str,
    };
    // Sliding window calibrations are valid if calibrated_at is within the window
    let valid = Database::is_calibration_valid(&cal, "sliding", &start, &end);
    assert!(valid, "calibration should be valid for sliding window with recent calibrated_at");
}
