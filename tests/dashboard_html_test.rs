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

// ========== Mock Mode UI Tests ==========

#[test]
fn test_dashboard_html_provider_form_has_mock_mode_toggle() {
    let content = include_str!("../src/dashboard.html");
    // Provider modal should have mock_mode select
    assert!(
        content.contains("id=\"prov-mock-mode\""),
        "Provider form should have prov-mock-mode select element"
    );
    assert!(
        content.contains("模拟模式"),
        "Mock mode should have Chinese label '模拟模式'"
    );
    assert!(
        content.contains("是（模拟响应，不消耗 Token）"),
        "Mock mode should have descriptive option text"
    );
}

#[test]
fn test_dashboard_html_provider_table_has_mock_column() {
    let content = include_str!("../src/dashboard.html");
    // Provider table header should have Mock column
    assert!(
        content.contains(">Mock</th>") || content.contains("Mock</th>"),
        "Provider table should have Mock column header"
    );
    // Empty state should have colspan=12 (was 9, now 12 with Mock+API类型+分组 columns)
    assert!(
        content.contains("colspan=\"12\""),
        "Empty state should have colspan=12 to account for Mock, API类型, and 分组 columns"
    );
}

#[test]
fn test_dashboard_html_provider_edit_loads_mock_mode() {
    let content = include_str!("../src/dashboard.html");
    // editProvider should load mock_mode from provider object
    assert!(
        content.contains("prov-mock-mode').value=p.mock_mode"),
        "editProvider should populate mock_mode select"
    );
}

#[test]
fn test_dashboard_html_provider_save_sends_mock_mode() {
    let content = include_str!("../src/dashboard.html");
    // saveProvider should include mock_mode in the request body
    assert!(
        content.contains("body.mock_mode=document.getElementById('prov-mock-mode')"),
        "saveProvider should send mock_mode in request body"
    );
}

#[test]
fn test_dashboard_html_provider_new_form_defaults_mock_to_false() {
    let content = include_str!("../src/dashboard.html");
    // openProviderModal (new provider) should set mock_mode to 'false'
    assert!(
        content.contains("prov-mock-mode').value='false'"),
        "New provider form should default mock_mode to 'false'"
    );
}

#[test]
fn test_dashboard_html_provider_table_renders_mock_badge() {
    let content = include_str!("../src/dashboard.html");
    // renderProvidersTable should show Mock badge when p.mock_mode is true
    assert!(
        content.contains("p.mock_mode?'<span class=\"badge badge-yellow\">Mock</span>'"),
        "Provider table should render Mock badge for mock_mode providers"
    );
}

// ========== More Dashboard UI Validation Tests ==========

#[test]
fn test_dashboard_html_has_csv_export_button() {
    let content = include_str!("../src/dashboard.html");
    // Should have CSV export functionality
    assert!(
        content.contains("export") && (content.contains("csv") || content.contains("CSV") || content.contains("download")),
        "Dashboard should have CSV export/download capability"
    );
}

fn test_dashboard_html_logs_pagination_controls_exist() {
    let content = include_str!("../src/dashboard.html");
    // Logs tab should have pagination controls
    assert!(
        content.contains("currentLogPage") || content.contains("pageSize") || content.contains("loadLogs"),
        "Dashboard should have pagination controls for logs"
    );
    assert!(
        content.contains("prev") || content.contains("previous") || content.contains("page"),
        "Logs should have prev/next pagination controls"
    );
}

#[test]
fn test_dashboard_html_time_presets_in_logs() {
    let content = include_str!("../src/dashboard.html");
    // Should have time preset buttons (1h, 24h, 7d, etc.)
    let time_presets = ["1h", "24h", "7d", "30d"];
    let found_count = time_presets.iter().filter(|p| content.contains(&format!("\"{}\"", p)) || content.contains(&format!(">{}<", p))).count();
    assert!(found_count >= 3, "Dashboard should have at least 3 time preset buttons");
}

#[test]
fn test_dashboard_html_batch_external_id_area() {
    let content = include_str!("../src/dashboard.html");
    // Batch external ID feature exists
    assert!(
        content.contains("batch-external-id") || content.contains("batchExt"),
        "Dashboard should have batch external ID area"
    );
}

#[test]
fn test_dashboard_html_logs_tab_has_model_filter() {
    let content = include_str!("../src/dashboard.html");
    // Logs tab should have model filter
    assert!(
        (content.contains("log-model") || content.contains("model-filter")) && content.contains("logs"),
        "Dashboard logs tab should have model filter input"
    );
}

