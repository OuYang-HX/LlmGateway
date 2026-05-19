use sqlx::sqlite::{SqlitePoolOptions, SqlitePool};
use serde::Serialize;

/// Database connection pool wrapper
#[derive(Debug, Clone)]
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    /// Create a new database connection with migrations
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .after_connect(|conn, _meta| Box::pin(async move {
                // Disable strict type checking for SQLite
                sqlx::query("PRAGMA strict = OFF").execute(&mut *conn).await.ok();
                // Disable foreign key enforcement - we handle referential integrity in application code
                // This allows provider ID changes with cascade updates to child tables
                sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.ok();
                Ok(())
            }))
            .connect(database_url)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Create an in-memory database for testing
    pub async fn new_in_memory() -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .after_connect(|conn, _meta| Box::pin(async move {
                sqlx::query("PRAGMA strict = OFF").execute(&mut *conn).await.ok();
                sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.ok();
                Ok(())
            }))
            .connect("sqlite::memory:")
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        // Create tables inline (sqlx migrate requires specific directory structure)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                api_key TEXT NOT NULL UNIQUE,
                key_prefix TEXT NOT NULL,
                allowed_providers TEXT,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_type TEXT NOT NULL DEFAULT 'openai',
                auth_type TEXT NOT NULL DEFAULT 'api_key',
                api_key TEXT,
                token_url TEXT,
                token_username TEXT,
                token_password TEXT,
                token_request_method TEXT DEFAULT 'POST',
                token_content_type TEXT DEFAULT 'json',
                token_username_field TEXT DEFAULT 'username',
                token_password_field TEXT DEFAULT 'password',
                token_body_template TEXT,
                token_extra_headers TEXT,
                token_cookies TEXT,
                token_field TEXT DEFAULT 'token',
                refresh_token_field TEXT DEFAULT 'refreshToken',
                token_header_field TEXT DEFAULT 'Authorization',
                token_header_prefix TEXT DEFAULT 'Bearer ',
                token_expiry_seconds INTEGER DEFAULT 86400,
                current_token TEXT,
                current_refresh_token TEXT,
                token_expires_at TEXT,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                weight INTEGER NOT NULL DEFAULT 1,
                bypass_proxy BOOLEAN NOT NULL DEFAULT 0,
                response_content_path TEXT DEFAULT 'choices.0.message.content',
                response_reasoning_path TEXT DEFAULT 'choices.0.delta.reasoning_content',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS request_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                api_key_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                model TEXT,
                request_path TEXT NOT NULL,
                request_method TEXT NOT NULL DEFAULT 'POST',
                request_headers TEXT,
                request_body TEXT,
                response_status INTEGER,
                response_headers TEXT,
                response_body TEXT,
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

            CREATE TABLE IF NOT EXISTS token_rate_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id TEXT,
                tokens_per_second REAL NOT NULL,
                prompt_tokens INTEGER DEFAULT 0,
                completion_tokens INTEGER DEFAULT 0,
                request_count INTEGER DEFAULT 0,
                elapsed_seconds REAL DEFAULT 10,
                snapshot_time TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (provider_id) REFERENCES providers(id)
            );

            CREATE TABLE IF NOT EXISTS provider_models (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                last_test_status TEXT,
                last_test_message TEXT,
                last_tested_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE,
                UNIQUE(provider_id, model_id)
            );

            CREATE TABLE IF NOT EXISTS models (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                model_type TEXT NOT NULL DEFAULT 'chat',
                is_active BOOLEAN NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 0,
                config TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS model_mappings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                provider_model_id TEXT NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                weight INTEGER NOT NULL DEFAULT 1,
                cost_multiplier REAL NOT NULL DEFAULT 1.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE,
                UNIQUE(model_id, provider_id)
            );

            CREATE INDEX IF NOT EXISTS idx_models_active ON models(is_active);
            CREATE INDEX IF NOT EXISTS idx_model_mappings_model ON model_mappings(model_id);
            CREATE INDEX IF NOT EXISTS idx_model_mappings_provider ON model_mappings(provider_id);
            CREATE INDEX IF NOT EXISTS idx_model_mappings_active ON model_mappings(model_id, is_active);
            CREATE INDEX IF NOT EXISTS idx_provider_models_provider ON provider_models(provider_id);

            CREATE INDEX IF NOT EXISTS idx_request_logs_api_key ON request_logs(api_key_id);
            CREATE INDEX IF NOT EXISTS idx_request_logs_provider ON request_logs(provider_id);
            CREATE INDEX IF NOT EXISTS idx_request_logs_created_at ON request_logs(created_at);
            CREATE INDEX IF NOT EXISTS idx_request_logs_api_key_created ON request_logs(api_key_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_token_rate_snapshots_time ON token_rate_snapshots(snapshot_time);
            CREATE INDEX IF NOT EXISTS idx_token_rate_snapshots_provider_time ON token_rate_snapshots(provider_id, snapshot_time);

            CREATE TABLE IF NOT EXISTS provider_quotas (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id TEXT NOT NULL,
                quota_type TEXT NOT NULL,
                window_mode TEXT NOT NULL DEFAULT 'fixed',
                window_size TEXT NOT NULL DEFAULT '5h',
                window_start_override TEXT,
                limit_count INTEGER NOT NULL,
                is_enabled BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE,
                UNIQUE(provider_id, quota_type)
            );

            CREATE TABLE IF NOT EXISTS provider_quota_calibrations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id TEXT NOT NULL,
                quota_type TEXT NOT NULL,
                calibration_offset INTEGER NOT NULL DEFAULT 0,
                calibrated_at TEXT NOT NULL DEFAULT (datetime('now')),
                calibration_window_start TEXT,
                calibration_window_end TEXT,
                note TEXT,
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE,
                UNIQUE(provider_id, quota_type)
            );

            CREATE INDEX IF NOT EXISTS idx_provider_quotas_provider ON provider_quotas(provider_id);
            CREATE INDEX IF NOT EXISTS idx_provider_quota_calibrations_provider ON provider_quota_calibrations(provider_id);
            "#
        )
        .execute(&self.pool)
        .await?;

        // Migration: Add response_content_path column to providers table
        let _ = sqlx::query(
            "ALTER TABLE providers ADD COLUMN response_content_path TEXT DEFAULT 'choices.0.message.content'"
        )
        .execute(&self.pool)
        .await;

        // Migration: Add response_reasoning_path column
        let _ = sqlx::query(
            "ALTER TABLE providers ADD COLUMN response_reasoning_path TEXT DEFAULT 'choices.0.delta.reasoning_content'"
        )
        .execute(&self.pool)
        .await;

        // Migration: Add elapsed_seconds column to token_rate_snapshots
        let _ = sqlx::query(
            "ALTER TABLE token_rate_snapshots ADD COLUMN elapsed_seconds REAL DEFAULT 10"
        )
        .execute(&self.pool)
        .await;

        // Migration: Add chart_color column to providers table
        let _ = sqlx::query(
            "ALTER TABLE providers ADD COLUMN chart_color TEXT"
        )
        .execute(&self.pool)
        .await;

        // Migration: Add window_mode and window_size columns to provider_quotas
        let _ = sqlx::query(
            "ALTER TABLE provider_quotas ADD COLUMN window_mode TEXT NOT NULL DEFAULT 'fixed'"
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE provider_quotas ADD COLUMN window_size TEXT NOT NULL DEFAULT '5h'"
        )
        .execute(&self.pool)
        .await;

        // Migration: Add calibration_window_start and calibration_window_end columns
        let _ = sqlx::query(
            "ALTER TABLE provider_quota_calibrations ADD COLUMN calibration_window_start TEXT"
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE provider_quota_calibrations ADD COLUMN calibration_window_end TEXT"
        )
        .execute(&self.pool)
        .await;

        // Migration: Add window_start_override column to provider_quotas
        let _ = sqlx::query(
            "ALTER TABLE provider_quotas ADD COLUMN window_start_override TEXT"
        )
        .execute(&self.pool)
        .await;

        // Migration: Update legacy quota_type values to new format
        // "5h" → "sliding:5h", "weekly" → "fixed:7d", "monthly" → "fixed:30d"
        let _ = sqlx::query(
            "UPDATE provider_quotas SET quota_type = 'sliding:5h', window_mode = 'sliding', window_size = '5h' WHERE quota_type = '5h'"
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "UPDATE provider_quotas SET quota_type = 'fixed:7d', window_mode = 'fixed', window_size = '7d' WHERE quota_type = 'weekly'"
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "UPDATE provider_quotas SET quota_type = 'fixed:30d', window_mode = 'fixed', window_size = '30d' WHERE quota_type = 'monthly'"
        )
        .execute(&self.pool)
        .await;
        // Also update corresponding calibrations
        let _ = sqlx::query(
            "UPDATE provider_quota_calibrations SET quota_type = 'sliding:5h' WHERE quota_type = '5h'"
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "UPDATE provider_quota_calibrations SET quota_type = 'fixed:7d' WHERE quota_type = 'weekly'"
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "UPDATE provider_quota_calibrations SET quota_type = 'fixed:30d' WHERE quota_type = 'monthly'"
        )
        .execute(&self.pool)
        .await;

        Ok(())
    }

    // ========== API Key operations ==========

    /// Create a new API key
    pub async fn create_api_key(
        &self,
        id: &str,
        name: &str,
        api_key: &str,
        key_prefix: &str,
        allowed_providers: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO api_keys (id, name, api_key, key_prefix, allowed_providers) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(id)
        .bind(name)
        .bind(api_key)
        .bind(key_prefix)
        .bind(allowed_providers)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get API key by the raw key value (plaintext lookup)
    pub async fn get_api_key_by_key(&self, api_key: &str) -> Result<Option<ApiKeyRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT id, name, api_key, key_prefix, allowed_providers, is_active, created_at, updated_at FROM api_keys WHERE api_key = ? AND is_active = 1"
        )
        .bind(api_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get API key by ID
    pub async fn get_api_key_by_id(&self, id: &str) -> Result<Option<ApiKeyRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT id, name, api_key, key_prefix, allowed_providers, is_active, created_at, updated_at FROM api_keys WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// List all API keys
    pub async fn list_api_keys(&self) -> Result<Vec<ApiKeyRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ApiKeyRow>(
            "SELECT id, name, api_key, key_prefix, allowed_providers, is_active, created_at, updated_at FROM api_keys ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Deactivate an API key
    pub async fn deactivate_api_key(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE api_keys SET is_active = 0, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update an API key's name and allowed_providers
    pub async fn update_api_key(
        &self,
        id: &str,
        name: &str,
        allowed_providers: Option<&str>,
        is_active: bool,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE api_keys SET name = ?, allowed_providers = ?, is_active = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(name)
        .bind(allowed_providers)
        .bind(is_active)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete an API key
    pub async fn delete_api_key(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM api_keys WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Regenerate an API key - updates the key and prefix, returns the new raw key
    pub async fn regenerate_api_key(&self, id: &str, new_api_key: &str, new_key_prefix: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE api_keys SET api_key = ?, key_prefix = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(new_api_key)
        .bind(new_key_prefix)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // ========== Provider operations ==========

    /// Create a new provider
    pub async fn create_provider(
        &self,
        id: &str,
        name: &str,
        base_url: &str,
        api_type: &str,
        auth_type: &str,
        api_key: Option<&str>,
        token_url: Option<&str>,
        token_username: Option<&str>,
        token_password: Option<&str>,
        token_request_method: Option<&str>,
        token_content_type: Option<&str>,
        token_username_field: Option<&str>,
        token_password_field: Option<&str>,
        token_body_template: Option<&str>,
        token_extra_headers: Option<&str>,
        token_cookies: Option<&str>,
        token_field: &str,
        refresh_token_field: &str,
        token_header_field: &str,
        token_header_prefix: &str,
        token_expiry_seconds: i64,
        weight: i64,
        bypass_proxy: bool,
        response_content_path: &str,
        response_reasoning_path: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO providers (id, name, base_url, api_type, auth_type, api_key, token_url, token_username, token_password, token_request_method, token_content_type, token_username_field, token_password_field, token_body_template, token_extra_headers, token_cookies, token_field, refresh_token_field, token_header_field, token_header_prefix, token_expiry_seconds, weight, bypass_proxy, response_content_path, response_reasoning_path)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(id)
        .bind(name)
        .bind(base_url)
        .bind(api_type)
        .bind(auth_type)
        .bind(api_key)
        .bind(token_url)
        .bind(token_username)
        .bind(token_password)
        .bind(token_request_method)
        .bind(token_content_type)
        .bind(token_username_field)
        .bind(token_password_field)
        .bind(token_body_template)
        .bind(token_extra_headers)
        .bind(token_cookies)
        .bind(token_field)
        .bind(refresh_token_field)
        .bind(token_header_field)
        .bind(token_header_prefix)
        .bind(token_expiry_seconds)
        .bind(weight)
        .bind(bypass_proxy)
        .bind(response_content_path)
        .bind(response_reasoning_path)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Convenience wrapper for create_provider with default token request settings
    /// (POST, json, username/password field names, no body template)
    pub async fn create_provider_simple(
        &self,
        id: &str,
        name: &str,
        base_url: &str,
        api_type: &str,
        auth_type: &str,
        api_key: Option<&str>,
        token_url: Option<&str>,
        token_username: Option<&str>,
        token_password: Option<&str>,
        token_field: &str,
        refresh_token_field: &str,
        token_header_field: &str,
        token_header_prefix: &str,
        token_expiry_seconds: i64,
        weight: i64,
    ) -> Result<(), sqlx::Error> {
        self.create_provider(
            id, name, base_url, api_type, auth_type,
            api_key, token_url, token_username, token_password,
            None, None, None, None, None, None, None,
            token_field, refresh_token_field, token_header_field, token_header_prefix,
            token_expiry_seconds, weight, false, "choices.0.message.content", "choices.0.delta.reasoning_content",
        ).await
    }

    /// Get provider by ID
    pub async fn get_provider(&self, id: &str) -> Result<Option<ProviderRow>, sqlx::Error> {
        use sqlx::Row;
        let row = sqlx::query("SELECT * FROM providers WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| Self::row_to_provider(&r)))
    }

    /// List all providers
    pub async fn list_providers(&self) -> Result<Vec<ProviderRow>, sqlx::Error> {
        use sqlx::Row;
        let rows = sqlx::query("SELECT * FROM providers ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.iter().map(Self::row_to_provider).collect())
    }

    /// List active providers
    pub async fn list_active_providers(&self) -> Result<Vec<ProviderRow>, sqlx::Error> {
        use sqlx::Row;
        let rows = sqlx::query("SELECT * FROM providers WHERE is_active = 1 ORDER BY weight DESC, name")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.iter().map(Self::row_to_provider).collect())
    }

    /// Convert a sqlx Row to ProviderRow
    fn row_to_provider(r: &sqlx::sqlite::SqliteRow) -> ProviderRow {
        use sqlx::Row;
        fn opt_str(r: &sqlx::sqlite::SqliteRow, col: &str) -> Option<String> {
            let v: Option<String> = r.try_get(col).ok();
            v.filter(|s| !s.is_empty())
        }
        ProviderRow {
            id: r.try_get("id").unwrap_or_default(),
            name: r.try_get("name").unwrap_or_default(),
            base_url: r.try_get("base_url").unwrap_or_default(),
            api_type: r.try_get("api_type").unwrap_or_default(),
            auth_type: r.try_get("auth_type").unwrap_or_default(),
            api_key: opt_str(r, "api_key"),
            token_url: opt_str(r, "token_url"),
            token_username: opt_str(r, "token_username"),
            token_password: opt_str(r, "token_password"),
            token_request_method: opt_str(r, "token_request_method"),
            token_content_type: opt_str(r, "token_content_type"),
            token_username_field: opt_str(r, "token_username_field"),
            token_password_field: opt_str(r, "token_password_field"),
            token_body_template: opt_str(r, "token_body_template"),
            token_extra_headers: opt_str(r, "token_extra_headers"),
            token_cookies: opt_str(r, "token_cookies"),
            token_field: r.try_get("token_field").unwrap_or_default(),
            refresh_token_field: r.try_get("refresh_token_field").unwrap_or_default(),
            token_header_field: r.try_get("token_header_field").unwrap_or_default(),
            token_header_prefix: r.try_get("token_header_prefix").unwrap_or_default(),
            token_expiry_seconds: r.try_get::<i64, _>("token_expiry_seconds").unwrap_or(86400),
            current_token: opt_str(r, "current_token"),
            current_refresh_token: opt_str(r, "current_refresh_token"),
            token_expires_at: opt_str(r, "token_expires_at"),
            is_active: r.try_get::<i64, _>("is_active").unwrap_or(1) != 0,
            weight: r.try_get::<i64, _>("weight").unwrap_or(1),
            bypass_proxy: r.try_get::<i64, _>("bypass_proxy").unwrap_or(0) != 0,
            response_content_path: r.try_get("response_content_path").unwrap_or_else(|_| "choices.0.message.content".to_string()),
            response_reasoning_path: r.try_get("response_reasoning_path").unwrap_or_else(|_| "choices.0.delta.reasoning_content".to_string()),
            chart_color: opt_str(r, "chart_color"),
            created_at: r.try_get("created_at").unwrap_or_default(),
            updated_at: r.try_get("updated_at").unwrap_or_default(),
        }
    }

    /// Update provider's dynamic token
    pub async fn update_provider_token(
        &self,
        id: &str,
        token: &str,
        refresh_token: Option<&str>,
        expires_at: &str,
    ) -> Result<(), sqlx::Error> {
        if let Some(rt) = refresh_token {
            sqlx::query(
                "UPDATE providers SET current_token = ?, current_refresh_token = ?, token_expires_at = ?, updated_at = datetime('now') WHERE id = ?"
            )
            .bind(token)
            .bind(rt)
            .bind(expires_at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "UPDATE providers SET current_token = ?, token_expires_at = ?, updated_at = datetime('now') WHERE id = ?"
            )
            .bind(token)
            .bind(expires_at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Update provider cookies (from Set-Cookie in auth response)
    pub async fn update_provider_cookies(&self, id: &str, cookies: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE providers SET token_cookies = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(cookies)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update provider fields using SQL UPDATE (no delete/recreate)
    /// This avoids FOREIGN KEY constraint failures with request_logs
    pub async fn update_provider(
        &self,
        old_id: &str,
        new_id: &str,
        name: &str,
        base_url: &str,
        api_type: &str,
        auth_type: &str,
        api_key: Option<&str>,
        token_url: Option<&str>,
        token_username: Option<&str>,
        token_password: Option<&str>,
        token_request_method: Option<&str>,
        token_content_type: Option<&str>,
        token_username_field: Option<&str>,
        token_password_field: Option<&str>,
        token_body_template: Option<&str>,
        token_extra_headers: Option<&str>,
        token_cookies: Option<&str>,
        token_field: &str,
        refresh_token_field: &str,
        token_header_field: &str,
        token_header_prefix: &str,
        token_expiry_seconds: i64,
        weight: i64,
        is_active: bool,
        bypass_proxy: bool,
        response_content_path: &str,
        response_reasoning_path: &str,
        chart_color: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        // If ID changed, cascade update all related tables
        // Disable FK checks temporarily since we're updating the referenced key
        if old_id != new_id {

            sqlx::query("UPDATE provider_models SET provider_id = ? WHERE provider_id = ?")
                .bind(new_id).bind(old_id).execute(&self.pool).await?;
            sqlx::query("UPDATE model_mappings SET provider_id = ? WHERE provider_id = ?")
                .bind(new_id).bind(old_id).execute(&self.pool).await?;
            sqlx::query("UPDATE request_logs SET provider_id = ? WHERE provider_id = ?")
                .bind(new_id).bind(old_id).execute(&self.pool).await?;
            sqlx::query("UPDATE token_rate_snapshots SET provider_id = ? WHERE provider_id = ?")
                .bind(new_id).bind(old_id).execute(&self.pool).await?;
        }

        sqlx::query(
            r#"UPDATE providers SET
                id = ?, name = ?, base_url = ?, api_type = ?, auth_type = ?,
                api_key = ?, token_url = ?, token_username = ?, token_password = ?,
                token_request_method = ?, token_content_type = ?,
                token_username_field = ?, token_password_field = ?,
                token_body_template = ?, token_extra_headers = ?,
                token_cookies = ?,
                token_field = ?, refresh_token_field = ?,
                token_header_field = ?, token_header_prefix = ?,
                token_expiry_seconds = ?, weight = ?, is_active = ?,
                bypass_proxy = ?,
                response_content_path = ?,
                response_reasoning_path = ?,
                chart_color = ?,
                updated_at = datetime('now')
                WHERE id = ?"#
        )
        .bind(new_id)
        .bind(name)
        .bind(base_url)
        .bind(api_type)
        .bind(auth_type)
        .bind(api_key)
        .bind(token_url)
        .bind(token_username)
        .bind(token_password)
        .bind(token_request_method)
        .bind(token_content_type)
        .bind(token_username_field)
        .bind(token_password_field)
        .bind(token_body_template)
        .bind(token_extra_headers)
        .bind(token_cookies)
        .bind(token_field)
        .bind(refresh_token_field)
        .bind(token_header_field)
        .bind(token_header_prefix)
        .bind(token_expiry_seconds)
        .bind(weight)
        .bind(is_active)
        .bind(bypass_proxy)
        .bind(response_content_path)
        .bind(response_reasoning_path)
        .bind(chart_color)
        .bind(old_id)
        .execute(&self.pool)
        .await?;


        Ok(())
    }

    /// Update provider chart color only
    pub async fn update_provider_chart_color(&self, id: &str, chart_color: Option<&str>) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE providers SET chart_color = ?, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(chart_color)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deactivate a provider
    pub async fn deactivate_provider(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE providers SET is_active = 0, updated_at = datetime('now') WHERE id = ?"
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Batch update provider active status (enable or disable)
    pub async fn batch_set_provider_active_status(&self, ids: &[String], is_active: bool) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        let ids_json = serde_json::to_string(ids).unwrap_or("[]".to_string());
        let result = sqlx::query(
            "UPDATE providers SET is_active = ?, updated_at = datetime('now') WHERE id IN (SELECT value FROM json_each(?))"
        )
        .bind(if is_active { 1 } else { 0 })
        .bind(&ids_json)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Delete a provider
    pub async fn delete_provider(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM providers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ========== Provider Model operations ==========

    /// Add a model to a provider
    pub async fn add_provider_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO provider_models (provider_id, model_id) VALUES (?, ?)"
        )
        .bind(provider_id)
        .bind(model_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Remove a model from a provider
    pub async fn remove_provider_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM provider_models WHERE provider_id = ? AND model_id = ?"
        )
        .bind(provider_id)
        .bind(model_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Clean up model_mappings when a provider model is removed.
    /// Deletes mappings where provider_id matches AND provider_model_id matches,
    /// then removes any unified models that no longer have any mappings.
    pub async fn cleanup_mappings_for_provider_model(
        &self,
        provider_id: &str,
        provider_model_id: &str,
    ) -> Result<(), sqlx::Error> {
        // Find all model_ids in model_mappings that have this provider+provider_model_id
        let affected_model_ids: Vec<String> = sqlx::query_scalar(
            r#"SELECT model_id FROM model_mappings WHERE provider_id = ? AND provider_model_id = ?"#
        )
        .bind(provider_id)
        .bind(provider_model_id)
        .fetch_all(&self.pool)
        .await?;

        // Delete the mappings
        sqlx::query(
            r#"DELETE FROM model_mappings WHERE provider_id = ? AND provider_model_id = ?"#
        )
        .bind(provider_id)
        .bind(provider_model_id)
        .execute(&self.pool)
        .await?;

        // Clean up unified models that no longer have any mappings
        for model_id in &affected_model_ids {
            let remaining: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM model_mappings WHERE model_id = ?"
            )
            .bind(model_id)
            .fetch_one(&self.pool)
            .await?;

            if remaining == 0 {
                sqlx::query("DELETE FROM models WHERE id = ?")
                    .bind(model_id)
                    .execute(&self.pool)
                    .await?;
            }
        }

        Ok(())
    }

    /// List all models for a provider
    pub async fn list_provider_models(
        &self,
        provider_id: &str,
    ) -> Result<Vec<ProviderModelRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ProviderModelRow>(
            "SELECT * FROM provider_models WHERE provider_id = ? ORDER BY model_id"
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// List active models for a provider
    pub async fn list_active_provider_models(
        &self,
        provider_id: &str,
    ) -> Result<Vec<ProviderModelRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ProviderModelRow>(
            "SELECT * FROM provider_models WHERE provider_id = ? AND is_active = 1 ORDER BY model_id"
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get a specific provider model
    pub async fn get_provider_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<Option<ProviderModelRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, ProviderModelRow>(
            "SELECT * FROM provider_models WHERE provider_id = ? AND model_id = ?"
        )
        .bind(provider_id)
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Update provider model test status
    pub async fn update_provider_model_test_status(
        &self,
        provider_id: &str,
        model_id: &str,
        status: &str,
        message: Option<&str>,
        is_active: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE provider_models SET
                last_test_status = ?,
                last_test_message = ?,
                is_active = ?,
                last_tested_at = datetime('now'),
                updated_at = datetime('now')
                WHERE provider_id = ? AND model_id = ?"#
        )
        .bind(status)
        .bind(message)
        .bind(is_active)
        .bind(provider_id)
        .bind(model_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Set all models for a provider (replace existing)
    pub async fn set_provider_models(
        &self,
        provider_id: &str,
        model_ids: &[String],
    ) -> Result<(), sqlx::Error> {
        // Delete existing models
        sqlx::query("DELETE FROM provider_models WHERE provider_id = ?")
            .bind(provider_id)
            .execute(&self.pool)
            .await?;

        // Insert new models
        for model_id in model_ids {
            sqlx::query(
                "INSERT INTO provider_models (provider_id, model_id) VALUES (?, ?)"
            )
            .bind(provider_id)
            .bind(model_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Find all active providers that have a specific model configured
    pub async fn find_providers_with_model(
        &self,
        model_id: &str,
    ) -> Result<Vec<ProviderRow>, sqlx::Error> {
        // Get all providers that have this model active
        let provider_ids: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT provider_id FROM provider_models WHERE model_id = ? AND is_active = 1"
        )
        .bind(model_id)
        .fetch_all(&self.pool)
        .await?;

        if provider_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Get active providers that have this model
        let mut providers = Vec::new();
        for pid in &provider_ids {
            if let Some(provider) = self.get_provider(pid).await? {
                if provider.is_active {
                    providers.push(provider);
                }
            }
        }

        Ok(providers)
    }

    /// Check if a model is allowed for a provider
    pub async fn is_model_allowed_for_provider(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<bool, sqlx::Error> {
        // If provider has no models configured, allow all
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM provider_models WHERE provider_id = ?"
        )
        .bind(provider_id)
        .fetch_one(&self.pool)
        .await?;

        if count == 0 {
            return Ok(true);
        }

        // Check if this specific model is active
        let active_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM provider_models WHERE provider_id = ? AND model_id = ? AND is_active = 1"
        )
        .bind(provider_id)
        .bind(model_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(active_count > 0)
    }

    // ========== Request log operations ==========

    /// Insert a request log entry
    pub async fn insert_request_log(
        &self,
        api_key_id: &str,
        provider_id: &str,
        model: Option<&str>,
        request_path: &str,
        request_method: &str,
        request_headers: Option<&str>,
        request_body: Option<&str>,
        response_status: Option<i32>,
        response_headers: Option<&str>,
        response_body: Option<&str>,
        prompt_tokens: i64,
        completion_tokens: i64,
        total_tokens: i64,
        duration_ms: Option<i64>,
        is_streaming: bool,
        is_throttled: bool,
        error_message: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"INSERT INTO request_logs (api_key_id, provider_id, model, request_path, request_method, request_headers, request_body, response_status, response_headers, response_body, prompt_tokens, completion_tokens, total_tokens, duration_ms, is_streaming, is_throttled, error_message)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(api_key_id)
        .bind(provider_id)
        .bind(model)
        .bind(request_path)
        .bind(request_method)
        .bind(request_headers)
        .bind(request_body)
        .bind(response_status)
        .bind(response_headers)
        .bind(response_body)
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .bind(total_tokens)
        .bind(duration_ms)
        .bind(is_streaming)
        .bind(is_throttled)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Query request logs with filters
    pub async fn query_request_logs(
        &self,
        api_key_id: Option<&str>,
        provider_id: Option<&str>,
        start_time: Option<&str>,
        end_time: Option<&str>,
        search: Option<&str>,
        model: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RequestLogRow>, sqlx::Error> {
        let mut query = String::from(
            "SELECT * FROM request_logs WHERE 1=1"
        );
        if api_key_id.is_some() { query.push_str(" AND api_key_id = ?"); }
        if provider_id.is_some() { query.push_str(" AND provider_id = ?"); }
        if start_time.is_some() { query.push_str(" AND created_at >= ?"); }
        if end_time.is_some() { query.push_str(" AND created_at <= ?"); }
        if search.is_some() { query.push_str(" AND (request_body LIKE ? OR response_body LIKE ?)"); }
        if model.is_some() { query.push_str(" AND model = ?"); }
        query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

        let mut q = sqlx::query_as::<_, RequestLogRow>(&query);
        if let Some(v) = api_key_id { q = q.bind(v); }
        if let Some(v) = provider_id { q = q.bind(v); }
        if let Some(v) = start_time { q = q.bind(v); }
        if let Some(v) = end_time { q = q.bind(v); }
        if let Some(ref v) = search {
            q = q.bind(format!("%{}%", v));
            q = q.bind(format!("%{}%", v));
        }
        if let Some(v) = model { q = q.bind(v); }
        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Count request logs with filters
    pub async fn count_request_logs(
        &self,
        api_key_id: Option<&str>,
        provider_id: Option<&str>,
        start_time: Option<&str>,
        end_time: Option<&str>,
        search: Option<&str>,
        model: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let mut query = String::from("SELECT COUNT(*) as count FROM request_logs WHERE 1=1");
        if api_key_id.is_some() { query.push_str(" AND api_key_id = ?"); }
        if provider_id.is_some() { query.push_str(" AND provider_id = ?"); }
        if start_time.is_some() { query.push_str(" AND created_at >= ?"); }
        if end_time.is_some() { query.push_str(" AND created_at <= ?"); }
        if search.is_some() { query.push_str(" AND (request_body LIKE ? OR response_body LIKE ?)"); }
        if model.is_some() { query.push_str(" AND model = ?"); }

        let mut q = sqlx::query_scalar::<_, i64>(&query);
        if let Some(v) = api_key_id { q = q.bind(v); }
        if let Some(v) = provider_id { q = q.bind(v); }
        if let Some(v) = start_time { q = q.bind(v); }
        if let Some(v) = end_time { q = q.bind(v); }
        if let Some(ref v) = search {
            q = q.bind(format!("%{}%", v));
            q = q.bind(format!("%{}%", v));
        }
        if let Some(v) = model { q = q.bind(v); }

        let count = q.fetch_one(&self.pool).await?;
        Ok(count)
    }

    /// Query request logs with filters — lightweight version without request_body/response_body
    pub async fn query_request_logs_lightweight(
        &self,
        api_key_id: Option<&str>,
        provider_id: Option<&str>,
        start_time: Option<&str>,
        end_time: Option<&str>,
        search: Option<&str>,
        model: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RequestLogListRow>, sqlx::Error> {
        let mut query = String::from(
            r#"SELECT id, api_key_id, provider_id, model, request_path, request_method,
                      response_status, prompt_tokens, completion_tokens, total_tokens,
                      duration_ms, is_streaming, is_throttled, error_message, created_at
               FROM request_logs WHERE 1=1"#
        );
        if api_key_id.is_some() { query.push_str(" AND api_key_id = ?"); }
        if provider_id.is_some() { query.push_str(" AND provider_id = ?"); }
        if start_time.is_some() { query.push_str(" AND created_at >= ?"); }
        if end_time.is_some() { query.push_str(" AND created_at <= ?"); }
        if search.is_some() { query.push_str(" AND (request_body LIKE ? OR response_body LIKE ?)"); }
        if model.is_some() { query.push_str(" AND model = ?"); }
        query.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

        let mut q = sqlx::query_as::<_, RequestLogListRow>(&query);
        if let Some(v) = api_key_id { q = q.bind(v); }
        if let Some(v) = provider_id { q = q.bind(v); }
        if let Some(v) = start_time { q = q.bind(v); }
        if let Some(v) = end_time { q = q.bind(v); }
        if let Some(ref v) = search {
            q = q.bind(format!("%{}%", v));
            q = q.bind(format!("%{}%", v));
        }
        if let Some(v) = model { q = q.bind(v); }
        q = q.bind(limit).bind(offset);

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Get a single request log by ID
    pub async fn get_request_log(&self, id: i64) -> Result<Option<RequestLogRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, RequestLogRow>(
            "SELECT * FROM request_logs WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Delete a single request log by ID
    pub async fn delete_request_log(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM request_logs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete request logs by provider ID
    pub async fn delete_request_logs_by_provider(&self, provider_id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM request_logs WHERE provider_id = ?")
            .bind(provider_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete request logs by API key ID
    pub async fn delete_request_logs_by_api_key(&self, api_key_id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM request_logs WHERE api_key_id = ?")
            .bind(api_key_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete all request logs
    pub async fn delete_all_request_logs(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM request_logs")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    // ========== Statistics operations ==========

    /// Get aggregate statistics
    pub async fn get_stats(
        &self,
        api_key_id: Option<&str>,
        provider_id: Option<&str>,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<AggregateStats, sqlx::Error> {
        let mut query = String::from(
            r#"SELECT
                COUNT(*) as total_requests,
                COALESCE(SUM(prompt_tokens), 0) as total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0) as total_completion_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COALESCE(AVG(duration_ms), 0.0) as avg_duration_ms,
                SUM(CASE WHEN is_throttled = 1 THEN 1 ELSE 0 END) as throttle_count,
                SUM(CASE WHEN error_message IS NOT NULL THEN 1 ELSE 0 END) as error_count
            FROM request_logs WHERE 1=1"#
        );
        if api_key_id.is_some() { query.push_str(" AND api_key_id = ?"); }
        if provider_id.is_some() { query.push_str(" AND provider_id = ?"); }
        if start_time.is_some() { query.push_str(" AND created_at >= ?"); }
        if end_time.is_some() { query.push_str(" AND created_at <= ?"); }

        let mut q = sqlx::query_as::<_, AggregateStats>(&query);
        if let Some(v) = api_key_id { q = q.bind(v); }
        if let Some(v) = provider_id { q = q.bind(v); }
        if let Some(v) = start_time { q = q.bind(v); }
        if let Some(v) = end_time { q = q.bind(v); }

        let stats = q.fetch_one(&self.pool).await?;
        Ok(stats)
    }

    /// Get time-bucketed statistics
    /// For "5h" granularity: groups into 5-hour buckets starting from midnight each day
    /// (0:00-5:00, 5:00-10:00, 10:00-15:00, 15:00-20:00, 20:00-24:00)
    pub async fn get_time_bucketed_stats(
        &self,
        api_key_id: Option<&str>,
        provider_id: Option<&str>,
        start_time: &str,
        end_time: &str,
        granularity: &str, // "5h", "day", "week", "month"
    ) -> Result<Vec<TimeBucketStats>, sqlx::Error> {
        if granularity == "5h" {
            // Special handling: query by hour, then merge into 5-hour buckets
            return self.get_5h_bucketed_stats(api_key_id, provider_id, start_time, end_time).await;
        }

        let strftime_format = match granularity {
            "day" => "%Y-%m-%d",
            "week" => "%Y-W%W",
            "month" => "%Y-%m",
            _ => "%Y-%m-%d",
        };

        let mut query = format!(
            r#"SELECT
                strftime('{}', created_at) as period,
                COUNT(*) as request_count,
                COALESCE(SUM(prompt_tokens), 0) as prompt_tokens,
                COALESCE(SUM(completion_tokens), 0) as completion_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                SUM(CASE WHEN is_throttled = 1 THEN 1 ELSE 0 END) as throttle_count,
                SUM(CASE WHEN error_message IS NOT NULL THEN 1 ELSE 0 END) as error_count,
                COALESCE(AVG(duration_ms), 0.0) as avg_duration_ms
            FROM request_logs WHERE created_at >= ? AND created_at <= ?"#
        , strftime_format);

        if api_key_id.is_some() { query.push_str(" AND api_key_id = ?"); }
        if provider_id.is_some() { query.push_str(" AND provider_id = ?"); }
        query.push_str(" GROUP BY period ORDER BY period");

        let mut q = sqlx::query_as::<_, TimeBucketStats>(&query);
        q = q.bind(start_time).bind(end_time);
        if let Some(v) = api_key_id { q = q.bind(v); }
        if let Some(v) = provider_id { q = q.bind(v); }

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Get 5-hour bucketed statistics
    /// Each day is split into 5-hour buckets starting from midnight:
    /// 0:00-5:00, 5:00-10:00, 10:00-15:00, 15:00-20:00, 20:00-24:00
    /// The last bucket is only 4 hours (20-24) since a day has 24 hours.
    async fn get_5h_bucketed_stats(
        &self,
        api_key_id: Option<&str>,
        provider_id: Option<&str>,
        start_time: &str,
        end_time: &str,
    ) -> Result<Vec<TimeBucketStats>, sqlx::Error> {
        // First, query hourly stats
        let mut query = String::from(
            r#"SELECT
                strftime('%Y-%m-%d %H', created_at) as period,
                COUNT(*) as request_count,
                COALESCE(SUM(prompt_tokens), 0) as prompt_tokens,
                COALESCE(SUM(completion_tokens), 0) as completion_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                SUM(CASE WHEN is_throttled = 1 THEN 1 ELSE 0 END) as throttle_count,
                SUM(CASE WHEN error_message IS NOT NULL THEN 1 ELSE 0 END) as error_count,
                COALESCE(AVG(duration_ms), 0.0) as avg_duration_ms
            FROM request_logs WHERE created_at >= ? AND created_at <= ?"#
        );
        if api_key_id.is_some() { query.push_str(" AND api_key_id = ?"); }
        if provider_id.is_some() { query.push_str(" AND provider_id = ?"); }
        query.push_str(" GROUP BY period ORDER BY period");

        let mut q = sqlx::query_as::<_, TimeBucketStats>(&query);
        q = q.bind(start_time).bind(end_time);
        if let Some(v) = api_key_id { q = q.bind(v); }
        if let Some(v) = provider_id { q = q.bind(v); }

        let hourly_rows = q.fetch_all(&self.pool).await?;

        // Merge hourly rows into 5-hour buckets
        // Bucket index: hour / 5 => 0(0-4), 1(5-9), 2(10-14), 3(15-19), 4(20-23)
        let mut buckets: std::collections::BTreeMap<String, TimeBucketStats> = std::collections::BTreeMap::new();

        for row in &hourly_rows {
            // Parse period like "2026-05-15 15"
            let parts: Vec<&str> = row.period.split(' ').collect();
            if parts.len() != 2 { continue; }
            let date = parts[0];
            let hour: i64 = parts[1].parse().unwrap_or(0);

            // Determine bucket index (0-4)
            let bucket_idx = hour / 5;
            // Bucket start and end hours
            let bucket_start = bucket_idx * 5;
            let bucket_end = if bucket_idx == 4 { 24 } else { bucket_start + 5 }; // Last bucket is 20-24

            // Generate period label like "2026-05-15 00:00~05:00"
            let period_label = format!("{} {:02}:00~{:02}:00", date, bucket_start, bucket_end);

            // Merge into bucket
            if let Some(existing) = buckets.get_mut(&period_label) {
                existing.request_count += row.request_count;
                existing.prompt_tokens += row.prompt_tokens;
                existing.completion_tokens += row.completion_tokens;
                existing.total_tokens += row.total_tokens;
                existing.throttle_count += row.throttle_count;
                existing.error_count += row.error_count;
                // Weighted average for duration_ms
                let total_requests = existing.request_count;
                if total_requests > 0 {
                    existing.avg_duration_ms = (existing.avg_duration_ms * (total_requests - row.request_count) as f64 + row.avg_duration_ms * row.request_count as f64) / total_requests as f64;
                }
            } else {
                let label = period_label.clone();
                buckets.insert(period_label, TimeBucketStats {
                    period: label,
                    request_count: row.request_count,
                    prompt_tokens: row.prompt_tokens,
                    completion_tokens: row.completion_tokens,
                    total_tokens: row.total_tokens,
                    throttle_count: row.throttle_count,
                    error_count: row.error_count,
                    avg_duration_ms: row.avg_duration_ms,
                });
            }
        }

        Ok(buckets.into_values().collect())
    }

    // ========== Token rate operations ==========

    /// Insert a token rate snapshot
    pub async fn insert_token_rate_snapshot(
        &self,
        provider_id: Option<&str>,
        tokens_per_second: f64,
        prompt_tokens: i64,
        completion_tokens: i64,
        request_count: i64,
        elapsed_seconds: f64,
    ) -> Result<(), sqlx::Error> {
        // Use UTC ISO 8601 format for consistent timezone handling
        let snapshot_time = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        sqlx::query(
            "INSERT INTO token_rate_snapshots (provider_id, tokens_per_second, prompt_tokens, completion_tokens, request_count, elapsed_seconds, snapshot_time) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(provider_id)
        .bind(tokens_per_second)
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .bind(request_count)
        .bind(elapsed_seconds)
        .bind(&snapshot_time)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get token rate snapshots for a time range
    pub async fn get_token_rate_snapshots(
        &self,
        provider_id: Option<&str>,
        start_time: &str,
        limit: i64,
    ) -> Result<Vec<TokenRateRow>, sqlx::Error> {
        let mut query = String::from(
            "SELECT * FROM token_rate_snapshots WHERE snapshot_time >= ? AND provider_id IS NOT NULL"
        );
        if provider_id.is_some() { query.push_str(" AND provider_id = ?"); }
        query.push_str(" ORDER BY snapshot_time ASC LIMIT ?");

        let mut q = sqlx::query_as::<_, TokenRateRow>(&query);
        q = q.bind(start_time);
        if let Some(v) = provider_id { q = q.bind(v); }
        q = q.bind(limit);

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Clean up old token rate snapshots (older than 1 hour)
    pub async fn cleanup_old_snapshots(&self, before_time: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM token_rate_snapshots WHERE snapshot_time < ?"
        )
        .bind(before_time)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get the provider with highest token usage in the last hour
    pub async fn get_top_provider_by_usage(&self) -> Result<Option<String>, sqlx::Error> {
        let start = (chrono::Utc::now() - chrono::Duration::hours(1)).format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let result: Option<(String, i64)> = sqlx::query_as(
            r#"SELECT provider_id, SUM(prompt_tokens + completion_tokens) as total_tokens FROM token_rate_snapshots WHERE snapshot_time >= ? AND provider_id IS NOT NULL GROUP BY provider_id ORDER BY total_tokens DESC LIMIT 1"#
        ).bind(&start).fetch_optional(&self.pool).await?;
        Ok(result.map(|(id, _)| id))
    }

    /// Get dashboard summary
    pub async fn get_dashboard_summary(&self) -> Result<DashboardSummaryData, sqlx::Error> {
        let total_api_keys: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_keys")
            .fetch_one(&self.pool)
            .await?;
        let active_api_keys: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM api_keys WHERE is_active = 1")
            .fetch_one(&self.pool)
            .await?;
        let total_providers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM providers")
            .fetch_one(&self.pool)
            .await?;
        let active_providers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM providers WHERE is_active = 1")
            .fetch_one(&self.pool)
            .await?;

        let now = chrono::Utc::now();
        let yesterday = now - chrono::Duration::hours(24);
        let yesterday_str = yesterday.format("%Y-%m-%d %H:%M:%S").to_string();

        let stats_24h = self.get_stats(None, None, Some(&yesterday_str), None).await?;

        // Get latest token rate
        let latest_rate: f64 = sqlx::query_scalar(
            "SELECT COALESCE(AVG(tokens_per_second), 0.0) FROM token_rate_snapshots WHERE snapshot_time >= ?"
        )
        .bind(&yesterday_str)
        .fetch_one(&self.pool)
        .await?;

        Ok(DashboardSummaryData {
            total_api_keys,
            active_api_keys,
            total_providers,
            active_providers,
            total_requests_24h: stats_24h.total_requests,
            total_tokens_24h: stats_24h.total_tokens,
            avg_tokens_per_second: latest_rate,
            throttle_count_24h: stats_24h.throttle_count,
        })
    }

    // ========== Unified Model operations ==========

    /// Create a new unified model
    pub async fn create_model(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        model_type: &str,
        priority: i64,
        config: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO models (id, name, description, model_type, priority, config) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(model_type)
        .bind(priority)
        .bind(config)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get aggregated stats grouped by API key (top N by total tokens)
    pub async fn get_stats_by_api_key(
        &self,
        limit: i64,
        start_time: Option<&str>,
        end_time: Option<&str>,
    ) -> Result<Vec<ApiKeyStatsRow>, sqlx::Error> {
        let mut query = String::from(
            r#"SELECT api_key_id, COUNT(*) as request_count,
            COALESCE(SUM(prompt_tokens), 0) as total_prompt_tokens,
            COALESCE(SUM(completion_tokens), 0) as total_completion_tokens,
            COALESCE(SUM(total_tokens), 0) as total_tokens,
            COALESCE(AVG(duration_ms), 0.0) as avg_duration_ms,
            SUM(CASE WHEN is_throttled = 1 THEN 1 ELSE 0 END) as throttle_count,
            SUM(CASE WHEN error_message IS NOT NULL THEN 1 ELSE 0 END) as error_count
            FROM request_logs WHERE 1=1"#
        );
        if start_time.is_some() { query.push_str(" AND created_at >= ?"); }
        if end_time.is_some() { query.push_str(" AND created_at <= ?"); }
        query.push_str(" GROUP BY api_key_id ORDER BY total_tokens DESC LIMIT ?");

        let mut q = sqlx::query_as::<_, ApiKeyStatsRow>(&query);
        if let Some(v) = start_time { q = q.bind(v); }
        if let Some(v) = end_time { q = q.bind(v); }
        q = q.bind(limit);

        q.fetch_all(&self.pool).await
    }

    /// Get health stats for all providers (last 24h)
    pub async fn get_provider_health_stats(&self) -> Result<Vec<ProviderHealthStats>, sqlx::Error> {
        let one_day_ago = (chrono::Utc::now() - chrono::Duration::days(1)).format("%Y-%m-%d %H:%M:%S").to_string();
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let rows = sqlx::query_as::<_, (String, i64, i64, i64, f64, Option<String>)>(
            r#"
            SELECT
                provider_id,
                COUNT(*) as request_count,
                SUM(CASE WHEN error_message IS NOT NULL THEN 1 ELSE 0 END) as error_count,
                SUM(CASE WHEN is_throttled = 1 THEN 1 ELSE 0 END) as throttle_count,
                COALESCE(AVG(duration_ms), 0.0) as avg_duration_ms,
                MAX(created_at) as last_request_at
            FROM request_logs
            WHERE created_at >= ? AND created_at <= ?
            GROUP BY provider_id
            "#
        )
        .bind(&one_day_ago)
        .bind(&now)
        .fetch_all(&self.pool)
        .await?;

        let stats = rows.into_iter().map(|(provider_id, request_count, error_count, throttle_count, avg_duration_ms, last_request_at)| {
            ProviderHealthStats {
                provider_id,
                request_count_24h: request_count,
                error_count_24h: error_count,
                throttle_count_24h: throttle_count,
                avg_duration_ms,
                last_request_at,
            }
        }).collect();

        Ok(stats)
    }

    /// Get model by ID
    pub async fn get_model(&self, id: &str) -> Result<Option<ModelRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, ModelRow>(
            "SELECT * FROM models WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all models
    pub async fn list_models(&self) -> Result<Vec<ModelRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ModelRow>(
            "SELECT * FROM models ORDER BY priority DESC, name"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List active models
    pub async fn list_active_models(&self) -> Result<Vec<ModelRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ModelRow>(
            "SELECT * FROM models WHERE is_active = 1 ORDER BY priority DESC, name"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update model
    pub async fn update_model(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        model_type: &str,
        is_active: bool,
        priority: i64,
        config: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE models SET 
                name = ?, description = ?, model_type = ?, is_active = ?, 
                priority = ?, config = ?, updated_at = datetime('now')
                WHERE id = ?"#
        )
        .bind(name)
        .bind(description)
        .bind(model_type)
        .bind(is_active)
        .bind(priority)
        .bind(config)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete model
    pub async fn delete_model(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Add a provider mapping to a model
    pub async fn add_model_mapping(
        &self,
        model_id: &str,
        provider_id: &str,
        provider_model_id: &str,
        weight: i64,
        cost_multiplier: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO model_mappings (model_id, provider_id, provider_model_id, weight, cost_multiplier) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(model_id)
        .bind(provider_id)
        .bind(provider_model_id)
        .bind(weight)
        .bind(cost_multiplier)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a provider mapping from a model
    pub async fn remove_model_mapping(
        &self,
        model_id: &str,
        provider_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM model_mappings WHERE model_id = ? AND provider_id = ?"
        )
        .bind(model_id)
        .bind(provider_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all mappings for a model
    pub async fn list_model_mappings(
        &self,
        model_id: &str,
    ) -> Result<Vec<ModelMappingRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ModelMappingRow>(
            "SELECT * FROM model_mappings WHERE model_id = ? ORDER BY weight DESC"
        )
        .bind(model_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List active mappings for a model
    pub async fn list_active_model_mappings(
        &self,
        model_id: &str,
    ) -> Result<Vec<ModelMappingRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ModelMappingRow>(
            "SELECT * FROM model_mappings WHERE model_id = ? AND is_active = 1 ORDER BY weight DESC"
        )
        .bind(model_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update model mapping
    pub async fn update_model_mapping(
        &self,
        model_id: &str,
        provider_id: &str,
        provider_model_id: &str,
        is_active: bool,
        weight: i64,
        cost_multiplier: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE model_mappings SET 
                provider_model_id = ?, is_active = ?, weight = ?, cost_multiplier = ?
                WHERE model_id = ? AND provider_id = ?"#
        )
        .bind(provider_model_id)
        .bind(is_active)
        .bind(weight)
        .bind(cost_multiplier)
        .bind(model_id)
        .bind(provider_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get model with all provider mappings (full model info)
    pub async fn get_model_with_mappings(
        &self,
        id: &str,
    ) -> Result<Option<ModelWithMappings>, sqlx::Error> {
        let model = self.get_model(id).await?;
        if model.is_none() {
            return Ok(None);
        }
        let model = model.unwrap();

        let mappings = self.list_model_mappings(id).await?;
        let mut mappings_with_provider = Vec::new();

        for mapping in mappings {
            if let Some(provider) = self.get_provider(&mapping.provider_id).await? {
                mappings_with_provider.push(ModelMappingWithProvider {
                    mapping,
                    provider,
                });
            }
        }

        Ok(Some(ModelWithMappings {
            model,
            mappings: mappings_with_provider,
        }))
    }

    /// List all models with their mappings
    pub async fn list_models_with_mappings(&self) -> Result<Vec<ModelWithMappings>, sqlx::Error> {
        let models = self.list_models().await?;
        let mut result = Vec::new();

        for model in models {
            let mappings = self.list_model_mappings(&model.id).await?;
            let mut mappings_with_provider = Vec::new();

            for mapping in mappings {
                if let Some(provider) = self.get_provider(&mapping.provider_id).await? {
                    mappings_with_provider.push(ModelMappingWithProvider {
                        mapping,
                        provider,
                    });
                }
            }

            result.push(ModelWithMappings {
                model,
                mappings: mappings_with_provider,
            });
        }

        Ok(result)
    }

    /// Get model by the external model ID used in requests
    pub async fn get_model_by_external_id(&self, external_id: &str) -> Result<Option<ModelRow>, sqlx::Error> {
        // First check if it's a unified model ID
        if let Some(model) = self.get_model(external_id).await? {
            return Ok(Some(model));
        }
        // If not found, return None (could be extended to search provider_model_id)
        Ok(None)
    }

    // ========== Provider Quota operations ==========

    /// Set (upsert) a quota limit for a provider
    pub async fn set_provider_quota(
        &self,
        provider_id: &str,
        quota_type: &str,
        window_mode: &str,
        window_size: &str,
        window_start_override: Option<&str>,
        limit_count: i64,
        is_enabled: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO provider_quotas (provider_id, quota_type, window_mode, window_size, window_start_override, limit_count, is_enabled)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(provider_id, quota_type) DO UPDATE SET
                window_mode = excluded.window_mode,
                window_size = excluded.window_size,
                window_start_override = excluded.window_start_override,
                limit_count = excluded.limit_count,
                is_enabled = excluded.is_enabled,
                updated_at = datetime('now')"#
        )
        .bind(provider_id)
        .bind(quota_type)
        .bind(window_mode)
        .bind(window_size)
        .bind(window_start_override)
        .bind(limit_count)
        .bind(is_enabled)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get all quotas for a provider
    pub async fn list_provider_quotas(
        &self,
        provider_id: &str,
    ) -> Result<Vec<ProviderQuotaRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ProviderQuotaRow>(
            "SELECT * FROM provider_quotas WHERE provider_id = ? ORDER BY quota_type"
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get all quotas across all providers
    pub async fn list_all_quotas(&self) -> Result<Vec<ProviderQuotaRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ProviderQuotaRow>(
            "SELECT * FROM provider_quotas ORDER BY provider_id, quota_type"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Delete a quota for a provider
    pub async fn delete_provider_quota(
        &self,
        provider_id: &str,
        quota_type: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM provider_quotas WHERE provider_id = ? AND quota_type = ?"
        )
        .bind(provider_id)
        .bind(quota_type)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Set (upsert) calibration offset for a provider quota
    /// calibration_offset represents the number of requests made outside the gateway
    /// that should be added to the gateway count for accurate total usage.
    pub async fn set_quota_calibration(
        &self,
        provider_id: &str,
        quota_type: &str,
        calibration_offset: i64,
        calibration_window_start: Option<&str>,
        calibration_window_end: Option<&str>,
        note: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO provider_quota_calibrations (provider_id, quota_type, calibration_offset, calibration_window_start, calibration_window_end, note)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(provider_id, quota_type) DO UPDATE SET
                calibration_offset = excluded.calibration_offset,
                calibrated_at = datetime('now'),
                calibration_window_start = excluded.calibration_window_start,
                calibration_window_end = excluded.calibration_window_end,
                note = excluded.note"#
        )
        .bind(provider_id)
        .bind(quota_type)
        .bind(calibration_offset)
        .bind(calibration_window_start)
        .bind(calibration_window_end)
        .bind(note)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get calibration for a specific provider quota
    pub async fn get_quota_calibration(
        &self,
        provider_id: &str,
        quota_type: &str,
    ) -> Result<Option<ProviderQuotaCalibrationRow>, sqlx::Error> {
        let row = sqlx::query_as::<_, ProviderQuotaCalibrationRow>(
            "SELECT * FROM provider_quota_calibrations WHERE provider_id = ? AND quota_type = ?"
        )
        .bind(provider_id)
        .bind(quota_type)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get all calibrations for a provider
    pub async fn list_provider_calibrations(
        &self,
        provider_id: &str,
    ) -> Result<Vec<ProviderQuotaCalibrationRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ProviderQuotaCalibrationRow>(
            "SELECT * FROM provider_quota_calibrations WHERE provider_id = ? ORDER BY quota_type"
        )
        .bind(provider_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Count gateway requests for a provider in a given time range
    pub async fn count_provider_requests(
        &self,
        provider_id: &str,
        start_time: &str,
        end_time: &str,
    ) -> Result<i64, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM request_logs WHERE provider_id = ? AND created_at >= ? AND created_at <= ?"
        )
        .bind(provider_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Compute quota usage info for a single quota
    fn compute_quota_usage(
        &self,
        quota: &ProviderQuotaRow,
        provider_name: &str,
        now: &chrono::DateTime<chrono::Utc>,
        gateway_count: i64,
        calibration: Option<ProviderQuotaCalibrationRow>,
    ) -> QuotaUsageInfo {
        let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();

        if !quota.is_enabled {
            return QuotaUsageInfo {
                provider_id: quota.provider_id.clone(),
                provider_name: provider_name.to_string(),
                quota_type: quota.quota_type.clone(),
                window_mode: quota.window_mode.clone(),
                window_size: quota.window_size.clone(),
                window_start_override: quota.window_start_override.clone(),
                limit_count: quota.limit_count,
                is_enabled: false,
                gateway_count: 0,
                calibration_offset: 0,
                calibration_valid: false,
                calibration_window_start: None,
                calibration_window_end: None,
                estimated_total: 0,
                remaining: quota.limit_count,
                usage_percent: 0.0,
                period_start: String::new(),
                period_current: now_str,
            };
        }

        // Determine window_mode and window_size, with backward compat for legacy quota_type
        let (window_mode, window_size) = if quota.window_mode.is_empty() || quota.window_mode == "sliding" && quota.window_size == "5h" && quota.quota_type == "5h" {
            // Legacy format
            Self::normalize_quota_type(&quota.quota_type)
        } else {
            (quota.window_mode.clone(), quota.window_size.clone())
        };

        // Calculate period: if window_start_override is set for fixed window, use it
        let (period_start, period_end) = if window_mode == "fixed" && quota.window_start_override.is_some() {
            // User-specified start time override
            let override_str = quota.window_start_override.as_ref().unwrap();
            if let Ok(custom_start) = chrono::DateTime::parse_from_rfc3339(
                &format!("{}Z", override_str.replace(' ', "T"))
            ) {
                let custom_start_utc = custom_start.to_utc();
                let duration = Self::parse_window_size(&window_size);
                let custom_end = custom_start_utc + duration;
                (custom_start_utc, custom_end)
            } else {
                // Fallback to auto-alignment if override is invalid
                Self::get_quota_period(&window_mode, &window_size, now)
            }
        } else {
            Self::get_quota_period(&window_mode, &window_size, now)
        };
        let period_start_str = period_start.format("%Y-%m-%d %H:%M:%S").to_string();

        // Check calibration validity
        let (calibration_offset, calibration_valid, calibration_window_start, calibration_window_end) =
            if let Some(ref cal) = calibration {
                let valid = Self::is_calibration_valid(cal, &window_mode, &period_start, &period_end);
                let offset = if valid { cal.calibration_offset } else { 0 };
                (offset, valid, cal.calibration_window_start.clone(), cal.calibration_window_end.clone())
            } else {
                (0, false, None, None)
            };

        let estimated_total = gateway_count + calibration_offset;
        let remaining = (quota.limit_count - estimated_total).max(0);
        let usage_percent = if quota.limit_count > 0 {
            (estimated_total as f64 / quota.limit_count as f64) * 100.0
        } else {
            0.0
        };

        QuotaUsageInfo {
            provider_id: quota.provider_id.clone(),
            provider_name: provider_name.to_string(),
            quota_type: quota.quota_type.clone(),
            window_mode,
            window_size,
            window_start_override: quota.window_start_override.clone(),
            limit_count: quota.limit_count,
            is_enabled: quota.is_enabled,
            gateway_count,
            calibration_offset,
            calibration_valid,
            calibration_window_start,
            calibration_window_end,
            estimated_total,
            remaining,
            usage_percent: usage_percent.min(100.0),
            period_start: period_start_str,
            period_current: now_str,
        }
    }

    /// Get computed quota usage for all providers
    pub async fn get_all_quota_usage(&self) -> Result<Vec<QuotaUsageInfo>, sqlx::Error> {
        let quotas = self.list_all_quotas().await?;
        let mut result = Vec::new();

        let now = chrono::Utc::now();

        for quota in &quotas {
            let provider = self.get_provider(&quota.provider_id).await.ok().flatten();
            let provider_name = provider.map(|p| p.name).unwrap_or_default();

            if !quota.is_enabled {
                result.push(self.compute_quota_usage(quota, &provider_name, &now, 0, None));
                continue;
            }

            let (window_mode, window_size) = if quota.window_mode.is_empty() {
                Self::normalize_quota_type(&quota.quota_type)
            } else {
                (quota.window_mode.clone(), quota.window_size.clone())
            };
            let (period_start, period_end) = Self::get_quota_period(&window_mode, &window_size, &now);
            let period_start_str = period_start.format("%Y-%m-%d %H:%M:%S").to_string();
            let period_end_str = period_end.format("%Y-%m-%d %H:%M:%S").to_string();

            let gateway_count = self.count_provider_requests(
                &quota.provider_id,
                &period_start_str,
                &period_end_str,
            ).await.unwrap_or(0);

            let calibration = self.get_quota_calibration(&quota.provider_id, &quota.quota_type)
                .await.ok().flatten();

            result.push(self.compute_quota_usage(quota, &provider_name, &now, gateway_count, calibration));
        }

        Ok(result)
    }

    /// Get computed quota usage for a specific provider
    pub async fn get_provider_quota_usage(
        &self,
        provider_id: &str,
    ) -> Result<Vec<QuotaUsageInfo>, sqlx::Error> {
        let quotas = self.list_provider_quotas(provider_id).await?;
        let mut result = Vec::new();

        let now = chrono::Utc::now();

        let provider = self.get_provider(provider_id).await.ok().flatten();
        let provider_name = provider.map(|p| p.name).unwrap_or_default();

        for quota in &quotas {
            if !quota.is_enabled {
                result.push(self.compute_quota_usage(quota, &provider_name, &now, 0, None));
                continue;
            }

            let (window_mode, window_size) = if quota.window_mode.is_empty() {
                Self::normalize_quota_type(&quota.quota_type)
            } else {
                (quota.window_mode.clone(), quota.window_size.clone())
            };
            let (period_start, period_end) = Self::get_quota_period(&window_mode, &window_size, &now);
            let period_start_str = period_start.format("%Y-%m-%d %H:%M:%S").to_string();
            let period_end_str = period_end.format("%Y-%m-%d %H:%M:%S").to_string();

            let gateway_count = self.count_provider_requests(
                &quota.provider_id,
                &period_start_str,
                &period_end_str,
            ).await.unwrap_or(0);

            let calibration = self.get_quota_calibration(&quota.provider_id, &quota.quota_type)
                .await.ok().flatten();

            result.push(self.compute_quota_usage(quota, &provider_name, &now, gateway_count, calibration));
        }

        Ok(result)
    }

    /// Calculate the period start and end times for a quota type
    /// quota_type: "5h", "weekly", "monthly"
    /// Returns (period_start, period_end)
    /// Calculate the period start and end times for a quota
    /// Supports both fixed and sliding window modes
    pub fn get_quota_period(window_mode: &str, window_size: &str, now: &chrono::DateTime<chrono::Utc>) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
        use chrono::Datelike;

        let duration = Self::parse_window_size(window_size);

        match window_mode {
            "fixed" => {
                Self::get_fixed_window_period(window_size, duration, now)
            }
            "sliding" => {
                // Sliding window: from (now - duration) to now
                let start = *now - duration;
                (start, *now)
            }
            _ => {
                // Default: sliding 24h
                let start = *now - chrono::Duration::hours(24);
                (start, *now)
            }
        }
    }

    /// Parse window_size string like "5h", "7d", "30d" into a Duration
    fn parse_window_size(window_size: &str) -> chrono::Duration {
        let window_size = window_size.trim();
        if let Some(hours) = window_size.strip_suffix('h') {
            if let Ok(h) = hours.parse::<i64>() {
                return chrono::Duration::hours(h);
            }
        }
        if let Some(days) = window_size.strip_suffix('d') {
            if let Ok(d) = days.parse::<i64>() {
                return chrono::Duration::days(d);
            }
        }
        // Default: 5 hours
        chrono::Duration::hours(5)
    }

    /// Calculate fixed window period aligned to natural boundaries
    fn get_fixed_window_period(window_size: &str, duration: chrono::Duration, now: &chrono::DateTime<chrono::Utc>) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
        use chrono::Datelike;

        if window_size.ends_with('h') {
            // Hour-based fixed window: align to 0:00 of the day, then step by N-hour intervals
            // e.g. 5h → windows at 0:00, 5:00, 10:00, 15:00, 20:00
            if let Some(hours_str) = window_size.strip_suffix('h') {
                if let Ok(window_hours) = hours_str.parse::<i64>() {
                    if window_hours > 0 {
                        let day_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
                        let day_start_utc = day_start.and_utc();
                        let hours_since_day_start = (*now - day_start_utc).num_hours();
                        let window_index = hours_since_day_start / window_hours;
                        let period_start = day_start_utc + chrono::Duration::hours(window_index * window_hours);
                        let period_end = period_start + chrono::Duration::hours(window_hours);
                        return (period_start, period_end);
                    }
                }
            }
        }

        if window_size.ends_with('d') {
            if let Some(days_str) = window_size.strip_suffix('d') {
                if let Ok(window_days) = days_str.parse::<i64>() {
                    if window_days == 1 {
                        // 1d = today 0:00 to tomorrow 0:00
                        let day_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
                        let start = day_start.and_utc();
                        let end = start + chrono::Duration::days(1);
                        return (start, end);
                    } else if window_days == 7 {
                        // 7d = this week Monday 0:00 to next Monday 0:00
                        let weekday = now.weekday().num_days_from_monday();
                        let week_start_naive = now.date_naive()
                            .checked_sub_signed(chrono::Duration::days(weekday as i64))
                            .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                            .unwrap_or_else(|| now.date_naive().and_hms_opt(0, 0, 0).unwrap());
                        let start = week_start_naive.and_utc();
                        let end = start + chrono::Duration::weeks(1);
                        return (start, end);
                    } else if window_days == 30 || window_days == 31 {
                        // 30d/31d = this month 1st 0:00 to next month 1st 0:00
                        let month_start_date = chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), 1)
                            .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
                        let month_start_naive = month_start_date.and_hms_opt(0, 0, 0).unwrap();
                        let start = month_start_naive.and_utc();
                        let next_month = if now.month() == 12 {
                            chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1)
                        } else {
                            chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1)
                        }.unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
                        let end_naive = next_month.and_hms_opt(0, 0, 0).unwrap();
                        let end = end_naive.and_utc();
                        return (start, end);
                    } else {
                        // Generic N-day fixed window: align to epoch-like start
                        // Simplification: align to most recent N-day boundary from a reference point
                        // Use days since 2000-01-01 as reference
                        let ref_date = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
                        let ref_date_utc = ref_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                        let days_since_ref = (*now - ref_date_utc).num_days();
                        let window_index = days_since_ref / window_days;
                        let period_start_date = ref_date.checked_add_signed(chrono::Duration::days(window_index * window_days)).unwrap();
                        let period_start_naive = period_start_date.and_hms_opt(0, 0, 0).unwrap();
                        let period_end_date = period_start_date.checked_add_signed(chrono::Duration::days(window_days)).unwrap();
                        let period_end_naive = period_end_date.and_hms_opt(0, 0, 0).unwrap();
                        let start = period_start_naive.and_utc();
                        let end = period_end_naive.and_utc();
                        return (start, end);
                    }
                }
            }
        }

        // Default: fixed 5h window
        Self::get_fixed_window_period("5h", chrono::Duration::hours(5), now)
    }

    /// Check if a calibration is still valid in the current window period
    pub fn is_calibration_valid(
        cal: &ProviderQuotaCalibrationRow,
        window_mode: &str,
        current_period_start: &chrono::DateTime<chrono::Utc>,
        current_period_end: &chrono::DateTime<chrono::Utc>,
    ) -> bool {
        match window_mode {
            "fixed" => {
                // Fixed window: calibration is valid only if its window boundaries
                // exactly match the current window boundaries
                if let (Some(cal_start), Some(cal_end)) = (&cal.calibration_window_start, &cal.calibration_window_end) {
                    let current_start_str = current_period_start.format("%Y-%m-%d %H:%M:%S").to_string();
                    let current_end_str = current_period_end.format("%Y-%m-%d %H:%M:%S").to_string();
                    // Compare as strings (UTC format)
                    cal_start == &current_start_str && cal_end == &current_end_str
                } else {
                    // No window info recorded → invalid (old calibration without window binding)
                    false
                }
            }
            "sliding" => {
                // Sliding window: calibration is valid if calibrated_at is within
                // the current sliding window period
                if let Ok(calibrated_at) = chrono::DateTime::parse_from_rfc3339(
                    &format!("{}Z", cal.calibrated_at.replace(' ', "T"))
                ) {
                    let calibrated_utc = calibrated_at.to_utc();
                    calibrated_utc >= *current_period_start && calibrated_utc <= *current_period_end
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Normalize a legacy quota_type to the new format
    /// "5h" → ("sliding", "5h")  — old 5h was rolling/sliding
    /// "weekly" → ("fixed", "7d")
    /// "monthly" → ("fixed", "30d")
    pub fn normalize_quota_type(quota_type: &str) -> (String, String) {
        match quota_type {
            "5h" => ("sliding".to_string(), "5h".to_string()),
            "weekly" => ("fixed".to_string(), "7d".to_string()),
            "monthly" => ("fixed".to_string(), "30d".to_string()),
            other => {
                // New format: "fixed:5h" or "sliding:5h"
                if let Some((mode, size)) = other.split_once(':') {
                    (mode.to_string(), size.to_string())
                } else {
                    // Unknown format → default to fixed
                    ("fixed".to_string(), other.to_string())
                }
            }
        }
    }
}

// ========== Model Row types ==========

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ModelRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub model_type: String,
    pub is_active: bool,
    pub priority: i64,
    pub config: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ModelMappingRow {
    pub id: i64,
    pub model_id: String,
    pub provider_id: String,
    pub provider_model_id: String,
    pub is_active: bool,
    pub weight: i64,
    pub cost_multiplier: f64,
    pub created_at: String,
}

// ========== Unified Model response types ==========

#[derive(Debug, Clone, Serialize)]
pub struct ModelWithMappings {
    pub model: ModelRow,
    pub mappings: Vec<ModelMappingWithProvider>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelMappingWithProvider {
    pub mapping: ModelMappingRow,
    pub provider: ProviderRow,
}

// ========== Row types ==========

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ApiKeyRow {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub key_prefix: String,
    pub allowed_providers: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_type: String,
    pub auth_type: String,
    pub api_key: Option<String>,
    pub token_url: Option<String>,
    pub token_username: Option<String>,
    pub token_password: Option<String>,
    pub token_request_method: Option<String>,
    pub token_content_type: Option<String>,
    pub token_username_field: Option<String>,
    pub token_password_field: Option<String>,
    pub token_body_template: Option<String>,
    pub token_extra_headers: Option<String>,
    pub token_cookies: Option<String>,
    pub token_field: String,
    pub refresh_token_field: String,
    pub token_header_field: String,
    pub token_header_prefix: String,
    pub token_expiry_seconds: i64,
    pub current_token: Option<String>,
    pub current_refresh_token: Option<String>,
    pub token_expires_at: Option<String>,
    pub is_active: bool,
    pub weight: i64,
    pub bypass_proxy: bool,
    pub response_content_path: String,
    pub response_reasoning_path: String,
    pub chart_color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProviderModelRow {
    pub id: i64,
    pub provider_id: String,
    pub model_id: String,
    pub is_active: bool,
    pub last_test_status: Option<String>,
    pub last_test_message: Option<String>,
    pub last_tested_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
/// Lightweight log row for list queries — excludes request_body and response_body
/// to avoid transferring large payloads when only displaying a table.
pub struct RequestLogListRow {
    pub id: i64,
    pub api_key_id: String,
    pub provider_id: String,
    pub model: Option<String>,
    pub request_path: String,
    pub request_method: String,
    pub response_status: Option<i32>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: Option<i64>,
    pub is_streaming: bool,
    pub is_throttled: bool,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RequestLogRow {
    pub id: i64,
    pub api_key_id: String,
    pub provider_id: String,
    pub model: Option<String>,
    pub request_path: String,
    pub request_method: String,
    pub request_headers: Option<String>,
    pub request_body: Option<String>,
    pub response_status: Option<i32>,
    pub response_headers: Option<String>,
    pub response_body: Option<String>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: Option<i64>,
    pub is_streaming: bool,
    pub is_throttled: bool,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TokenRateRow {
    pub id: i64,
    pub provider_id: Option<String>,
    pub tokens_per_second: f64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub request_count: i64,
    pub elapsed_seconds: f64,
    pub snapshot_time: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AggregateStats {
    pub total_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub throttle_count: i64,
    pub error_count: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ApiKeyStatsRow {
    pub api_key_id: String,
    pub request_count: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub avg_duration_ms: f64,
    pub throttle_count: i64,
    pub error_count: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TimeBucketStats {
    pub period: String,
    pub request_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub throttle_count: i64,
    pub error_count: i64,
    pub avg_duration_ms: f64,
}



#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealthStats {
    pub provider_id: String,
    pub request_count_24h: i64,
    pub error_count_24h: i64,
    pub throttle_count_24h: i64,
    pub avg_duration_ms: f64,
    pub last_request_at: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct DashboardSummaryData {
    pub total_api_keys: i64,
    pub active_api_keys: i64,
    pub total_providers: i64,
    pub active_providers: i64,
    pub total_requests_24h: i64,
    pub total_tokens_24h: i64,
    pub avg_tokens_per_second: f64,
    pub throttle_count_24h: i64,
}

// ========== Quota Row types ==========

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProviderQuotaRow {
    pub id: i64,
    pub provider_id: String,
    pub quota_type: String,
    pub window_mode: String,
    pub window_size: String,
    pub window_start_override: Option<String>,
    pub limit_count: i64,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProviderQuotaCalibrationRow {
    pub id: i64,
    pub provider_id: String,
    pub quota_type: String,
    pub calibration_offset: i64,
    pub calibrated_at: String,
    pub calibration_window_start: Option<String>,
    pub calibration_window_end: Option<String>,
    pub note: Option<String>,
}

/// Computed quota usage info for a provider
#[derive(Debug, Clone, Serialize)]
pub struct QuotaUsageInfo {
    pub provider_id: String,
    pub provider_name: String,
    pub quota_type: String,
    pub window_mode: String,
    pub window_size: String,
    pub window_start_override: Option<String>,
    pub limit_count: i64,
    pub is_enabled: bool,
    /// Requests counted by the gateway in the current period
    pub gateway_count: i64,
    /// Manual calibration offset (for requests made outside the gateway)
    pub calibration_offset: i64,
    /// Whether the calibration is still valid in the current window
    pub calibration_valid: bool,
    /// The window period when the calibration was set
    pub calibration_window_start: Option<String>,
    pub calibration_window_end: Option<String>,
    /// Total estimated usage = gateway_count + calibration_offset (only if calibration_valid)
    pub estimated_total: i64,
    /// Remaining = limit_count - estimated_total
    pub remaining: i64,
    /// Usage percentage (0.0 - 100.0)
    pub usage_percent: f64,
    /// The start time of the current period
    pub period_start: String,
    /// The current time (for reference)
    pub period_current: String,
}
