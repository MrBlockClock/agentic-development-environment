use ade_core::error::AdeError;
use ade_core::money::Money;
use ade_db::usage_ledger::{Reservation, ReserveRequest, UsageCommit, UsageLedgerStore};
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpendScope {
    Session,
    Workspace,
    User,
    Model,
}

impl SpendScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Workspace => "workspace",
            Self::User => "user",
            Self::Model => "model",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpendPeriod {
    Session,
    Day,
    Week,
    Month,
    Lifetime,
}

impl SpendPeriod {
    pub fn key(self, session_id: Uuid) -> String {
        let now = Utc::now();
        match self {
            Self::Session => format!("session:{session_id}"),
            Self::Day => format!("day:{}", now.format("%Y-%m-%d")),
            Self::Week => {
                let iso = now.iso_week();
                format!("week:{}-W{:02}", iso.year(), iso.week())
            }
            Self::Month => format!("month:{}", now.format("%Y-%m")),
            Self::Lifetime => "lifetime".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendPolicy {
    pub scope: SpendScope,
    pub period: SpendPeriod,
    pub hard_cap: Money,
    pub soft_cap: Option<Money>,
    pub model: Option<String>,
}

impl SpendPolicy {
    pub fn session(hard_cap: Money) -> Self {
        Self {
            scope: SpendScope::Session,
            period: SpendPeriod::Session,
            hard_cap,
            soft_cap: None,
            model: None,
        }
    }

    pub fn daily_workspace(hard_cap: Money) -> Self {
        Self {
            scope: SpendScope::Workspace,
            period: SpendPeriod::Day,
            hard_cap,
            soft_cap: None,
            model: None,
        }
    }
}

/// Compatibility wrapper for older session/daily USD env caps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendCaps {
    pub session: Money,
    pub daily: Money,
}

impl Default for SpendCaps {
    fn default() -> Self {
        Self::from_env()
    }
}

impl SpendCaps {
    pub fn from_env() -> Self {
        Self {
            session: env_money("ADE_SESSION_SPEND_CAP_USD", "1.0"),
            daily: env_money("ADE_DAILY_SPEND_CAP_USD", "10.0"),
        }
    }

    pub fn unlimited() -> Self {
        Self {
            session: Money::from_micros(i64::MAX / 4),
            daily: Money::from_micros(i64::MAX / 4),
        }
    }

    pub fn policies(&self) -> Vec<SpendPolicy> {
        vec![
            SpendPolicy::session(self.session),
            SpendPolicy::daily_workspace(self.daily),
        ]
    }

    /// True when session/daily caps are finite and non-zero (SpendGuard can bind).
    pub fn has_enforced_caps(&self) -> bool {
        let unlimited = Money::from_micros(i64::MAX / 4);
        (self.session > Money::ZERO && self.session < unlimited)
            || (self.daily > Money::ZERO && self.daily < unlimited)
    }
}

/// When caps are enforced, rates must be non-zero unless allowed unpriced.
pub fn require_priced_for_caps(
    caps: &SpendCaps,
    input_cost: Money,
    output_cost: Money,
) -> Result<(), AdeError> {
    require_priced_for_caps_with_override(caps, input_cost, output_cost, false)
}

pub fn require_priced_for_caps_with_override(
    caps: &SpendCaps,
    input_cost: Money,
    output_cost: Money,
    allow_unpriced: bool,
) -> Result<(), AdeError> {
    if !caps.has_enforced_caps() {
        return Ok(());
    }
    let priced = input_cost > Money::ZERO || output_cost > Money::ZERO;
    if priced {
        return Ok(());
    }
    if allow_unpriced || allow_unpriced_from_env() {
        return Ok(());
    }
    Err(AdeError::Config(
        "spend_honesty: session/daily caps are set but Input/Output $/MTok are $0 — \
         caps cannot reserve real dollars. Set rates to your provider invoice class, \
         confirm unmetered for this turn, or set ADE_ALLOW_UNPRICED=1."
            .into(),
    ))
}

fn allow_unpriced_from_env() -> bool {
    matches!(
        std::env::var("ADE_ALLOW_UNPRICED")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpendOutcome {
    Allowed,
    SoftWarning {
        scope: SpendScope,
        period_key: String,
        projected: Money,
        soft_cap: Money,
    },
}

#[derive(Clone)]
pub struct SpendGuard {
    policies: Vec<SpendPolicy>,
    session_id: Uuid,
    workspace_root: PathBuf,
    actor: Option<String>,
    ledger: UsageLedgerStore,
    reservation_ttl_secs: u64,
}

impl SpendGuard {
    pub fn new(
        workspace_root: impl Into<PathBuf>,
        session_id: Uuid,
        caps: SpendCaps,
        ledger: UsageLedgerStore,
    ) -> Self {
        Self::with_policies(workspace_root, session_id, caps.policies(), ledger)
    }

    pub fn with_policies(
        workspace_root: impl Into<PathBuf>,
        session_id: Uuid,
        policies: Vec<SpendPolicy>,
        ledger: UsageLedgerStore,
    ) -> Self {
        Self {
            policies,
            session_id,
            workspace_root: workspace_root.into(),
            actor: None,
            ledger,
            reservation_ttl_secs: 120,
        }
    }

    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn policies(&self) -> &[SpendPolicy] {
        &self.policies
    }

    /// Reserve worst-case cost against every hard cap. Returns soft warnings if thresholds crossed.
    pub async fn reserve(
        &self,
        estimate: Money,
        provider: &str,
        model: &str,
    ) -> Result<(Vec<Reservation>, Vec<SpendOutcome>), AdeError> {
        let mut reservations = Vec::new();
        let mut outcomes = Vec::new();
        let workspace = self.workspace_root.display().to_string();

        for policy in &self.policies {
            if let Some(expected) = &policy.model {
                if expected != model {
                    continue;
                }
            }
            let period_key = policy.period.key(self.session_id);
            let scope_key = match policy.scope {
                SpendScope::Model => format!("model:{model}"),
                SpendScope::User => format!(
                    "user:{}",
                    self.actor.clone().unwrap_or_else(|| "anonymous".into())
                ),
                other => other.as_str().to_string(),
            };
            let active = self
                .ledger
                .active_spend(&scope_key, &period_key, &workspace)
                .await?;
            let projected = active.saturating_add(estimate);
            if let Some(soft) = policy.soft_cap {
                if projected > soft && projected <= policy.hard_cap {
                    outcomes.push(SpendOutcome::SoftWarning {
                        scope: policy.scope,
                        period_key: period_key.clone(),
                        projected,
                        soft_cap: soft,
                    });
                }
            }
            match self
                .ledger
                .reserve(ReserveRequest {
                    session_id: self.session_id,
                    workspace_root: workspace.clone(),
                    actor: self.actor.clone(),
                    scope: scope_key,
                    period_key,
                    provider: Some(provider.into()),
                    model: Some(model.into()),
                    estimate,
                    hard_cap: policy.hard_cap,
                    ttl_secs: self.reservation_ttl_secs,
                })
                .await
            {
                Ok(reservation) => reservations.push(reservation),
                Err(error) => {
                    for prior in &reservations {
                        let _ = self.ledger.release(prior.id).await;
                    }
                    return Err(error);
                }
            }
        }
        if outcomes.is_empty() {
            outcomes.push(SpendOutcome::Allowed);
        }
        Ok((reservations, outcomes))
    }

    pub async fn reconcile(
        &self,
        reservations: &[Reservation],
        actual: Money,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Result<(), AdeError> {
        for reservation in reservations {
            self.ledger
                .commit(UsageCommit {
                    reservation_id: reservation.id,
                    actual,
                    input_tokens,
                    output_tokens,
                })
                .await?;
        }
        Ok(())
    }

    pub async fn release(&self, reservations: &[Reservation]) -> Result<(), AdeError> {
        for reservation in reservations {
            self.ledger.release(reservation.id).await?;
        }
        Ok(())
    }
}

fn env_money(name: &str, default: &str) -> Money {
    std::env::var(name)
        .ok()
        .and_then(|value| Money::from_usd_str(&value).ok())
        .unwrap_or_else(|| Money::from_usd_str(default).expect("default USD amount"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_db::repo::AdeDatabase;

    async fn make_guard(caps: SpendCaps) -> (SpendGuard, PathBuf) {
        let root = std::env::temp_dir().join(format!("ade-spend-{}", Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&root);
        let database = AdeDatabase::open_path(":memory:").await.unwrap();
        let guard = SpendGuard::new(
            &root,
            Uuid::new_v4(),
            caps,
            UsageLedgerStore::new(database.connect().unwrap()),
        );
        (guard, root)
    }

    #[tokio::test]
    async fn blocks_session_overspend_on_reserve() {
        let (guard, root) = make_guard(SpendCaps {
            session: Money::from_usd_str("0.01").unwrap(),
            daily: Money::from_usd_str("10.0").unwrap(),
        })
        .await;
        let estimate = Money::from_usd_str("0.02").unwrap();
        assert!(guard.reserve(estimate, "mock", "mock-1").await.is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn require_priced_blocks_zero_rates_when_caps_on() {
        let prev = std::env::var("ADE_ALLOW_UNPRICED").ok();
        std::env::remove_var("ADE_ALLOW_UNPRICED");
        let caps = SpendCaps {
            session: Money::from_usd_str("1.0").unwrap(),
            daily: Money::from_usd_str("10.0").unwrap(),
        };
        let err = require_priced_for_caps(&caps, Money::ZERO, Money::ZERO).unwrap_err();
        assert!(err.to_string().contains("spend_honesty"));
        assert!(require_priced_for_caps_with_override(
            &caps,
            Money::ZERO,
            Money::ZERO,
            true
        )
        .is_ok());
        match prev {
            Some(value) => std::env::set_var("ADE_ALLOW_UNPRICED", value),
            None => std::env::remove_var("ADE_ALLOW_UNPRICED"),
        }
    }

    #[tokio::test]
    async fn reconciles_under_estimate() {
        let (guard, root) = make_guard(SpendCaps {
            session: Money::from_usd_str("1.0").unwrap(),
            daily: Money::from_usd_str("5.0").unwrap(),
        })
        .await;
        let (reservations, _) = guard
            .reserve(Money::from_usd_str("0.50").unwrap(), "mock", "mock-1")
            .await
            .unwrap();
        guard
            .reconcile(&reservations, Money::from_usd_str("0.25").unwrap(), 100, 50)
            .await
            .unwrap();
        let (again, _) = guard
            .reserve(Money::from_usd_str("0.70").unwrap(), "mock", "mock-1")
            .await
            .unwrap();
        assert!(!again.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
