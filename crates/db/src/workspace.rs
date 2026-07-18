use ade_core::error::AdeError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use turso::Connection;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub root_path: Option<String>,
    pub recipe_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct WorkspaceStore {
    connection: Connection,
}

impl WorkspaceStore {
    pub fn new(connection: Connection) -> Self {
        Self { connection }
    }

    pub async fn create(
        &self,
        name: &str,
        root_path: Option<&str>,
        recipe_id: Option<&str>,
    ) -> Result<Workspace, AdeError> {
        let name = name.trim();
        if name.is_empty() || name.len() > 128 {
            return Err(AdeError::PlanValidation(
                "workspace name must be 1-128 characters".into(),
            ));
        }
        let workspace = Workspace {
            id: Uuid::new_v4(),
            name: name.to_string(),
            root_path: root_path.map(str::to_string),
            recipe_id: recipe_id.map(str::to_string),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.connection
            .execute(
                "INSERT INTO workspaces (id, name, root_path, recipe_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    workspace.id.to_string(),
                    workspace.name.clone(),
                    optional_text(workspace.root_path.clone()),
                    optional_text(workspace.recipe_id.clone()),
                    workspace.created_at.to_rfc3339(),
                    workspace.updated_at.to_rfc3339(),
                ),
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(workspace)
    }

    pub async fn list(&self) -> Result<Vec<Workspace>, AdeError> {
        let mut rows = self
            .connection
            .query(
                "SELECT id, name, root_path, recipe_id, created_at, updated_at
                 FROM workspaces ORDER BY name",
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
            result.push(workspace_from_row(&row)?);
        }
        Ok(result)
    }

    /// Deletes by id (UUID) or exact name; returns whether a row was removed.
    pub async fn delete(&self, id_or_name: &str) -> Result<bool, AdeError> {
        let affected = self
            .connection
            .execute(
                "DELETE FROM workspaces WHERE id = ?1 OR name = ?1",
                [id_or_name],
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(affected > 0)
    }
}

fn optional_text(value: Option<String>) -> turso::Value {
    match value {
        Some(text) => turso::Value::Text(text),
        None => turso::Value::Null,
    }
}

fn workspace_from_row(row: &turso::Row) -> Result<Workspace, AdeError> {
    let get_text = |index: usize| -> Result<String, AdeError> {
        row.get(index)
            .map_err(|error| AdeError::Database(error.to_string()))
    };
    let get_optional = |index: usize| -> Result<Option<String>, AdeError> {
        row.get(index)
            .map_err(|error| AdeError::Database(error.to_string()))
    };
    let parse_time = |value: String| -> Result<DateTime<Utc>, AdeError> {
        DateTime::parse_from_rfc3339(&value)
            .map(|time| time.with_timezone(&Utc))
            .map_err(|error| AdeError::Database(error.to_string()))
    };
    Ok(Workspace {
        id: Uuid::parse_str(&get_text(0)?)
            .map_err(|error| AdeError::Database(error.to_string()))?,
        name: get_text(1)?,
        root_path: get_optional(2)?,
        recipe_id: get_optional(3)?,
        created_at: parse_time(get_text(4)?)?,
        updated_at: parse_time(get_text(5)?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::AdeDatabase;

    async fn store() -> WorkspaceStore {
        let database = AdeDatabase::open_path(":memory:").await.unwrap();
        WorkspaceStore::new(database.connect().unwrap())
    }

    #[tokio::test]
    async fn creates_lists_and_deletes_workspaces() {
        let store = store().await;
        let created = store
            .create("demo", Some(r"C:\Dev\demo"), Some("rust-api-turso"))
            .await
            .unwrap();

        let listed = store.list().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].root_path.as_deref(), Some(r"C:\Dev\demo"));

        assert!(store.delete("demo").await.unwrap());
        assert!(store.list().await.unwrap().is_empty());
        assert!(!store.delete("demo").await.unwrap());
    }

    #[tokio::test]
    async fn rejects_blank_and_duplicate_names() {
        let store = store().await;
        assert!(store.create("  ", None, None).await.is_err());

        store.create("demo", None, None).await.unwrap();
        assert!(store.create("demo", None, None).await.is_err());
    }

    #[tokio::test]
    async fn deletes_by_id_too() {
        let store = store().await;
        let created = store.create("byid", None, None).await.unwrap();
        assert!(store.delete(&created.id.to_string()).await.unwrap());
    }
}