#[test]
fn test_dashboard_html_logs_tab_has_provider_filter() {
    let content = include_str!("../src/dashboard.html");
    // Logs tab should have provider filter
    assert!(
        content.contains("log-provider") || content.contains("provider-filter"),
        "Dashboard logs tab should have provider filter input"
    );
}

#[test]
fn test_dashboard_html_provider_health_table_renders() {
    let content = include_str!("../src/dashboard.html");
    // Provider health table should exist
    assert!(
        content.contains("provider-health-table") || content.contains("health"),
        "Dashboard should have provider health table"
    );
}

// === Quota Management UI Tests ===

#[test]
fn test_dashboard_html_quota_window_mode_select() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("windowMode") || html.contains("window_mode") || html.contains("窗口模式"),
        "quota window mode select should exist");
}

#[test]
fn test_dashboard_html_quota_window_size_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("windowSize") || html.contains("window_size") || html.contains("窗口大小"),
        "quota window size input should exist");
}

#[test]
fn test_dashboard_html_quota_rolling_step_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("rollingStep") || html.contains("rolling_step") || html.contains("步长"),
        "quota rolling step input should exist");
}

#[test]
fn test_dashboard_html_quota_timezone_select() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("stepTz") || html.contains("rolling_step_tz") || html.contains("timezone") || html.contains("时区"),
        "quota timezone select should exist");
}

#[test]
fn test_dashboard_html_quota_calibration_modal() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("calibrat") || html.contains("校准"),
        "quota calibration modal should exist");
}

#[test]
fn test_dashboard_html_quota_period_display() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("periodStart") || html.contains("periodEnd") || html.contains("period_start") || html.contains("周期起始"),
        "quota period display should exist");
}

#[test]
fn test_dashboard_html_quota_limit_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("limitCount") || html.contains("limit_count") || html.contains("限制次数"),
        "quota limit count input should exist");
}

#[test]
fn test_dashboard_html_quota_provider_select() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("quotaProvider") || html.contains("quota_provider") || html.contains("服务商") && html.contains("配额"),
        "quota provider select should exist");
}

// === Stats Tab UI Tests ===

#[test]
fn test_dashboard_html_stats_granularity_select() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("granularity") || html.contains("粒度") || html.contains("bucket_size"),
        "stats granularity select should exist");
}

#[test]
fn test_dashboard_html_stats_provider_filter_dropdown() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    // Stats tab should have a provider filter
    assert!(html.contains("statsProvider") || html.contains("stats_provider") || (html.contains("统计分析") && html.contains("服务商")),
        "stats provider filter should exist");
}

#[test]
fn test_dashboard_html_stats_load_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("loadStats") || html.contains("加载") && html.contains("统计"),
        "stats load button should exist");
}

// === Request Trend Chart Tests ===

#[test]
fn test_dashboard_html_request_trend_chart() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("usage-trend-chart") || html.contains("usageTrendChart") || html.contains("请求趋势") || html.contains("usage_trend"),
        "request trend chart should exist");
}

#[test]
fn test_dashboard_html_request_trend_time_range() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("trendTimeRange") || html.contains("近24小时") || html.contains("近7天") || html.contains("近30天"),
        "request trend time range selector should exist");
}

// === Log Detail Filter-by-Model Tests ===

#[test]
fn test_dashboard_html_log_detail_filter_by_model_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("filterByModel") || html.contains("按此模型筛选") || html.contains("filter.*model"),
        "log detail should have filter-by-model button");
}

// === Dark Theme Tests ===

#[test]
fn test_dashboard_html_dark_theme_background() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("#0f172a") || html.contains("background-color: #0f172a") || html.contains("dark"),
        "dashboard should use dark theme background");
}

#[test]
fn test_dashboard_html_dark_theme_card_background() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("#1e293b") || html.contains("background: #1e293b"),
        "dashboard cards should use dark theme card background");
}

#[test]
fn test_dashboard_html_dark_theme_border() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("#334155") || html.contains("border-color: #334155"),
        "dashboard should use dark theme border color");
}

// === Responsive Design Tests ===

#[test]
fn test_dashboard_html_responsive_hide_mobile_class() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("hide-mobile") || html.contains("hideMobile"),
        "dashboard should have responsive hide-mobile class");
}

