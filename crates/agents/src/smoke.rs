use crate::authority::AuthorityEnforcer;
use crate::ignore_enforcer::IgnoreEnforcer;
use crate::session::AgentEvent;
use crate::spend::{SpendCaps, SpendGuard};
use crate::turn::{AgentTurnBuilder, AgentTurnSpec};
use ade_core::error::AdeError;
use ade_core::money::Money;
use ade_core::recipe::canonical_recipe_ids;
use ade_db::repo::{AdeDatabase, DbConfig};
use ade_db::secrets::{NativeProviderKeyVault, ProviderKeyVault};
use ade_db::usage_ledger::UsageLedgerStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct SmokeReport {
    pub ok: bool,
    pub checks: Vec<SmokeCheck>,
}

#[derive(Debug, Serialize)]
pub struct SmokeCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct LiveSmokeSpec {
    pub workspace_root: PathBuf,
    pub profile: String,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub input_cost_per_mtok: Money,
    pub output_cost_per_mtok: Money,
    pub context_limit: u64,
    pub output_limit: u64,
    pub max_cost: Money,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveSmokeStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSmokeReport {
    pub status: LiveSmokeStatus,
    pub detail: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: i64,
    pub ledger_verified: bool,
    pub handoff_verified: bool,
}

/// Validates foundation wiring without calling a live LLM provider.
pub async fn run_foundation_smoke(
    root: impl Into<PathBuf>,
    profile: &str,
) -> Result<SmokeReport, AdeError> {
    let root = root.into();
    let mut checks = Vec::new();

    checks.push(check(
        "agents_md",
        root.join("AGENTS.md").is_file(),
        "AGENTS.md present",
    ));
    checks.push(check(
        "recipe_catalog",
        canonical_recipe_ids().len() == 13,
        &format!("{} canonical recipes", canonical_recipe_ids().len()),
    ));

    let authority = AuthorityEnforcer::load(&root, Vec::<String>::new());
    checks.push(match &authority {
        Ok(_) => check("authority_load", true, "runtime authority loaded"),
        Err(error) => check("authority_load", false, &error.to_string()),
    });

    if let Ok(policy) = &authority {
        let allowed = policy.authorize_tool(
            "fs",
            "read_file",
            &serde_json::json!({ "path": "AGENTS.md" }),
        );
        checks.push(match allowed {
            Ok(_) => check("authority_read", true, "read_file allowed"),
            Err(error) => check("authority_read", false, &error.to_string()),
        });
        let denied = policy.authorize_tool(
            "fs",
            "mystery_tool",
            &serde_json::json!({ "payload": "no-path" }),
        );
        checks.push(match denied {
            Err(_) => check(
                "authority_deny_unknown",
                true,
                "unknown tool denied without human approval",
            ),
            Ok(_) => check(
                "authority_deny_unknown",
                false,
                "unknown tool was allowed without approval",
            ),
        });
    }

    let ignore = IgnoreEnforcer::new(&root).check_alignment();
    let ignore_ok = !ignore.is_empty();
    checks.push(check(
        "ignore_alignment",
        ignore_ok,
        &format!("{} surfaces checked", ignore.len()),
    ));

    let spend_ok = async {
        let database = ade_db::repo::AdeDatabase::open_path(":memory:").await?;
        let guard = SpendGuard::new(
            &root,
            Uuid::new_v4(),
            SpendCaps {
                session: Money::from_usd_str("1.0")?,
                daily: Money::from_usd_str("10.0")?,
            },
            UsageLedgerStore::new(database.connect()?),
        );
        guard.reserve(Money::ZERO, "smoke", "smoke-model").await?;
        Ok::<(), AdeError>(())
    }
    .await;
    checks.push(match spend_ok {
        Ok(()) => check(
            "spend_caps",
            true,
            "session/daily caps authorize $0 reservation",
        ),
        Err(error) => check("spend_caps", false, &error.to_string()),
    });

    let vault = ade_db::secrets::SecretsVault::for_profile(profile)?;
    let key_present = vault.contains("openai").unwrap_or(false);
    checks.push(check(
        "byok_openai",
        true,
        if key_present {
            "openai key configured (live provider smoke available)"
        } else {
            "openai key not configured — run `ade keys set openai` for live provider smoke"
        },
    ));

    let handoff = crate::handoff::HandoffManager::new(&root).load_latest();
    checks.push(match handoff {
        Ok(capsule) => check(
            "handoff_summary",
            capsule.prompt_summary(200).contains("next_safe_command"),
            "latest handoff summary includes next_safe_command",
        ),
        Err(_) => check(
            "handoff_summary",
            true,
            "no handoff yet (ok for fresh workspace)",
        ),
    });

    let ok = checks.iter().all(|item| item.ok);
    Ok(SmokeReport { ok, checks })
}

/// Runs one explicitly requested, tightly capped live turn through the shared service.
///
/// Missing credentials are a safe skip. Pricing is mandatory so the caller's
/// maximum cost can be enforced before any provider request is made.
pub async fn run_live_agent_smoke(spec: LiveSmokeSpec) -> Result<LiveSmokeReport, AdeError> {
    run_live_agent_smoke_with_vault(spec, Arc::new(NativeProviderKeyVault)).await
}

pub async fn run_live_agent_smoke_with_vault(
    spec: LiveSmokeSpec,
    vault: Arc<dyn ProviderKeyVault>,
) -> Result<LiveSmokeReport, AdeError> {
    if !vault.contains(&spec.profile, &spec.provider)? {
        return Ok(LiveSmokeReport {
            status: LiveSmokeStatus::Skipped,
            detail: format!(
                "{} credential is absent for profile {}; no network request was made",
                spec.provider, spec.profile
            ),
            input_tokens: 0,
            output_tokens: 0,
            cost_micros: 0,
            ledger_verified: false,
            handoff_verified: false,
        });
    }
    if spec.input_cost_per_mtok == Money::ZERO && spec.output_cost_per_mtok == Money::ZERO {
        return Err(AdeError::Spend(
            "live smoke requires provider input/output pricing so its cost cap is enforceable"
                .into(),
        ));
    }

    let model = crate::provider::ModelConfig {
        id: spec.model.clone(),
        name: spec.model.clone(),
        context_limit: spec.context_limit,
        output_limit: spec.output_limit,
        cost_per_input_mtok: spec.input_cost_per_mtok,
        cost_per_output_mtok: spec.output_cost_per_mtok,
    };
    let estimate = model.max_round_cost()?;
    if estimate > spec.max_cost {
        return Err(AdeError::Spend(format!(
            "live smoke worst-case cost ${} exceeds --max-cost-usd ${}",
            estimate.format_usd(),
            spec.max_cost.format_usd()
        )));
    }

    let daily_cap = SpendCaps::from_env().daily;
    let workspace_root = spec.workspace_root.clone();
    let workspace_display = workspace_root.display().to_string();
    let provider = spec.provider.clone();
    let model_id = spec.model.clone();
    let database = AdeDatabase::open(&DbConfig::from_ade_config(
        &ade_core::config::AdeConfig::load()?,
    ))
    .await?;
    let ledger = UsageLedgerStore::new(database.connect()?);
    let service = AgentTurnBuilder::new(AgentTurnSpec {
        prompt: "Reply with exactly ADE_SMOKE_OK and no other text. Do not call tools.".into(),
        provider: spec.provider,
        base_url: spec.base_url,
        model: spec.model,
        input_cost_per_mtok: spec.input_cost_per_mtok,
        output_cost_per_mtok: spec.output_cost_per_mtok,
        context_limit: spec.context_limit,
        output_limit: spec.output_limit,
        profile: spec.profile,
        workspace_root: spec.workspace_root,
        owned_paths: vec![],
        handoff_chars: 1_500,
    })
    .spend_caps(SpendCaps {
        session: spec.max_cost,
        daily: daily_cap,
    })
    .ledger(ledger.clone())
    .key_vault(vault)
    .prepare()
    .await?;

    let mut events = service.start();
    while let Some(event) = events.recv().await {
        match event {
            AgentEvent::Completed { result } => {
                let sentinel_ok = result.text.trim() == "ADE_SMOKE_OK";
                let ledger_verified = ledger
                    .has_committed_entry(result.session_id, "turn_summary", &workspace_display)
                    .await?;
                let capsule = crate::handoff::HandoffManager::new(&workspace_root).load_latest()?;
                let handoff_verified = capsule.session_id.as_deref()
                    == Some(result.session_id.to_string().as_str())
                    && capsule.provider.as_deref() == Some(provider.as_str())
                    && capsule.model.as_deref() == Some(model_id.as_str())
                    && capsule.turn_status.as_deref() == Some("completed")
                    && capsule.next_safe_command.is_some();
                let passed = sentinel_ok && ledger_verified && handoff_verified;
                return Ok(LiveSmokeReport {
                    status: if passed {
                        LiveSmokeStatus::Passed
                    } else {
                        LiveSmokeStatus::Failed
                    },
                    detail: if passed {
                        "provider sentinel, committed ledger entry, and redacted handoff verified"
                            .into()
                    } else if !sentinel_ok {
                        "provider responded, but did not return the exact ADE_SMOKE_OK sentinel"
                            .into()
                    } else if !ledger_verified {
                        "provider passed, but no committed turn-summary ledger entry was found"
                            .into()
                    } else {
                        "provider and ledger passed, but persisted handoff metadata was incomplete"
                            .into()
                    },
                    input_tokens: result.usage.input_tokens,
                    output_tokens: result.usage.output_tokens,
                    cost_micros: result.cost_micros,
                    ledger_verified,
                    handoff_verified,
                });
            }
            AgentEvent::Failed { error } | AgentEvent::Cancelled { reason: error } => {
                return Ok(LiveSmokeReport {
                    status: LiveSmokeStatus::Failed,
                    detail: error,
                    input_tokens: 0,
                    output_tokens: 0,
                    cost_micros: 0,
                    ledger_verified: false,
                    handoff_verified: false,
                });
            }
            _ => {}
        }
    }

    Ok(LiveSmokeReport {
        status: LiveSmokeStatus::Failed,
        detail: "provider event stream ended without a completion".into(),
        input_tokens: 0,
        output_tokens: 0,
        cost_micros: 0,
        ledger_verified: false,
        handoff_verified: false,
    })
}

fn check(name: &str, ok: bool, detail: &str) -> SmokeCheck {
    SmokeCheck {
        name: name.into(),
        ok,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ade_db::secrets::InMemoryProviderKeyVault;

    fn smoke_spec() -> LiveSmokeSpec {
        LiveSmokeSpec {
            workspace_root: PathBuf::from("."),
            profile: "local".into(),
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "test-model".into(),
            input_cost_per_mtok: Money::from_usd_str("1").unwrap(),
            output_cost_per_mtok: Money::from_usd_str("1").unwrap(),
            context_limit: 1_000,
            output_limit: 16,
            max_cost: Money::from_usd_str("0.01").unwrap(),
        }
    }

    #[tokio::test]
    async fn live_smoke_skips_without_a_credential() {
        let report = run_live_agent_smoke_with_vault(
            smoke_spec(),
            Arc::new(InMemoryProviderKeyVault::default()),
        )
        .await
        .unwrap();

        assert_eq!(report.status, LiveSmokeStatus::Skipped);
        assert_eq!(report.cost_micros, 0);
    }

    #[tokio::test]
    async fn live_smoke_rejects_estimate_over_cap_before_network() {
        let vault = InMemoryProviderKeyVault::default();
        vault.set("local", "openai", "not-a-real-key").unwrap();
        let mut spec = smoke_spec();
        spec.max_cost = Money::from_micros(1);

        let error = run_live_agent_smoke_with_vault(spec, Arc::new(vault))
            .await
            .unwrap_err();

        assert!(matches!(error, AdeError::Spend(_)));
        assert!(error.to_string().contains("worst-case cost"));
    }
}
