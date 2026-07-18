use ade_core::error::AdeError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use turso::Connection;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    Audit,
    Plan,
    Execute,
    Verify,
}

impl ReportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audit => "audit",
            Self::Plan => "plan",
            Self::Execute => "execute",
            Self::Verify => "verify",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredReport {
    pub id: Uuid,
    pub kind: String,
    pub schema_name: String,
    pub workspace_root: String,
    pub payload_json: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ReportStore {
    connection: Connection,
}

impl ReportStore {
    pub fn new(connection: Connection) -> Self {
        Self { connection }
    }

    pub async fn save<T: Serialize>(
        &self,
        kind: ReportKind,
        schema_name: &str,
        workspace_root: &str,
        report: &T,
    ) -> Result<Uuid, AdeError> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let payload = serde_json::to_string(report)?;
        let params = [
            id.to_string(),
            kind.as_str().to_string(),
            schema_name.to_string(),
            workspace_root.to_string(),
            payload,
            created_at.to_rfc3339(),
        ];
        self.connection
            .execute(
                "INSERT INTO reports
                 (id, kind, schema_name, workspace_root, payload_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params,
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(id)
    }

    pub async fn latest(
        &self,
        kind: ReportKind,
        workspace_root: &str,
    ) -> Result<Option<StoredReport>, AdeError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, kind, schema_name, workspace_root, payload_json, created_at
                 FROM reports
                 WHERE kind = ?1 AND workspace_root = ?2
                 ORDER BY created_at DESC
                 LIMIT 1",
                [kind.as_str(), workspace_root],
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;

        let Some(row) = rows
            .next()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?
        else {
            return Ok(None);
        };

        let id: String = row
            .get(0)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let created_at: String = row
            .get(5)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(Some(StoredReport {
            id: Uuid::parse_str(&id).map_err(|error| AdeError::Database(error.to_string()))?,
            kind: row
                .get(1)
                .map_err(|error| AdeError::Database(error.to_string()))?,
            schema_name: row
                .get(2)
                .map_err(|error| AdeError::Database(error.to_string()))?,
            workspace_root: row
                .get(3)
                .map_err(|error| AdeError::Database(error.to_string()))?,
            payload_json: row
                .get(4)
                .map_err(|error| AdeError::Database(error.to_string()))?,
            created_at: DateTime::parse_from_rfc3339(&created_at)
                .map_err(|error| AdeError::Database(error.to_string()))?
                .with_timezone(&Utc),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::AdeDatabase;

    #[tokio::test]
    async fn persists_and_loads_latest_report() {
        let database = AdeDatabase::open_path(":memory:").await.unwrap();
        let store = ReportStore::new(database.connect().unwrap());
        let payload = serde_json::json!({"score": 42});

        let id = store
            .save(
                ReportKind::Audit,
                "ade.audit.report/v1",
                r"C:\Dev\demo",
                &payload,
            )
            .await
            .unwrap();
        let stored = store
            .latest(ReportKind::Audit, r"C:\Dev\demo")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.id, id);
        assert_eq!(stored.kind, "audit");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&stored.payload_json).unwrap(),
            payload
        );
    }

    #[tokio::test]
    async fn latest_is_scoped_to_workspace() {
        let database = AdeDatabase::open_path(":memory:").await.unwrap();
        let store = ReportStore::new(database.connect().unwrap());
        store
            .save(
                ReportKind::Plan,
                "ade.plan.report/v1",
                "workspace-a",
                &serde_json::json!({}),
            )
            .await
            .unwrap();

        assert!(store
            .latest(ReportKind::Plan, "workspace-b")
            .await
            .unwrap()
            .is_none());
    }
}