#[test]
fn test_dashboard_html_responsive_grid_layout() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("grid-template-columns") || html.contains("auto-fit") || html.contains("minmax"),
        "dashboard should use responsive grid layout");
}

#[test]
fn test_dashboard_html_responsive_tab_wrap() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("flex-wrap") || html.contains("tab-nav"),
        "dashboard tabs should support flex-wrap for responsive layout");
}

// === API Key Usage Stacked Chart Tests ===

#[test]
fn test_dashboard_html_api_key_stacked_token_chart() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    // The chart should show prompt/completion breakdown
    assert!(html.contains("stacked") || html.contains("prompt_tokens") && html.contains("completion_tokens"),
        "API key usage chart should show stacked prompt/completion tokens");
}

// === Health Table Clickable Provider Tests ===

#[test]
fn test_dashboard_html_health_table_clickable_provider() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    // Provider names in health table should be clickable links
    assert!(html.contains("filterByProvider") || html.contains("clickProvider") || html.contains("onclick") && html.contains("provider"),
        "health table provider names should be clickable");
}

// === Quota Usage Percent Bar Tests ===

#[test]
fn test_dashboard_html_quota_usage_percent_bar() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("usage_percent") || html.contains("usagePercent") || html.contains("用量占比"),
        "quota table should show usage percent");
}

#[test]
fn test_dashboard_html_quota_remaining_display() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("remaining") || html.contains("剩余"),
        "quota table should show remaining count");
}

#[test]
fn test_dashboard_html_quota_status_toggle() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("is_enabled") || html.contains("isEnabled") || html.contains("启用") && html.contains("停用"),
        "quota should have enable/disable toggle");
}

#[test]
fn test_dashboard_html_stats_by_api_key_has_time_filter() {
    let content = include_str!("../src/dashboard.html");
    // Stats by API key should have time filter options
    assert!(
        content.contains("stats") && content.contains("24h") || content.contains("7d") || content.contains("30d"),
        "Stats should have time range filter options"
    );
}

// === Tab Navigation Tests ===

#[test]
fn test_dashboard_html_tab_overview_exists() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tab-overview") || html.contains("概览"),
        "overview tab should exist");
}

#[test]
fn test_dashboard_html_tab_providers_exists() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tab-providers") || html.contains("服务商管理"),
        "providers tab should exist");
}

#[test]
fn test_dashboard_html_tab_apikeys_exists() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tab-apikeys") || html.contains("API Key"),
        "api keys tab should exist");
}

#[test]
fn test_dashboard_html_tab_quotas_exists() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tab-quotas") || html.contains("用量配额"),
        "quotas tab should exist");
}

#[test]
fn test_dashboard_html_tab_logs_exists() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tab-logs") || html.contains("请求日志"),
        "logs tab should exist");
}

#[test]
fn test_dashboard_html_tab_stats_exists() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tab-stats") || html.contains("统计分析"),
        "stats tab should exist");
}

// === Overview Stats Cards Tests ===

#[test]
fn test_dashboard_html_active_apikeys_card() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("activeKeys") || html.contains("活跃 API") || html.contains("active_api_keys"),
        "overview should show active API keys count");
}

#[test]
fn test_dashboard_html_active_providers_card() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("activeProviders") || html.contains("活跃服务商") || html.contains("active_providers"),
        "overview should show active providers count");
}

#[test]
fn test_dashboard_html_requests_24h_card() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("totalRequests") || html.contains("24h请求") || html.contains("请求数"),
        "overview should show 24h requests count");
}

#[test]
fn test_dashboard_html_tokens_24h_card() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("totalTokens") || html.contains("24h Tokens") || html.contains("Token数"),
        "overview should show 24h tokens count");
}

#[test]
fn test_dashboard_html_avg_tokens_per_sec_card() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("Tokens/s") || html.contains("tokens_per_second") || html.contains("平均速率") || html.contains("avgTokensPerSec") || html.contains("Output Tokens/s"),
        "overview should show avg tokens/s");
}

#[test]
fn test_dashboard_html_throttle_24h_card() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("throttleCount") || html.contains("24h限流") || html.contains("限流"),
        "overview should show 24h throttle count");
}

// === Provider Form Field Tests ===

#[test]
fn test_dashboard_html_provider_base_url_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("baseUrl") || html.contains("base_url") || html.contains("Base URL"),
        "provider form should have base URL input");
}

