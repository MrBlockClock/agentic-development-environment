use ade_core::error::AdeError;
use tracing::info;
use turso::Connection;

const MIGRATION_V1: &str = r#"
BEGIN;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS reports (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    schema_name TEXT NOT NULL,
    workspace_root TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reports_root_kind_created
    ON reports (workspace_root, kind, created_at DESC);

INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (1, CURRENT_TIMESTAMP);

COMMIT;
"#;

const MIGRATION_V2: &str = r#"
BEGIN;

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    root_path TEXT,
    recipe_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS analytics_events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    workspace_root TEXT,
    detail TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_analytics_type_created
    ON analytics_events (event_type, created_at DESC);

INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (2, CURRENT_TIMESTAMP);

COMMIT;
"#;

const MIGRATION_V3: &str = r#"
BEGIN;

CREATE TABLE IF NOT EXISTS usage_ledger_entries (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    workspace_root TEXT NOT NULL,
    actor TEXT,
    scope TEXT NOT NULL,
    period_key TEXT NOT NULL,
    provider TEXT,
    model TEXT,
    status TEXT NOT NULL,
    reserved_micros INTEGER NOT NULL DEFAULT 0,
    actual_micros INTEGER NOT NULL DEFAULT 0,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    expires_at TEXT,
    reconciled_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_usage_ledger_workspace_status
    ON usage_ledger_entries (workspace_root, status, expires_at);

CREATE INDEX IF NOT EXISTS idx_usage_ledger_scope_period
    ON usage_ledger_entries (scope, period_key, status);

CREATE INDEX IF NOT EXISTS idx_usage_ledger_session
    ON usage_ledger_entries (session_id, created_at);

INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (3, CURRENT_TIMESTAMP);

COMMIT;
"#;

const MIGRATION_V4: &str = r#"
BEGIN;

ALTER TABLE usage_ledger_entries ADD COLUMN hard_cap_micros INTEGER;

INSERT OR IGNORE INTO schema_migrations (version, applied_at)
VALUES (4, CURRENT_TIMESTAMP);

COMMIT;
"#;

pub async fn migrate(connection: &Connection) -> Result<(), AdeError> {
    info!("Running database migrations...");
    for migration in [MIGRATION_V1, MIGRATION_V2, MIGRATION_V3] {
        connection
            .execute_batch(migration)
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
    }
    let version = current_schema_version(connection).await?;
    if version < 4 {
        connection
            .execute_batch(MIGRATION_V4)
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
    }
    Ok(())
}

async fn current_schema_version(connection: &Connection) -> Result<i64, AdeError> {
    let mut rows = connection
        .query(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            (),
        )
        .await
        .map_err(|error| AdeError::Database(error.to_string()))?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| AdeError::Database(error.to_string()))?
    else {
        return Ok(0);
    };
    row.get::<i64>(0)
        .map_err(|error| AdeError::Database(error.to_string()))
}
