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

pub async fn migrate(connection: &Connection) -> Result<(), AdeError> {
    info!("Running database migrations...");
    connection
        .execute_batch(MIGRATION_V1)
        .await
        .map_err(|error| AdeError::Database(error.to_string()))?;
    Ok(())
}
