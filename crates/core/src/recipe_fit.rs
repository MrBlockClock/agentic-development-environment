//! Stack Fit — deterministic ranking of trust-contract recipes.

use crate::recipe::{RecipeEra, RecipeG5, StackRecipe};
use serde::{Deserialize, Serialize};

/// Interview answers (empty / `any` = wildcard).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FitAnswers {
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub primary_runtime: String,
    #[serde(default)]
    pub ui_surface: String,
    #[serde(default)]
    pub evidence: String,
    #[serde(default)]
    pub compliance: String,
    #[serde(default)]
    pub repo_state: String,
    #[serde(default)]
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScoredRecipe {
    pub id: String,
    pub name: String,
    pub score: i32,
    pub why: Vec<String>,
    pub era: RecipeEra,
    pub domain: String,
}

fn norm(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn is_wild(value: &str) -> bool {
    let v = norm(value);
    v.is_empty() || v == "any"
}

fn list_has(list: &[String], needle: &str) -> bool {
    let n = norm(needle);
    list.iter().any(|item| {
        let i = norm(item);
        i == n || i == "any"
    })
}

fn g5_evidence_tokens(g5: &RecipeG5) -> &'static [&'static str] {
    match g5 {
        RecipeG5::Playwright => &["playwright"],
        RecipeG5::HttpContract => &["http"],
        RecipeG5::BinarySmoke | RecipeG5::InstallSmoke => &["binary"],
        RecipeG5::DeviceChecklist => &["device"],
        RecipeG5::HardwareSignoff => &["hil"],
        RecipeG5::PlanChecklist => &["plan"],
        RecipeG5::PlaytestChecklist
        | RecipeG5::ReproducibilityNote
        | RecipeG5::UpstreamTests
        | RecipeG5::ParityProbes
        | RecipeG5::None => &["any"],
    }
}

fn score_list_match(
    answer: &str,
    hints: &[String],
    match_pts: i32,
    mismatch_pts: i32,
    why_match: &str,
    why: &mut Vec<String>,
) -> i32 {
    if is_wild(answer) {
        return 0;
    }
    if hints.is_empty() {
        return 0;
    }
    if list_has(hints, answer) {
        why.push(why_match.into());
        match_pts
    } else {
        mismatch_pts
    }
}

/// Rank recipes for the given answers. Never filters the catalog — only sorts.
pub fn rank_recipes(answers: &FitAnswers, recipes: &[StackRecipe]) -> Vec<ScoredRecipe> {
    let mut scored: Vec<(usize, ScoredRecipe)> = recipes
        .iter()
        .enumerate()
        .filter(|(_, r)| r.id != "node-web")
        .map(|(idx, recipe)| {
            let mut score: i32 = 0;
            let mut why = Vec::new();

            score += score_list_match(
                &answers.intent,
                &recipe.fit.intents,
                12,
                -4,
                "Matches project intent",
                &mut why,
            );
            score += score_list_match(
                &answers.primary_runtime,
                &recipe.fit.runtimes,
                18,
                -10,
                "Matches primary runtime",
                &mut why,
            );
            score += score_list_match(
                &answers.ui_surface,
                &recipe.fit.ui_surfaces,
                14,
                -8,
                "Matches UI surface",
                &mut why,
            );

            let evidence = norm(&answers.evidence);
            if !is_wild(&evidence) {
                let tokens = g5_evidence_tokens(&recipe.g5);
                let hint_ok = list_has(&recipe.fit.evidence, &evidence)
                    || tokens
                        .iter()
                        .any(|t| *t == evidence.as_str() || *t == "any");
                if hint_ok {
                    score += 20;
                    why.push("Evidence story aligns with G5".into());
                } else {
                    score -= 22;
                }
            }

            let compliance = norm(&answers.compliance);
            if compliance == "regulated" {
                if list_has(&recipe.fit.compliance, "regulated") || recipe.id.contains("regulated")
                {
                    score += 28;
                    why.push("Built for regulated / compliance workflows".into());
                } else if recipe.id.contains("saas") || recipe.domain == "saas" {
                    score -= 12;
                }
            } else if compliance == "none"
                && list_has(&recipe.fit.compliance, "regulated")
                && !is_wild(&answers.compliance)
            {
                score -= 6;
            }

            score += score_list_match(
                &answers.host,
                &recipe.fit.hosts,
                4,
                -1,
                "Host-friendly recipe",
                &mut why,
            );

            let repo = norm(&answers.repo_state);
            if repo == "existing" && (recipe.id == "oss-fork-maintainer" || recipe.domain == "oss")
            {
                score += 8;
                why.push("Good for existing / fork maintenance".into());
            }
            if repo == "empty" && recipe.id == "ade-plan-heavy" {
                score -= 4;
            }

            if why.is_empty() {
                why.push(format!("{} · {}", recipe.domain, era_label(&recipe.era)));
            }

            (
                idx,
                ScoredRecipe {
                    id: recipe.id.clone(),
                    name: recipe.name.clone(),
                    score,
                    why,
                    era: recipe.era.clone(),
                    domain: recipe.domain.clone(),
                },
            )
        })
        .collect();

    scored.sort_by(|a, b| b.1.score.cmp(&a.1.score).then_with(|| a.0.cmp(&b.0)));
    scored.into_iter().map(|(_, s)| s).collect()
}

fn era_label(era: &RecipeEra) -> &'static str {
    match era {
        RecipeEra::Classic => "classic",
        RecipeEra::Modern => "modern",
        RecipeEra::Frontier => "frontier",
    }
}

/// Convenience: rank the built-in catalog.
pub fn rank_builtin_recipes(answers: &FitAnswers) -> Vec<ScoredRecipe> {
    let recipes = crate::recipe::builtin_recipes();
    rank_recipes(answers, &recipes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::builtin_recipes;

    #[test]
    fn regulated_web_prefers_business_regulated() {
        let answers = FitAnswers {
            intent: "product".into(),
            primary_runtime: "mixed".into(),
            ui_surface: "web".into(),
            evidence: "playwright".into(),
            compliance: "regulated".into(),
            ..Default::default()
        };
        let ranked = rank_recipes(&answers, &builtin_recipes());
        assert_eq!(ranked[0].id, "business-regulated");
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn rust_api_http_prefers_rust_api_turso() {
        let answers = FitAnswers {
            intent: "product".into(),
            primary_runtime: "rust".into(),
            ui_surface: "none".into(),
            evidence: "http".into(),
            compliance: "none".into(),
            ..Default::default()
        };
        let ranked = rank_recipes(&answers, &builtin_recipes());
        assert_eq!(ranked[0].id, "rust-api-turso");
    }

    #[test]
    fn never_drops_canonical_recipes() {
        let ranked = rank_builtin_recipes(&FitAnswers::default());
        assert_eq!(ranked.len(), 13);
    }
}
