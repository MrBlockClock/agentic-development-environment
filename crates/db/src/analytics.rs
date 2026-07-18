use ade_core::error::AdeError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use turso::Connection;
use uuid::Uuid;

/// Lightweight phase/tool event log persisted in the local Turso database.
///
/// Rich model-quality analytics (tokens, cost, accept rate) will layer on top
/// of this once provider integrations land.
#[derive(Clone)]
pub struct AnalyticsStore {
    connection: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedEvent {
    pub id: Uuid,
    pub event_type: String,
    pub workspace_root: Option<String>,
    pub detail: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSummary {
    pub event_type: String,
    pub count: u64,
    pub last_seen: DateTime<Utc>,
}

impl AnalyticsStore {
    pub fn new(connection: Connection) -> Self {
        Self { connection }
    }

    pub async fn record(
        &self,
        event_type: &str,
        workspace_root: Option<&str>,
        detail: Option<&str>,
    ) -> Result<Uuid, AdeError> {
        let id = Uuid::new_v4();
        self.connection
            .execute(
                "INSERT INTO analytics_events (id, event_type, workspace_root, detail, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    id.to_string(),
                    event_type.to_string(),
                    optional_text(workspace_root),
                    optional_text(detail),
                    Utc::now().to_rfc3339(),
                ),
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(id)
    }

    pub async fn summary(&self) -> Result<Vec<EventSummary>, AdeError> {
        let mut rows = self
            .connection
            .query(
                "SELECT event_type, COUNT(*), MAX(created_at)
                 FROM analytics_events
                 GROUP BY event_type
                 ORDER BY COUNT(*) DESC, event_type",
                (),
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let mut result = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?
        {
            let event_type: String = row
                .get(0)
                .map_err(|error| AdeError::Database(error.to_string()))?;
            let count: i64 = row
                .get(1)
                .map_err(|error| AdeError::Database(error.to_string()))?;
            let last_seen: String = row
                .get(2)
                .map_err(|error| AdeError::Database(error.to_string()))?;
            result.push(EventSummary {
                event_type,
                count: count.max(0) as u64,
                last_seen: DateTime::parse_from_rfc3339(&last_seen)
                    .map_err(|error| AdeError::Database(error.to_string()))?
                    .with_timezone(&Utc),
            });
        }
        Ok(result)
    }

    pub async fn recent(&self, limit: u32) -> Result<Vec<RecordedEvent>, AdeError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, event_type, workspace_root, detail, created_at
                 FROM analytics_events
                 ORDER BY created_at DESC
                 LIMIT ?1",
                [limit as i64],
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let mut result = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?
        {
            let id: String = row
                .get(0)
                .map_err(|error| AdeError::Database(error.to_string()))?;
            let created_at: String = row
                .get(4)
                .map_err(|error| AdeError::Database(error.to_string()))?;
            result.push(RecordedEvent {
                id: Uuid::parse_str(&id).map_err(|error| AdeError::Database(error.to_string()))?,
                event_type: row
                    .get(1)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                workspace_root: row
                    .get(2)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                detail: row
                    .get(3)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|error| AdeError::Database(error.to_string()))?
                    .with_timezone(&Utc),
            });
        }
        Ok(result)
    }
}

fn optional_text(value: Option<&str>) -> turso::Value {
    match value {
        Some(text) => turso::Value::Text(text.to_string()),
        None => turso::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::AdeDatabase;

    async fn store() -> AnalyticsStore {
        let database = AdeDatabase::open_path(":memory:").await.unwrap();
        AnalyticsStore::new(database.connect().unwrap())
    }

    #[tokio::test]
    async fn records_and_summarizes_events() {
        let store = store().await;
        store
            .record("audit_run", Some(r"C:\Dev\demo"), None)
            .await
            .unwrap();
        store
            .record("audit_run", Some(r"C:\Dev\demo"), None)
            .await
            .unwrap();
        store
            .record("verify_run", Some(r"C:\Dev\demo"), Some("G3"))
            .await
            .unwrap();

        let summary = store.summary().await.unwrap();
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].event_type, "audit_run");
        assert_eq!(summary[0].count, 2);
        assert_eq!(summary[1].event_type, "verify_run");
        assert_eq!(summary[1].count, 1);
    }

    #[tokio::test]
    async fn lists_recent_events_newest_first() {
        let store = store().await;
        store.record("plan_run", None, None).await.unwrap();
        store
            .record("mcp_call", None, Some("filesystem/list_directory"))
            .await
            .unwrap();

        let recent = store.recent(10).await.unwrap();
        assert_eq!(recent.len(), 2);
        assert!(recent
            .iter()
            .any(|event| event.detail.as_deref() == Some("filesystem/list_directory")));
    }
}
