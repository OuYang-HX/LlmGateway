//! Dashboard HTML correctness tests
//!
//! Covers issues found during development:
//! 1. Token rate chart hidden dataset default not set correctly
//! 2. Chart.js legend click handler used wrong API
//! 3. Token rate legend text not updated on chart init
//! 4. Chart.js CDN loaded twice

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
fn test_dashboard_html_token_rate_input_hidden_by_default() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // The Input dataset should be hidden by default (hidden: true)
    // This is critical for the "default show Output only" requirement
    assert!(
        content.contains("hidden: true"),
        "Chart dataset should have 'hidden: true' for the default-hidden Input line"
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
fn test_dashboard_html_token_rate_legend_updated_on_init() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // In the else branch (chart first creation), updateTokenRateLegend must be called AFTER tokenRateChart = new Chart
    // We look for: '} else {' followed by 'tokenRateChart = new Chart', then later 'updateTokenRateLegend(tokenRateChart)'
    let else_pattern = content.find("} else {\n            tokenRateChart = new Chart");
    assert!(else_pattern.is_some(), "Should find else branch with chart creation");

    // Get everything after the else pattern start
    let after_else = &content[else_pattern.unwrap()..];

    // The else block ends at the next '});\n        }' - find the chart creation and update in this scope
    let chart_in_else = after_else.find("tokenRateChart = new Chart");
    let update_in_else = after_else.find("updateTokenRateLegend(tokenRateChart)");

    assert!(chart_in_else.is_some(), "Should find chart creation in else branch");
    assert!(update_in_else.is_some(), "Should find updateTokenRateLegend call in else branch");
    assert!(
        update_in_else > chart_in_else,
        "updateTokenRateLegend should be called AFTER chart creation in else branch"
    );
}

#[test]
fn test_dashboard_html_token_rate_uses_elapsed_seconds() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Frontend should use elapsed_seconds from backend for accurate rate calculation
    // NOT a hardcoded 10-second interval
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
fn test_dashboard_html_token_rate_legend_text_function_exists() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    assert!(
        content.contains("function updateTokenRateLegend"),
        "updateTokenRateLegend function should exist"
    );
}

#[test]
fn test_dashboard_html_token_rate_legend_shows_only_visible_datasets() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // The legend text should only show peaks for VISIBLE datasets
    // This is the key logic fix - previously it always showed both
    assert!(
        content.contains("visible.length"),
        "Legend text should check visible datasets"
    );
}

#[test]
fn test_dashboard_html_token_rate_uses_local_timezone() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // snapshot_time from backend is UTC ISO format (ending in Z)
    // Frontend should parse it with new Date() and display in local time
    // toLocaleTimeString handles timezone automatically
    assert!(
        content.contains("toLocaleTimeString"),
        "Should use toLocaleTimeString for local timezone display"
    );

    // Should NOT use getUTCHours or UTC-based manual formatting
    // (which would show UTC time instead of local time)
    assert!(
        !content.contains("getUTCHours"),
        "Should NOT use UTC methods that ignore local timezone"
    );
}

#[test]
fn test_dashboard_html_api_key_usage_chart_has_detail_table() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // API Key usage section should have a detail table element
    assert!(
        content.contains("apikey-usage-detail"),
        "Should have apikey-usage-detail element for per-key stats table"
    );
}

#[test]
fn test_dashboard_html_api_key_usage_chart_uses_getApiKeyName() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // Chart labels should use getApiKeyName to show readable names, not raw IDs
    assert!(
        content.contains("getApiKeyName"),
        "API Key usage chart should use getApiKeyName for labels"
    );
}

#[test]
fn test_dashboard_html_api_key_edit_modal_exists() {
    let content = fs::read_to_string("src/dashboard.html").unwrap();

    // API Key modal should have edit mode support
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

    // The "显示输入" checkbox was removed - it should NOT be present
    // (users now click the legend directly to toggle curves)
    assert!(
        !content.contains("显示输入"),
        "Should NOT have '显示输入' checkbox (replaced by legend click)"
    );
    assert!(
        !content.contains("tr-show-prompt"),
        "Should NOT have tr-show-prompt checkbox"
    );
}
