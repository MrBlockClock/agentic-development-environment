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

pub async fn migrate(connection: &Connection) -> Result<(), AdeError> {
    info!("Running database migrations...");
    for migration in [MIGRATION_V1, MIGRATION_V2] {
        connection
            .execute_batch(migration)
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
    }
    Ok(())
}