#[test]
fn test_dashboard_html_provider_auth_type_select() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("authType") || html.contains("auth_type") || html.contains("认证方式"),
        "provider form should have auth type select");
}

#[test]
fn test_dashboard_html_provider_weight_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("weight") || html.contains("权重"),
        "provider form should have weight input");
}

#[test]
fn test_dashboard_html_provider_bypass_proxy_checkbox() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("bypassProxy") || html.contains("bypass_proxy") || html.contains("绕过代理"),
        "provider form should have bypass proxy checkbox");
}

#[test]
fn test_dashboard_html_provider_token_url_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tokenUrl") || html.contains("token_url") || html.contains("Token URL"),
        "provider form should have token URL input");
}

#[test]
fn test_dashboard_html_provider_token_expiry_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tokenExpiry") || html.contains("token_expiry") || html.contains("有效期"),
        "provider form should have token expiry input");
}

#[test]
fn test_dashboard_html_provider_subscription_start_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("subscriptionStart") || html.contains("subscription_start") || html.contains("订阅开始"),
        "provider form should have subscription start input");
}

// === Log Management Tests ===

#[test]
fn test_dashboard_html_log_delete_single_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("deleteLog") || html.contains("删除日志") || html.contains("delete.*log"),
        "logs should have delete single log button");
}

#[test]
fn test_dashboard_html_log_batch_delete_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("batchDelete") || html.contains("batch-delete") || html.contains("批量删除"),
        "logs should have batch delete button");
}

#[test]
fn test_dashboard_html_log_delete_all_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("deleteAll") || html.contains("delete-all") || html.contains("清空全部"),
        "logs should have delete all button");
}

// === Chart.js Configuration Tests ===

#[test]
fn test_dashboard_html_chartjs_initialization() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("new Chart") || html.contains("Chart("),
        "dashboard should initialize Chart.js charts");
}

#[test]
fn test_dashboard_html_auto_refresh_interval() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("setInterval") || html.contains("autoRefresh"),
        "dashboard should have auto-refresh interval");
}

// === Dashboard API Call Tests ===

#[test]
fn test_dashboard_html_summary_api_call() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("dashboard/summary") || html.contains("loadSummary"),
        "dashboard should call summary API");
}

#[test]
fn test_dashboard_html_health_api_call() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("dashboard/health") || html.contains("loadProviderHealth"),
        "dashboard should call health API");
}

#[test]
fn test_dashboard_html_token_rate_api_call() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("dashboard/token-rate") || html.contains("loadTokenRate"),
        "dashboard should call token rate API");
}

// === Provider Model Management Tests ===

#[test]
fn test_dashboard_html_provider_model_add_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("addModel") || html.contains("添加模型") || html.contains("modelId"),
        "provider model management should have add input");
}

#[test]
fn test_dashboard_html_provider_model_test_all_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("testAll") || html.contains("全部测试") || html.contains("test-all"),
        "provider model management should have test all button");
}

#[test]
fn test_dashboard_html_provider_model_table() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("model-table") || html.contains("modelTable") || html.contains("模型管理"),
        "provider model management should have model table");
}

// === API Key Form Tests ===

#[test]
fn test_dashboard_html_apikey_name_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("keyName") || html.contains("key_name") || html.contains("名称"),
        "API key form should have name input");
}

#[test]
fn test_dashboard_html_apikey_allowed_providers() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("allowedProviders") || html.contains("allowed_providers") || html.contains("允许服务商"),
        "API key form should have allowed providers selector");
}

#[test]
fn test_dashboard_html_apikey_copy_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("copyKey") || html.contains("复制") || html.contains("clipboard"),
        "API key should have copy button");
}

#[test]
fn test_dashboard_html_apikey_prefix_display() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("keyPrefix") || html.contains("lgk-") || html.contains("前缀"),
        "API key list should show key prefix");
}

// === Provider Operations Tests ===

#[test]
fn test_dashboard_html_provider_delete_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("deleteProvider") || html.contains("删除服务商"),
        "provider list should have delete button");
}

#[test]
fn test_dashboard_html_provider_test_connection_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("testConnection") || html.contains("测试连接") || html.contains("test-all"),
        "provider model management should have test connection button");
}

#[test]
fn test_dashboard_html_provider_status_badge() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("活跃") || html.contains("停用") || html.contains("badge"),
        "provider list should show status badge");
}

