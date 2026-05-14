-- API Keys table
CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,
    allowed_providers TEXT, -- JSON array of provider IDs, null means all
    is_active BOOLEAN NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Providers table
CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_type TEXT NOT NULL DEFAULT 'openai', -- openai, anthropic, custom
    auth_type TEXT NOT NULL DEFAULT 'api_key', -- api_key, dynamic_token
    api_key TEXT, -- static API key (if auth_type = api_key)
    -- Dynamic token fields
    token_url TEXT,
    token_username TEXT,
    token_password TEXT,
    token_field TEXT DEFAULT 'token',
    refresh_token_field TEXT DEFAULT 'refreshToken',
    token_header_field TEXT DEFAULT 'Authorization',
    token_header_prefix TEXT DEFAULT 'Bearer ',
    token_expiry_seconds INTEGER DEFAULT 86400,
    current_token TEXT,
    current_refresh_token TEXT,
    token_expires_at TEXT,
    --
    is_active BOOLEAN NOT NULL DEFAULT 1,
    weight INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Request logs table
CREATE TABLE IF NOT EXISTS request_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    api_key_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model TEXT,
    request_path TEXT NOT NULL,
    request_method TEXT NOT NULL DEFAULT 'POST',
    request_headers TEXT, -- JSON
    request_body TEXT, -- JSON
    response_status INTEGER,
    response_headers TEXT, -- JSON
    response_body TEXT, -- JSON (truncated for large responses)
    prompt_tokens INTEGER DEFAULT 0,
    completion_tokens INTEGER DEFAULT 0,
    total_tokens INTEGER DEFAULT 0,
    duration_ms INTEGER,
    is_streaming BOOLEAN DEFAULT 0,
    is_throttled BOOLEAN DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (api_key_id) REFERENCES api_keys(id),
    FOREIGN KEY (provider_id) REFERENCES providers(id)
);

-- Token rate snapshots (for real-time dashboard)
CREATE TABLE IF NOT EXISTS token_rate_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT,
    tokens_per_second REAL NOT NULL,
    prompt_tokens INTEGER DEFAULT 0,
    completion_tokens INTEGER DEFAULT 0,
    request_count INTEGER DEFAULT 0,
    snapshot_time TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (provider_id) REFERENCES providers(id)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_request_logs_api_key ON request_logs(api_key_id);
CREATE INDEX IF NOT EXISTS idx_request_logs_provider ON request_logs(provider_id);
CREATE INDEX IF NOT EXISTS idx_request_logs_created_at ON request_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_request_logs_api_key_created ON request_logs(api_key_id, created_at);
CREATE INDEX IF NOT EXISTS idx_token_rate_snapshots_time ON token_rate_snapshots(snapshot_time);
CREATE INDEX IF NOT EXISTS idx_token_rate_snapshots_provider_time ON token_rate_snapshots(provider_id, snapshot_time);
