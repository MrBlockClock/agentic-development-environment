use ade_core::error::AdeError;
use ade_core::money::Money;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use turso::transaction::{Transaction, TransactionBehavior};
use turso::Connection;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerStatus {
    Reserved,
    Committed,
    Released,
    Failed,
}

impl LedgerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Committed => "committed",
            Self::Released => "released",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, AdeError> {
        match value {
            "reserved" => Ok(Self::Reserved),
            "committed" => Ok(Self::Committed),
            "released" => Ok(Self::Released),
            "failed" => Ok(Self::Failed),
            other => Err(AdeError::Database(format!(
                "unknown usage ledger status '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveRequest {
    pub session_id: Uuid,
    pub workspace_root: String,
    pub actor: Option<String>,
    pub scope: String,
    pub period_key: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub estimate: Money,
    pub hard_cap: Money,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reservation {
    pub id: Uuid,
    pub reserved: Money,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCommit {
    pub reservation_id: Uuid,
    pub actual: Money,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone)]
pub struct UsageLedgerStore {
    connection: Connection,
    /// Serializes reserve/check on a shared Connection clone family.
    write_lock: Arc<tokio::sync::Mutex<()>>,
}

impl UsageLedgerStore {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn active_spend(
        &self,
        scope: &str,
        period_key: &str,
        workspace_root: &str,
    ) -> Result<Money, AdeError> {
        let breakdown = self
            .active_spend_breakdown(scope, period_key, workspace_root)
            .await?;
        Ok(breakdown.active)
    }

    /// Invoice-class split: committed actuals vs still-open reserves.
    pub async fn active_spend_breakdown(
        &self,
        scope: &str,
        period_key: &str,
        workspace_root: &str,
    ) -> Result<SpendBreakdown, AdeError> {
        self.expire_stale().await?;
        Self::active_spend_breakdown_on(&self.connection, scope, period_key, workspace_root).await
    }

    pub async fn reserve(&self, request: ReserveRequest) -> Result<Reservation, AdeError> {
        let _guard = self.write_lock.lock().await;
        let tx = Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;

        if let Err(error) = Self::expire_stale_on(&tx).await {
            let _ = tx.rollback().await;
            return Err(error);
        }
        let active = match Self::active_spend_breakdown_on(
            &tx,
            &request.scope,
            &request.period_key,
            &request.workspace_root,
        )
        .await
        {
            Ok(breakdown) => breakdown.active,
            Err(error) => {
                let _ = tx.rollback().await;
                return Err(error);
            }
        };
        let next = active.saturating_add(request.estimate);
        if next > request.hard_cap {
            // unchecked_transaction does not auto-clear dangling txns — roll back now.
            tx.rollback()
                .await
                .map_err(|error| AdeError::Database(error.to_string()))?;
            return Err(AdeError::Spend(format!(
                "hard spend cap exceeded for {}/{}: {} + {} > {}",
                request.scope, request.period_key, active, request.estimate, request.hard_cap
            )));
        }

        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::seconds(request.ttl_secs.max(1) as i64);
        if let Err(error) = tx
            .execute(
                "INSERT INTO usage_ledger_entries (
                    id, session_id, workspace_root, actor, scope, period_key,
                    provider, model, status, reserved_micros, actual_micros,
                    input_tokens, output_tokens, created_at, expires_at, reconciled_at,
                    hard_cap_micros
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6,
                    ?7, ?8, ?9, ?10, 0,
                    0, 0, ?11, ?12, NULL,
                    ?13
                 )",
                (
                    id.to_string(),
                    request.session_id.to_string(),
                    request.workspace_root.clone(),
                    optional_text(request.actor.as_deref()),
                    request.scope.clone(),
                    request.period_key.clone(),
                    optional_text(request.provider.as_deref()),
                    optional_text(request.model.as_deref()),
                    LedgerStatus::Reserved.as_str().to_string(),
                    request.estimate.micros(),
                    created_at.to_rfc3339(),
                    expires_at.to_rfc3339(),
                    request.hard_cap.micros(),
                ),
            )
            .await
        {
            let _ = tx.rollback().await;
            return Err(AdeError::Database(error.to_string()));
        }
        tx.commit()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(Reservation {
            id,
            reserved: request.estimate,
            expires_at,
        })
    }

    async fn active_spend_breakdown_on(
        connection: &Connection,
        scope: &str,
        period_key: &str,
        workspace_root: &str,
    ) -> Result<SpendBreakdown, AdeError> {
        let now = Utc::now().to_rfc3339();
        let mut rows = connection
            .query(
                "SELECT
                    COALESCE(SUM(CASE WHEN status = 'committed' THEN actual_micros ELSE 0 END), 0),
                    COALESCE(SUM(CASE
                        WHEN status = 'reserved'
                             AND (expires_at IS NULL OR expires_at > ?1)
                        THEN reserved_micros
                        ELSE 0
                    END), 0)
                 FROM usage_ledger_entries
                 WHERE scope = ?2
                   AND period_key = ?3
                   AND workspace_root = ?4",
                (
                    now,
                    scope.to_string(),
                    period_key.to_string(),
                    workspace_root.to_string(),
                ),
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let Some(row) = rows
            .next()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?
        else {
            return Ok(SpendBreakdown::default());
        };
        let used_micros: i64 = row
            .get(0)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let reserved_micros: i64 = row
            .get(1)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let used = Money::from_micros(used_micros.max(0));
        let reserved = Money::from_micros(reserved_micros.max(0));
        Ok(SpendBreakdown {
            used,
            reserved,
            active: used.saturating_add(reserved),
        })
    }

    async fn expire_stale_on(connection: &Connection) -> Result<u64, AdeError> {
        let now = Utc::now().to_rfc3339();
        let updated = connection
            .execute(
                "UPDATE usage_ledger_entries
                 SET status = ?1, reconciled_at = ?2
                 WHERE status = 'reserved'
                   AND expires_at IS NOT NULL
                   AND expires_at <= ?2",
                (LedgerStatus::Failed.as_str().to_string(), now),
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(updated as u64)
    }

    pub async fn commit(&self, commit: UsageCommit) -> Result<(), AdeError> {
        let _guard = self.write_lock.lock().await;
        let tx = Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;

        let mut rows = match tx
            .query(
                "SELECT scope, period_key, workspace_root, reserved_micros, hard_cap_micros
                 FROM usage_ledger_entries
                 WHERE id = ?1 AND status = 'reserved'",
                [commit.reservation_id.to_string()],
            )
            .await
        {
            Ok(rows) => rows,
            Err(error) => {
                let _ = tx.rollback().await;
                return Err(AdeError::Database(error.to_string()));
            }
        };
        let row = match rows.next().await {
            Ok(Some(row)) => row,
            Ok(None) => {
                let _ = tx.rollback().await;
                return Err(AdeError::NotFound(format!(
                    "usage reservation '{}' not found or already reconciled",
                    commit.reservation_id
                )));
            }
            Err(error) => {
                let _ = tx.rollback().await;
                return Err(AdeError::Database(error.to_string()));
            }
        };

        let scope: String = row
            .get(0)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let period_key: String = row
            .get(1)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let workspace_root: String = row
            .get(2)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let reserved_micros: i64 = row
            .get(3)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let hard_cap_micros: Option<i64> = row.get(4).ok();

        if let Err(error) = Self::expire_stale_on(&tx).await {
            let _ = tx.rollback().await;
            return Err(error);
        }
        let active = match Self::active_spend_breakdown_on(&tx, &scope, &period_key, &workspace_root)
            .await
        {
            Ok(breakdown) => breakdown.active,
            Err(error) => {
                let _ = tx.rollback().await;
                return Err(error);
            }
        };
        // Active still includes this reservation's reserved amount.
        let without_this = Money::from_micros(
            active
                .micros()
                .saturating_sub(reserved_micros.max(0)),
        );
        let next = without_this.saturating_add(commit.actual);
        if let Some(cap_micros) = hard_cap_micros.filter(|value| *value >= 0) {
            let hard_cap = Money::from_micros(cap_micros);
            if next > hard_cap {
                let _ = tx.rollback().await;
                return Err(AdeError::Spend(format!(
                    "hard spend cap exceeded at commit for {scope}/{period_key}: {} + {} > {}",
                    without_this, commit.actual, hard_cap
                )));
            }
        }

        let updated = match tx
            .execute(
                "UPDATE usage_ledger_entries
                 SET status = ?1,
                     actual_micros = ?2,
                     input_tokens = ?3,
                     output_tokens = ?4,
                     reconciled_at = ?5
                 WHERE id = ?6 AND status = 'reserved'",
                (
                    LedgerStatus::Committed.as_str().to_string(),
                    commit.actual.micros(),
                    commit.input_tokens as i64,
                    commit.output_tokens as i64,
                    Utc::now().to_rfc3339(),
                    commit.reservation_id.to_string(),
                ),
            )
            .await
        {
            Ok(updated) => updated,
            Err(error) => {
                let _ = tx.rollback().await;
                return Err(AdeError::Database(error.to_string()));
            }
        };
        if updated == 0 {
            let _ = tx.rollback().await;
            return Err(AdeError::NotFound(format!(
                "usage reservation '{}' not found or already reconciled",
                commit.reservation_id
            )));
        }
        tx.commit()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(())
    }

    pub async fn release(&self, reservation_id: Uuid) -> Result<(), AdeError> {
        let _ = self
            .connection
            .execute(
                "UPDATE usage_ledger_entries
                 SET status = ?1, reconciled_at = ?2
                 WHERE id = ?3 AND status = 'reserved'",
                (
                    LedgerStatus::Released.as_str().to_string(),
                    Utc::now().to_rfc3339(),
                    reservation_id.to_string(),
                ),
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(())
    }

    pub async fn expire_stale(&self) -> Result<u64, AdeError> {
        Self::expire_stale_on(&self.connection).await
    }

    pub async fn entry_status(&self, id: Uuid) -> Result<LedgerStatus, AdeError> {
        let mut rows = self
            .connection
            .query(
                "SELECT status FROM usage_ledger_entries WHERE id = ?1",
                [id.to_string()],
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let row = rows
            .next()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?
            .ok_or_else(|| AdeError::NotFound(format!("usage reservation '{id}' not found")))?;
        let status: String = row
            .get(0)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        LedgerStatus::parse(&status)
    }

    pub async fn has_committed_entry(
        &self,
        session_id: Uuid,
        scope: &str,
        workspace_root: &str,
    ) -> Result<bool, AdeError> {
        let mut rows = self
            .connection
            .query(
                "SELECT COUNT(*)
                 FROM usage_ledger_entries
                 WHERE session_id = ?1
                   AND scope = ?2
                   AND workspace_root = ?3
                   AND status = 'committed'",
                (
                    session_id.to_string(),
                    scope.to_string(),
                    workspace_root.to_string(),
                ),
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let Some(row) = rows
            .next()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?
        else {
            return Ok(false);
        };
        let count: i64 = row
            .get(0)
            .map_err(|error| AdeError::Database(error.to_string()))?;
        Ok(count > 0)
    }

    pub async fn recent_for_workspace(
        &self,
        workspace_root: &str,
        limit: u32,
    ) -> Result<Vec<LedgerEntryView>, AdeError> {
        self.expire_stale().await?;
        let limit = limit.clamp(1, 200) as i64;
        let mut rows = self
            .connection
            .query(
                "SELECT id, created_at, status, scope, period_key, provider, model,
                        reserved_micros, actual_micros, input_tokens, output_tokens
                 FROM usage_ledger_entries
                 WHERE workspace_root = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
                (workspace_root.to_string(), limit),
            )
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let mut out = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?
        {
            let reserved_micros: i64 = row
                .get(7)
                .map_err(|error| AdeError::Database(error.to_string()))?;
            let actual_micros: i64 = row
                .get(8)
                .map_err(|error| AdeError::Database(error.to_string()))?;
            out.push(LedgerEntryView {
                id: row
                    .get::<String>(0)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                created_at: row
                    .get::<String>(1)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                status: row
                    .get::<String>(2)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                scope: row
                    .get::<String>(3)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                period_key: row
                    .get::<String>(4)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                provider: row
                    .get(5)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                model: row
                    .get(6)
                    .map_err(|error| AdeError::Database(error.to_string()))?,
                reserved_usd: Money::from_micros(reserved_micros.max(0)).to_usd_f64(),
                actual_usd: Money::from_micros(actual_micros.max(0)).to_usd_f64(),
                input_tokens: row
                    .get::<i64>(9)
                    .map_err(|error| AdeError::Database(error.to_string()))?
                    .max(0) as u64,
                output_tokens: row
                    .get::<i64>(10)
                    .map_err(|error| AdeError::Database(error.to_string()))?
                    .max(0) as u64,
            });
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpendBreakdown {
    /// Committed actuals (invoice-class used $).
    pub used: Money,
    /// Still-open reserved estimates.
    pub reserved: Money,
    /// used + reserved (what caps see).
    pub active: Money,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntryView {
    pub id: String,
    pub created_at: String,
    pub status: String,
    pub scope: String,
    pub period_key: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub reserved_usd: f64,
    pub actual_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
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

    async fn store() -> UsageLedgerStore {
        let database = AdeDatabase::open_path(":memory:").await.unwrap();
        UsageLedgerStore::new(database.connect().unwrap())
    }

    fn request(estimate: Money, hard_cap: Money) -> ReserveRequest {
        ReserveRequest {
            session_id: Uuid::new_v4(),
            workspace_root: "/tmp/workspace".into(),
            actor: Some("tester".into()),
            scope: "workspace".into(),
            period_key: "day:2026-07-18".into(),
            provider: Some("openai".into()),
            model: Some("gpt".into()),
            estimate,
            hard_cap,
            ttl_secs: 60,
        }
    }

    #[tokio::test]
    async fn reserves_commits_and_enforces_hard_cap() {
        let store = store().await;
        let reserved = store
            .reserve(request(
                Money::from_usd_str("0.50").unwrap(),
                Money::from_usd_str("1.00").unwrap(),
            ))
            .await
            .unwrap();
        store
            .commit(UsageCommit {
                reservation_id: reserved.id,
                actual: Money::from_usd_str("0.40").unwrap(),
                input_tokens: 10,
                output_tokens: 5,
            })
            .await
            .unwrap();
        assert!(store
            .reserve(request(
                Money::from_usd_str("0.70").unwrap(),
                Money::from_usd_str("1.00").unwrap(),
            ))
            .await
            .is_err());
        let active = store
            .active_spend("workspace", "day:2026-07-18", "/tmp/workspace")
            .await
            .unwrap();
        assert_eq!(active, Money::from_usd_str("0.40").unwrap());
    }

    #[tokio::test]
    async fn releases_reservation_budget() {
        let store = store().await;
        let reserved = store
            .reserve(request(
                Money::from_usd_str("0.80").unwrap(),
                Money::from_usd_str("1.00").unwrap(),
            ))
            .await
            .unwrap();
        store.release(reserved.id).await.unwrap();
        store
            .reserve(request(
                Money::from_usd_str("0.90").unwrap(),
                Money::from_usd_str("1.00").unwrap(),
            ))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn finds_committed_entries_by_session_and_scope() {
        let store = store().await;
        let request = request(
            Money::from_usd_str("0.10").unwrap(),
            Money::from_usd_str("1.00").unwrap(),
        );
        let session_id = request.session_id;
        let scope = request.scope.clone();
        let workspace = request.workspace_root.clone();
        let reserved = store.reserve(request).await.unwrap();
        store
            .commit(UsageCommit {
                reservation_id: reserved.id,
                actual: Money::from_usd_str("0.05").unwrap(),
                input_tokens: 10,
                output_tokens: 2,
            })
            .await
            .unwrap();

        assert!(store
            .has_committed_entry(session_id, &scope, &workspace)
            .await
            .unwrap());
        assert!(!store
            .has_committed_entry(session_id, "missing", &workspace)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn commit_rejects_when_actual_exceeds_hard_cap() {
        let store = store().await;
        let reserved = store
            .reserve(request(
                Money::from_usd_str("0.40").unwrap(),
                Money::from_usd_str("1.00").unwrap(),
            ))
            .await
            .unwrap();
        // Fill remaining headroom with another reservation so commit of 0.70 would exceed.
        store
            .reserve(request(
                Money::from_usd_str("0.50").unwrap(),
                Money::from_usd_str("1.00").unwrap(),
            ))
            .await
            .unwrap();
        let err = store
            .commit(UsageCommit {
                reservation_id: reserved.id,
                actual: Money::from_usd_str("0.70").unwrap(),
                input_tokens: 1,
                output_tokens: 1,
            })
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("hard spend cap exceeded at commit"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn concurrent_reserves_respect_hard_cap() {
        let db_path = std::env::temp_dir().join(format!(
            "ade-ledger-race-{}.db",
            Uuid::new_v4()
        ));
        let database = AdeDatabase::open_path(&db_path).await.unwrap();
        let store_a = UsageLedgerStore::new(database.connect().unwrap());
        let store_b = UsageLedgerStore::new(database.connect().unwrap());
        let estimate = Money::from_usd_str("0.60").unwrap();
        let hard_cap = Money::from_usd_str("1.00").unwrap();
        let (first, second) = tokio::join!(
            store_a.reserve(request(estimate, hard_cap)),
            store_b.reserve(request(estimate, hard_cap)),
        );
        let ok = first.is_ok() as u8 + second.is_ok() as u8;
        let _ = std::fs::remove_file(&db_path);
        assert_eq!(ok, 1, "exactly one of two racing reserves should succeed");
        let active = store_a
            .active_spend("workspace", "day:2026-07-18", "/tmp/workspace")
            .await
            .unwrap();
        assert_eq!(active, estimate);
    }
}