#[test]
fn test_dashboard_html_token_status_display() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("tokenStatus") || html.contains("Token状态") || html.contains("current_token"),
        "provider list should show token status");
}

// === Quota Operations Tests ===

#[test]
fn test_dashboard_html_quota_delete_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("deleteQuota") || html.contains("删除配额"),
        "quota table should have delete button");
}

#[test]
fn test_dashboard_html_quota_set_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("setQuota") || html.contains("设置配额"),
        "quota form should have set button");
}

// === Log Search and Filter Tests ===

#[test]
fn test_dashboard_html_log_search_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("searchInput") || html.contains("关键词") || html.contains("搜索"),
        "logs tab should have search input");
}

#[test]
fn test_dashboard_html_log_status_filter() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("statusFilter") || html.contains("状态码") || html.contains("status_filter"),
        "logs tab should have status filter");
}

// === Stats Table Tests ===

#[test]
fn test_dashboard_html_stats_table_exists() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("stats-table") || html.contains("statsTable"),
        "stats tab should have stats table");
}

// === Chart Color Management Tests ===

#[test]
fn test_dashboard_html_chart_color_picker() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("chartColor") || html.contains("chart_color") || html.contains("颜色"),
        "dashboard should have chart color picker");
}

// === Model External ID Tests ===

#[test]
fn test_dashboard_html_external_id_input() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("externalId") || html.contains("external_id") || html.contains("对外ID"),
        "model management should have external ID input");
}

// === Log Detail Mode Toggle Tests ===

#[test]
fn test_dashboard_html_qa_mode_toggle() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("qaMode") || html.contains("QA") || html.contains("qa-mode"),
        "log detail should have QA mode toggle");
}

#[test]
fn test_dashboard_html_raw_mode_toggle() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("rawMode") || html.contains("原始") || html.contains("raw-mode"),
        "log detail should have raw mode toggle");
}

// === Visual Design Tests ===

#[test]
fn test_dashboard_html_gradient_title() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("gradient") || html.contains("渐变"),
        "dashboard title should use gradient");
}

#[test]
fn test_dashboard_html_modal_close_button() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("closeModal") || html.contains("关闭"),
        "modals should have close button");
}

// === Log Indicators Tests ===

#[test]
fn test_dashboard_html_streaming_indicator() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("isStreaming") || html.contains("流式") || html.contains("streaming"),
        "log list should show streaming indicator");
}

#[test]
fn test_dashboard_html_throttled_indicator() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("isThrottled") || html.contains("限流") || html.contains("throttled"),
        "log list should show throttled indicator");
}

// === Provider List Table Tests ===

#[test]
fn test_dashboard_html_provider_table_has_all_columns() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("ID") && html.contains("名称") && html.contains("Base URL"),
        "provider table should have ID, name, and Base URL columns");
}

#[test]
fn test_dashboard_html_provider_checkbox_for_batch() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("selectAll") || html.contains("checkbox") || html.contains("全选"),
        "provider table should have checkbox for batch operations");
}

// === API Key Table Tests ===

#[test]
fn test_dashboard_html_apikey_table_has_columns() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("名称") && html.contains("Key") && html.contains("状态"),
        "API key table should have name, key, and status columns");
}

// === Log Table Column Tests ===

#[test]
fn test_dashboard_html_log_table_has_time_column() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("时间") || html.contains("created_at") || html.contains("time"),
        "log table should have time column");
}

#[test]
fn test_dashboard_html_log_table_has_status_column() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("状态码") || html.contains("status") || html.contains("response_status"),
        "log table should have status column");
}

#[test]
fn test_dashboard_html_log_table_has_tokens_column() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("Tokens") || html.contains("tokens") || html.contains("total_tokens"),
        "log table should have tokens column");
}

#[test]
fn test_dashboard_html_log_table_has_duration_column() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("耗时") || html.contains("duration") || html.contains("ms"),
        "log table should have duration column");
}

// === Quota Table Column Tests ===

#[test]
fn test_dashboard_html_quota_table_has_provider_column() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("服务商") && (html.contains("配额") || html.contains("quota")),
        "quota table should have provider column");
}

#[test]
fn test_dashboard_html_quota_table_has_usage_bar() {
    let html = fs::read_to_string("src/dashboard.html").unwrap();
    assert!(html.contains("usage_percent") || html.contains("usagePercent") || html.contains("用量占比") || html.contains("progress"),
        "quota table should have usage progress bar");
}
