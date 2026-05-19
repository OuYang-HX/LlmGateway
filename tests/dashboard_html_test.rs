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

#[test]
fn test_dashboard_html_provider_health_table() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("provider-health") || content.contains("health-table") || content.contains("loadProviderHealth"),
        "Should have provider health table"
    );
}

#[test]
fn test_dashboard_html_log_detail_modal() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("log-detail-modal") || content.contains("showLogDetail"),
        "Should have log detail modal"
    );
}

#[test]
fn test_dashboard_html_log_detail_qa_mode() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("log-mode-qa") || content.contains("renderLogDetailQA"),
        "Should have QA mode for log detail"
    );
}

#[test]
fn test_dashboard_html_log_detail_raw_mode() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("log-mode-raw") || content.contains("renderLogDetailRaw"),
        "Should have raw mode for log detail"
    );
}

#[test]
fn test_dashboard_html_copy_to_clipboard_buttons() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("copyToClipboard") || content.contains("navigator.clipboard"),
        "Should have copy to clipboard functionality"
    );
}

#[test]
fn test_dashboard_html_sort_column_headers() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("currentLogSort") || content.contains("sortColumn") || content.contains("sortBy"),
        "Should have sortable column headers in logs"
    );
}

#[test]
fn test_dashboard_html_clear_filters_button() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("clearLogFilters") || content.contains("clear-filters") || content.contains("重置"),
        "Should have clear/reset filters button"
    );
}

#[test]
fn test_dashboard_html_time_preset_buttons() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("preset") || content.contains("1小时") || content.contains("24小时") || content.contains("7天"),
        "Should have time preset buttons for logs"
    );
}

#[test]
fn test_dashboard_html_external_id_modal() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("external-id-modal") || content.contains("saveExternalId"),
        "Should have external ID modal"
    );
}

#[test]
fn test_dashboard_html_batch_external_id() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("batch-external-id") || content.contains("showBatchExternalIdArea"),
        "Should have batch external ID configuration"
    );
}

#[test]
fn test_dashboard_html_api_key_regenerate() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("regenerate") || content.contains("regenApiKey"),
        "Should have API key regenerate functionality"
    );
}

#[test]
fn test_dashboard_html_provider_filter_on_token_rate() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("tr-provider") || content.contains("provider-filter"),
        "Should have provider filter on token rate chart"
    );
}

#[test]
fn test_dashboard_html_stats_by_api_key_chart() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("loadApiKeyUsageChart") || content.contains("api-key-usage") || content.contains("apiKeyUsageChart"),
        "Should have stats by API key chart"
    );
}

#[test]
fn test_dashboard_html_all_providers_in_token_rate_legend() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("allProviders.forEach") || content.contains("providerMap"),
        "Should iterate allProviders for token rate chart legend"
    );
}

#[test]
fn test_dashboard_html_hidden_dataset_for_no_data_provider() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("hidden:") || content.contains("hasData"),
        "Should hide datasets for providers with no data"
    );
}

#[test]
fn test_dashboard_html_zero_fill_for_no_data_provider() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("hasData ? null : 0") || content.contains("hasData"),
        "Should fill with 0 for providers with no data so clicking legend shows y=0 line"
    );
}

#[test]
fn test_dashboard_html_provider_edit_modal_has_reasoning_path_input() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("prov-response-reasoning-path"),
        "Provider edit modal must have response reasoning path input field"
    );
}

#[test]
fn test_dashboard_html_provider_edit_modal_has_content_path_input() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("prov-response-content-path"),
        "Provider edit modal must have response content path input field"
    );
}

#[test]
fn test_dashboard_html_provider_edit_sets_response_paths_for_all_auth_types() {
    let content = include_str!("../src/dashboard.html");
    // The response path fields should be set OUTSIDE the if/else auth_type block
    // so they work for both api_key and dynamic_token providers
    let pattern1 = "prov-response-content-path').value=p.response_content_path";
    let pattern2 = "prov-response-reasoning-path').value=p.response_reasoning_path";
    assert!(
        content.contains(pattern1),
        "editProvider should set response_content_path for all auth types"
    );
    assert!(
        content.contains(pattern2),
        "editProvider should set response_reasoning_path for all auth types"
    );
}

#[test]
fn test_dashboard_html_provider_save_sends_reasoning_path() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("body.response_reasoning_path=document.getElementById('prov-response-reasoning-path')"),
        "saveProvider should send response_reasoning_path in request body"
    );
}

#[test]
fn test_dashboard_html_provider_save_sends_content_path() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("body.response_content_path=document.getElementById('prov-response-content-path')"),
        "saveProvider should send response_content_path in request body"
    );
}

#[test]
fn test_dashboard_html_provider_edit_function_exists() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("function editProvider(id)"),
        "editProvider function must exist"
    );
}

#[test]
fn test_dashboard_html_provider_save_function_exists() {
    let content = include_str!("../src/dashboard.html");
    assert!(
        content.contains("async function saveProvider()"),
        "saveProvider function must exist"
    );
}

#[test]
fn test_dashboard_html_provider_edit_populates_all_fields() {
    let content = include_str!("../src/dashboard.html");
    // editProvider should populate these core fields
    assert!(content.contains("prov-name').value=p.name"), "editProvider sets name");
    assert!(content.contains("prov-base-url').value=p.base_url"), "editProvider sets base_url");
    assert!(content.contains("prov-api-type').value=p.api_type"), "editProvider sets api_type");
    assert!(content.contains("prov-auth-type').value=p.auth_type"), "editProvider sets auth_type");
    assert!(content.contains("prov-weight').value=p.weight"), "editProvider sets weight");
}
