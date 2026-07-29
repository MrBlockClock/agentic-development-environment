use ade_agents::spend::{SpendCaps, SpendPeriod};
use ade_core::config::AdeConfig;
use ade_core::money::Money;
use ade_db::repo::{AdeDatabase, DbConfig};
use ade_db::usage_ledger::{LedgerEntryView, UsageLedgerStore};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::router::{ApiError, ApiResult, ApiState};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/ledger", get(spend_ledger))
        .route("/summary", get(spend_summary))
}

#[derive(Debug, Deserialize)]
pub struct LedgerQuery {
    pub limit: Option<u32>,
}

async fn spend_ledger(
    State(state): State<ApiState>,
    Query(query): Query<LedgerQuery>,
) -> ApiResult<Vec<LedgerEntryView>> {
    let workspace = state.workspace_root().display().to_string();
    let ledger = open_ledger().await?;
    ledger
        .recent_for_workspace(&workspace, query.limit.unwrap_or(40))
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[derive(Debug, Deserialize)]
pub struct SummaryQuery {
    pub session_cap_usd: Option<f64>,
    pub daily_cap_usd: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct SpendSummary {
    pub daily_usd: f64,
    pub used_usd: f64,
    pub reserved_usd: f64,
    pub remaining_usd: f64,
    pub daily_cap_usd: f64,
    pub session_cap_usd: f64,
    pub period_key: String,
}

async fn spend_summary(
    State(state): State<ApiState>,
    Query(query): Query<SummaryQuery>,
) -> ApiResult<SpendSummary> {
    let workspace = state.workspace_root().display().to_string();
    let caps = SpendCaps {
        session: Money::try_from_usd_f64(query.session_cap_usd.unwrap_or(1.0))
            .map_err(|error| ApiError::bad_request(error.to_string()))?,
        daily: Money::try_from_usd_f64(query.daily_cap_usd.unwrap_or(10.0))
            .map_err(|error| ApiError::bad_request(error.to_string()))?,
    };
    let period_key = SpendPeriod::Day.key(uuid::Uuid::nil());
    let ledger = open_ledger().await?;
    let breakdown = ledger
        .active_spend_breakdown("workspace", &period_key, &workspace)
        .await
        .map_err(ApiError::internal)?;
    let remaining = caps.daily.saturating_sub(breakdown.active);
    Ok(Json(SpendSummary {
        daily_usd: breakdown.active.to_usd_f64(),
        used_usd: breakdown.used.to_usd_f64(),
        reserved_usd: breakdown.reserved.to_usd_f64(),
        remaining_usd: remaining.to_usd_f64(),
        daily_cap_usd: caps.daily.to_usd_f64(),
        session_cap_usd: caps.session.to_usd_f64(),
        period_key,
    }))
}

async fn open_ledger() -> Result<UsageLedgerStore, ApiError> {
    let config = AdeConfig::load().map_err(ApiError::internal)?;
    let db = AdeDatabase::open(&DbConfig::from_ade_config(&config))
        .await
        .map_err(ApiError::internal)?;
    let conn = db.connect().map_err(ApiError::internal)?;
    Ok(UsageLedgerStore::new(conn))
}
