/// Dashboard module - serves the web UI and provides dashboard API endpoints
use axum::response::{Html, IntoResponse};

/// Returns the dashboard HTML page
pub async fn dashboard_page() -> impl IntoResponse {
    Html(DASHBOARD_HTML)
}

/// Dashboard HTML - a single-page application with real-time metrics
const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>LLM Gateway Dashboard</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #e2e8f0; }
        .container { max-width: 1400px; margin: 0 auto; padding: 20px; }
        h1 { text-align: center; margin: 20px 0; font-size: 2em; background: linear-gradient(135deg, #60a5fa, #a78bfa); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
        .tabs { display: flex; gap: 4px; margin-bottom: 20px; background: #1e293b; border-radius: 8px; padding: 4px; }
        .tab { padding: 10px 20px; border: none; background: transparent; color: #94a3b8; cursor: pointer; border-radius: 6px; font-size: 14px; }
        .tab.active { background: #3b82f6; color: white; }
        .tab:hover { background: #334155; }
        .tab.active:hover { background: #3b82f6; }
        .stats-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px; margin-bottom: 24px; }
        .stat-card { background: #1e293b; border-radius: 12px; padding: 20px; border: 1px solid #334155; }
        .stat-card .label { font-size: 12px; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em; }
        .stat-card .value { font-size: 28px; font-weight: 700; margin-top: 4px; }
        .stat-card .value.blue { color: #60a5fa; }
        .stat-card .value.green { color: #34d399; }
        .stat-card .value.yellow { color: #fbbf24; }
        .stat-card .value.red { color: #f87171; }
        .panel { background: #1e293b; border-radius: 12px; padding: 20px; border: 1px solid #334155; margin-bottom: 24px; }
        .panel h2 { font-size: 16px; margin-bottom: 16px; color: #e2e8f0; }
        canvas { width: 100% !important; height: 300px !important; }
        table { width: 100%; border-collapse: collapse; }
        th, td { padding: 10px 12px; text-align: left; border-bottom: 1px solid #334155; font-size: 13px; }
        th { color: #94a3b8; font-weight: 600; }
        .controls { display: flex; gap: 12px; margin-bottom: 16px; flex-wrap: wrap; align-items: center; }
        .controls label { font-size: 13px; color: #94a3b8; }
        .controls select, .controls input { background: #0f172a; border: 1px solid #334155; color: #e2e8f0; padding: 6px 10px; border-radius: 6px; font-size: 13px; }
        .controls button { background: #3b82f6; color: white; border: none; padding: 6px 16px; border-radius: 6px; cursor: pointer; font-size: 13px; }
        .controls button:hover { background: #2563eb; }
        .pagination { display: flex; gap: 8px; margin-top: 12px; justify-content: center; }
        .pagination button { background: #334155; color: #e2e8f0; border: none; padding: 6px 12px; border-radius: 4px; cursor: pointer; }
        .pagination button.active { background: #3b82f6; }
        .hidden { display: none; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 LLM Gateway Dashboard</h1>

        <div class="tabs">
            <button class="tab active" onclick="showTab('overview')">Overview</button>
            <button class="tab" onclick="showTab('realtime')">Real-time Rate</button>
            <button class="tab" onclick="showTab('logs')">Request Logs</button>
            <button class="tab" onclick="showTab('longterm')">Long-term Stats</button>
            <button class="tab" onclick="showTab('throttle')">Throttle Stats</button>
        </div>

        <!-- Overview Tab -->
        <div id="tab-overview">
            <div class="stats-grid" id="summary-stats"></div>
            <div class="panel">
                <h2>Provider Status</h2>
                <table id="provider-table">
                    <thead><tr><th>ID</th><th>Name</th><th>Auth Type</th><th>Token Valid</th><th>Requests</th><th>Tokens</th><th>Throttled</th></tr></thead>
                    <tbody></tbody>
                </table>
            </div>
            <div class="panel">
                <h2>API Keys</h2>
                <table id="apikey-table">
                    <thead><tr><th>Name</th><th>Prefix</th><th>Providers</th><th>Active</th><th>Created</th></tr></thead>
                    <tbody></tbody>
                </table>
            </div>
        </div>

        <!-- Real-time Rate Tab -->
        <div id="tab-realtime" class="hidden">
            <div class="controls">
                <label>Provider:</label>
                <select id="rate-provider"><option value="">All</option></select>
                <label>Time range:</label>
                <select id="rate-range">
                    <option value="60">1 min</option>
                    <option value="300">5 min</option>
                    <option value="900" selected>15 min</option>
                    <option value="1800">30 min</option>
                    <option value="3600">1 hour</option>
                </select>
            </div>
            <div class="panel">
                <h2>Token Rate (tokens/s)</h2>
                <canvas id="rate-chart"></canvas>
            </div>
        </div>

        <!-- Request Logs Tab -->
        <div id="tab-logs" class="hidden">
            <div class="controls">
                <label>API Key:</label>
                <select id="log-apikey"><option value="">All</option></select>
                <label>Provider:</label>
                <select id="log-provider"><option value="">All</option></select>
                <label>From:</label>
                <input type="datetime-local" id="log-start">
                <label>To:</label>
                <input type="datetime-local" id="log-end">
                <button onclick="loadLogs()">Search</button>
            </div>
            <div class="panel">
                <table id="logs-table">
                    <thead><tr><th>Time</th><th>API Key</th><th>Provider</th><th>Model</th><th>Status</th><th>Tokens</th><th>Duration</th><th>Streaming</th><th>Throttled</th></tr></thead>
                    <tbody></tbody>
                </table>
                <div class="pagination" id="logs-pagination"></div>
            </div>
        </div>

        <!-- Long-term Stats Tab -->
        <div id="tab-longterm" class="hidden">
            <div class="controls">
                <label>Granularity:</label>
                <select id="lt-granularity">
                    <option value="5h">5 Hours</option>
                    <option value="day" selected>Day</option>
                    <option value="week">Week</option>
                    <option value="month">Month</option>
                </select>
                <label>API Key:</label>
                <select id="lt-apikey"><option value="">All</option></select>
                <label>Provider:</label>
                <select id="lt-provider"><option value="">All</option></select>
                <button onclick="loadLongTermStats()">Load</button>
            </div>
            <div class="panel">
                <h2>Request Count</h2>
                <canvas id="lt-requests-chart"></canvas>
            </div>
            <div class="panel">
                <h2>Token Consumption</h2>
                <canvas id="lt-tokens-chart"></canvas>
            </div>
        </div>

        <!-- Throttle Stats Tab -->
        <div id="tab-throttle" class="hidden">
            <div class="controls">
                <label>Granularity:</label>
                <select id="th-granularity">
                    <option value="5h">5 Hours</option>
                    <option value="day" selected>Day</option>
                    <option value="week">Week</option>
                    <option value="month">Month</option>
                </select>
                <label>Provider:</label>
                <select id="th-provider"><option value="">All</option></select>
                <button onclick="loadThrottleStats()">Load</button>
            </div>
            <div class="panel">
                <h2>Throttle Count by Provider</h2>
                <canvas id="th-chart"></canvas>
            </div>
            <div class="panel">
                <h2>Throttle Details</h2>
                <table id="th-table">
                    <thead><tr><th>Period</th><th>Provider</th><th>Throttle Count</th><th>Error Count</th></tr></thead>
                    <tbody></tbody>
                </table>
            </div>
        </div>
    </div>

    <script>
        const API_BASE = '/api/v1';

        function showTab(name) {
            document.querySelectorAll('[id^="tab-"]').forEach(el => el.classList.add('hidden'));
            document.getElementById('tab-' + name).classList.remove('hidden');
            document.querySelectorAll('.tab').forEach(el => el.classList.remove('active'));
            event.target.classList.add('active');
        }

        async function loadSummary() {
            try {
                const resp = await fetch(API_BASE + '/dashboard/summary');
                const data = await resp.json();
                document.getElementById('summary-stats').innerHTML = `
                    <div class="stat-card"><div class="label">Active API Keys</div><div class="value blue">${data.active_api_keys}</div></div>
                    <div class="stat-card"><div class="label">Active Providers</div><div class="value green">${data.active_providers}</div></div>
                    <div class="stat-card"><div class="label">Requests (24h)</div><div class="value blue">${data.total_requests_24h}</div></div>
                    <div class="stat-card"><div class="label">Tokens (24h)</div><div class="value green">${data.total_tokens_24h.toLocaleString()}</div></div>
                    <div class="stat-card"><div class="label">Avg Tokens/s</div><div class="value yellow">${data.avg_tokens_per_second.toFixed(1)}</div></div>
                    <div class="stat-card"><div class="label">Throttled (24h)</div><div class="value red">${data.throttle_count_24h}</div></div>
                `;
            } catch(e) { console.error('Failed to load summary:', e); }
        }

        async function loadProviders() {
            try {
                const resp = await fetch(API_BASE + '/providers');
                const providers = await resp.json();
                const tbody = document.querySelector('#provider-table tbody');
                tbody.innerHTML = providers.map(p => `
                    <tr>
                        <td>${p.id}</td>
                        <td>${p.name}</td>
                        <td>${p.auth_type}</td>
                        <td>${p.auth_type === 'api_key' ? '✅' : (p.current_token ? '✅' : '❌')}</td>
                        <td>-</td>
                        <td>-</td>
                        <td>-</td>
                    </tr>
                `).join('');
            } catch(e) { console.error('Failed to load providers:', e); }
        }

        async function loadApiKeys() {
            try {
                const resp = await fetch(API_BASE + '/api-keys');
                const keys = await resp.json();
                const tbody = document.querySelector('#apikey-table tbody');
                tbody.innerHTML = keys.map(k => `
                    <tr>
                        <td>${k.name}</td>
                        <td>${k.key_prefix}...</td>
                        <td>${k.allowed_providers ? k.allowed_providers.join(', ') : 'All'}</td>
                        <td>${k.is_active ? '✅' : '❌'}</td>
                        <td>${new Date(k.created_at).toLocaleString()}</td>
                    </tr>
                `).join('');
            } catch(e) { console.error('Failed to load API keys:', e); }
        }

        async function loadLogs() {
            try {
                const params = new URLSearchParams();
                const apikey = document.getElementById('log-apikey').value;
                const provider = document.getElementById('log-provider').value;
                const start = document.getElementById('log-start').value;
                const end = document.getElementById('log-end').value;
                if (apikey) params.set('api_key_id', apikey);
                if (provider) params.set('provider_id', provider);
                if (start) params.set('start_time', new Date(start).toISOString());
                if (end) params.set('end_time', new Date(end).toISOString());

                const resp = await fetch(API_BASE + '/logs?' + params);
                const data = await resp.json();
                const tbody = document.querySelector('#logs-table tbody');
                tbody.innerHTML = (data.logs || []).map(l => `
                    <tr>
                        <td>${new Date(l.created_at).toLocaleString()}</td>
                        <td>${l.api_key_id.slice(0,8)}...</td>
                        <td>${l.provider_id}</td>
                        <td>${l.model || '-'}</td>
                        <td>${l.response_status || '-'}</td>
                        <td>${l.total_tokens}</td>
                        <td>${l.duration_ms ? l.duration_ms + 'ms' : '-'}</td>
                        <td>${l.is_streaming ? '✅' : '❌'}</td>
                        <td>${l.is_throttled ? '⚠️' : '✅'}</td>
                    </tr>
                `).join('');
            } catch(e) { console.error('Failed to load logs:', e); }
        }

        async function loadLongTermStats() {
            // TODO: Implement with Chart.js
            console.log('Loading long-term stats...');
        }

        async function loadThrottleStats() {
            // TODO: Implement with Chart.js
            console.log('Loading throttle stats...');
        }

        // Initial load
        loadSummary();
        loadProviders();
        loadApiKeys();

        // Auto-refresh every 30 seconds
        setInterval(loadSummary, 30000);
    </script>
</body>
</html>"#;
