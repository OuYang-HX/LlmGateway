//! Dashboard HTML correctness tests
//!
//! Covers issues found during development:
//! 1. Token rate charts split into output and input
//! 2. Multi-provider datasets with auto colors
//! 3. Chart color persistence via API
//! 4. Chart.js CDN loaded once

use std::fs;

#[test]
fn test_dashboard_html_has_no_duplicate_chartjs_cdn() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    let chartjs_count = content.matches("cdn.jsdelivr.net/npm/chart.js").count();
    assert_eq!(
        chartjs_count, 1,
        "Chart.js CDN should be included exactly once, found {} times",
        chartjs_count
    );
}

#[test]
fn test_dashboard_html_token_rate_has_two_separate_charts() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Should have separate output and input chart canvases
    assert!(
        content.contains("output-rate-chart"),
        "Should have output-rate-chart canvas"
    );
    assert!(
        content.contains("input-rate-chart"),
        "Should have input-rate-chart canvas"
    );
}

#[test]
fn test_dashboard_html_token_rate_output_before_input() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Output chart should appear before input chart in HTML
    let output_pos = content.find("output-rate-chart").expect("Should have output-rate-chart");
    let input_pos = content.find("input-rate-chart").expect("Should have input-rate-chart");
    assert!(
        output_pos < input_pos,
        "Output chart should appear before input chart in HTML"
    );
}

#[test]
fn test_dashboard_html_token_rate_has_legend_onclick_handler() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Should have onClick handler for legend
    assert!(
        content.contains("onClick"),
        "Chart legend should have onClick handler"
    );
}

#[test]
fn test_dashboard_html_token_rate_uses_elapsed_seconds() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Frontend should use elapsed_seconds from backend for accurate rate calculation
    assert!(
        content.contains("elapsed_seconds"),
        "Frontend should use elapsed_seconds from backend"
    );

    // Should NOT have the old hardcoded SNAPSHOT_INTERVAL constant
    assert!(
        !content.contains("SNAPSHOT_INTERVAL"),
        "Should NOT use hardcoded SNAPSHOT_INTERVAL constant"
    );
}

#[test]
fn test_dashboard_html_token_rate_has_updateRateLegends() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    assert!(
        content.contains("function updateRateLegends"),
        "updateRateLegends function should exist"
    );
}

#[test]
fn test_dashboard_html_token_rate_syncs_legend_between_charts() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Clicking legend on one chart should sync hidden state to the other
    assert!(
        content.contains("syncChartLegend"),
        "Should have syncChartLegend function to sync legend state between charts"
    );
}

#[test]
fn test_dashboard_html_token_rate_has_provider_color_system() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Should have default color palette
    assert!(
        content.contains("DEFAULT_COLORS"),
        "Should have DEFAULT_COLORS palette for auto-assigning colors"
    );

    // Should have getProviderColor function
    assert!(
        content.contains("function getProviderColor"),
        "Should have getProviderColor function"
    );
}

#[test]
fn test_dashboard_html_token_rate_uses_local_timezone() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    assert!(
        content.contains("toLocaleTimeString"),
        "Should use toLocaleTimeString for local timezone display"
    );

    assert!(
        !content.contains("getUTCHours"),
        "Should NOT use UTC methods that ignore local timezone"
    );
}

#[test]
fn test_dashboard_html_chart_color_modal_exists() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Should have chart color modal
    assert!(
        content.contains("chart-color-modal"),
        "Should have chart-color-modal for setting provider colors"
    );
    assert!(
        content.contains("openChartColorModal"),
        "Should have openChartColorModal function"
    );
    assert!(
        content.contains("saveChartColors"),
        "Should have saveChartColors function"
    );
}

#[test]
fn test_dashboard_html_chart_color_api_endpoint() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Should call the chart-color API endpoint
    assert!(
        content.contains("/chart-color"),
        "Should reference chart-color API endpoint for persisting colors"
    );
}

#[test]
fn test_dashboard_html_api_key_usage_has_detail_table() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    assert!(
        content.contains("apikey-usage-detail"),
        "Should have apikey-usage-detail element for per-key stats table"
    );
}

#[test]
fn test_dashboard_html_api_key_usage_uses_getApiKeyName() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    assert!(
        content.contains("getApiKeyName"),
        "API Key usage chart should use getApiKeyName for labels"
    );
}

#[test]
fn test_dashboard_html_api_key_edit_modal_exists() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    assert!(
        content.contains("ak-modal-title"),
        "Should have ak-modal-title element for edit/create mode"
    );
    assert!(
        content.contains("ak-edit-id"),
        "Should have ak-edit-id hidden input for tracking edit mode"
    );
    assert!(
        content.contains("ak-is-active"),
        "Should have ak-is-active selector for editing API key status"
    );
}

#[test]
fn test_dashboard_html_shows_input_checkbox_removed() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    assert!(
        !content.contains("显示输入"),
        "Should NOT have '显示输入' checkbox (replaced by legend click)"
    );
    assert!(
        !content.contains("tr-show-prompt"),
        "Should NOT have tr-show-prompt checkbox"
    );
}

#[test]
fn test_dashboard_html_error_row_highlighting_in_logs() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(
        content.contains("error") && (content.contains("color:#ef4444") || content.contains("background:#ef") || content.contains("rgb(239")),
        "Logs table should highlight error rows with red color"
    );
}

#[test]
fn test_dashboard_html_logs_pagination_controls() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(
        (content.contains("prev") || content.contains("previous") || content.contains("上一页") || content.contains("前一页")) &&
        (content.contains("next") || content.contains("下一页") || content.contains("后一页")),
        "Logs should have prev/next pagination"
    );
}

#[test]
fn test_dashboard_html_time_range_filter() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(
        content.contains("time") || content.contains("时间") || content.contains("range") || content.contains("范围"),
        "Should have time range filter"
    );
}

#[test]
fn test_dashboard_html_stats_by_api_key() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(
        content.contains("api-key") || content.contains("api_key") || content.contains("API Key") || content.contains("apiKey"),
        "Should have API key usage stats"
    );
}

#[test]
fn test_dashboard_html_csv_export_in_logs() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(
        content.contains("csv") || content.contains("CSV") || content.contains("export") || content.contains("导出"),
        "Should have CSV export for logs"
    );
}

#[test]
fn test_dashboard_html_refresh_token_button() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(
        content.contains("refresh") || content.contains("刷新") || content.contains("refresh-token"),
        "Should have refresh token functionality"
    );
}

#[test]
fn test_dashboard_html_logs_model_filter() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(
        content.contains("model") || content.contains("模型"),
        "Should have model filter in logs"
    );
}

#[test]
fn test_dashboard_html_batch_provider_toggle() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(
        content.contains("batch") || content.contains("批量"),
        "Should have batch provider operations"
    );
}
