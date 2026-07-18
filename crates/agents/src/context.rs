use serde::{Deserialize, Serialize};

/// Soft token budgets for assembling the always-on system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub always_on_tokens: u32,
    pub rules_tokens: u32,
    pub handoff_tokens: u32,
    pub skills_tokens: u32,
    pub mcp_servers: u32,
}

impl ContextBudget {
    pub fn default_daily() -> Self {
        Self {
            always_on_tokens: 400,
            rules_tokens: 2_400,
            handoff_tokens: 500,
            skills_tokens: 6_000,
            mcp_servers: 2,
        }
    }

    pub fn total_prompt_allowance(&self) -> u32 {
        self.always_on_tokens
            .saturating_add(self.rules_tokens)
            .saturating_add(self.handoff_tokens)
    }

    pub fn check_usage(&self, used_tokens: u32) -> ContextStatus {
        let allowance = self.total_prompt_allowance();
        if used_tokens > allowance {
            ContextStatus::Critical
        } else if used_tokens > (allowance * 85) / 100 {
            ContextStatus::Warning
        } else {
            ContextStatus::Green
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStatus {
    Green,
    Warning,
    Critical,
}

/// Rough token estimate — ~4 UTF-8 chars per token for English/code prompts.
pub fn estimate_tokens(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    chars.div_ceil(4).max(if text.is_empty() { 0 } else { 1 })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSection {
    pub name: String,
    pub tokens: u32,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssembledPrompt {
    pub text: String,
    pub tokens_estimated: u32,
    pub status: ContextStatus,
    pub sections: Vec<PromptSection>,
}

/// Builds the system prompt under measured context budgets.
#[derive(Debug, Clone)]
pub struct PromptAssembler {
    budget: ContextBudget,
    model_context_limit: u64,
}

impl PromptAssembler {
    pub fn new(budget: ContextBudget, model_context_limit: u64) -> Self {
        Self {
            budget,
            model_context_limit,
        }
    }

    pub fn daily(model_context_limit: u64) -> Self {
        Self::new(ContextBudget::default_daily(), model_context_limit)
    }

    pub fn budget(&self) -> &ContextBudget {
        &self.budget
    }

    pub fn assemble(
        &self,
        start_prompt: &str,
        authority_context: &str,
        handoff_summary: Option<&str>,
    ) -> AssembledPrompt {
        let mut sections = Vec::new();
        let mut parts = Vec::new();

        let (start, start_truncated) =
            truncate_to_tokens(start_prompt, self.budget.always_on_tokens);
        sections.push(PromptSection {
            name: "start".into(),
            tokens: estimate_tokens(&start),
            truncated: start_truncated,
        });
        parts.push(start);

        let (authority, authority_truncated) =
            truncate_to_tokens(authority_context, self.budget.rules_tokens);
        sections.push(PromptSection {
            name: "authority".into(),
            tokens: estimate_tokens(&authority),
            truncated: authority_truncated,
        });
        parts.push(authority);

        if let Some(handoff) = handoff_summary.filter(|text| !text.trim().is_empty()) {
            let (summary, handoff_truncated) =
                truncate_to_tokens(handoff, self.budget.handoff_tokens);
            sections.push(PromptSection {
                name: "handoff".into(),
                tokens: estimate_tokens(&summary),
                truncated: handoff_truncated,
            });
            parts.push(summary);
        }

        let text = parts.join("\n\n");
        let mut tokens_estimated = estimate_tokens(&text);
        // Keep assembled prompt inside both soft budget and model context window.
        let hard_cap = if self.model_context_limit == 0 {
            self.budget.total_prompt_allowance()
        } else {
            (self.model_context_limit as u32)
                .min(self.budget.total_prompt_allowance())
                .max(64)
        };
        let mut truncated_for_model = false;
        let text = if tokens_estimated > hard_cap {
            truncated_for_model = true;
            let (clipped, _) = truncate_to_tokens(&text, hard_cap);
            tokens_estimated = estimate_tokens(&clipped);
            clipped
        } else {
            text
        };
        if truncated_for_model {
            sections.push(PromptSection {
                name: "model_cap".into(),
                tokens: tokens_estimated,
                truncated: true,
            });
        }

        AssembledPrompt {
            status: self.budget.check_usage(tokens_estimated),
            text,
            tokens_estimated,
            sections,
        }
    }
}

fn truncate_to_tokens(text: &str, max_tokens: u32) -> (String, bool) {
    if max_tokens == 0 {
        return (String::new(), !text.is_empty());
    }
    let max_chars = (max_tokens as usize).saturating_mul(4);
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }
    let clipped: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    (format!("{clipped}..."), true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_and_truncates_under_budget() {
        let assembler = PromptAssembler::daily(8_000);
        let huge_rules = "rule\n".repeat(5_000);
        let assembled = assembler.assemble("start prompt", &huge_rules, Some("handoff body"));
        assert!(
            assembled.tokens_estimated <= ContextBudget::default_daily().total_prompt_allowance()
        );
        assert!(assembled
            .sections
            .iter()
            .any(|section| section.name == "authority" && section.truncated));
        assert!(
            !assembled.text.contains("rule\n".repeat(100).as_str())
                || assembled.sections.iter().any(|s| s.truncated)
        );
    }

    #[test]
    fn status_moves_to_warning_near_cap() {
        let budget = ContextBudget {
            always_on_tokens: 10,
            rules_tokens: 10,
            handoff_tokens: 10,
            skills_tokens: 0,
            mcp_servers: 0,
        };
        assert_eq!(budget.check_usage(5), ContextStatus::Green);
        assert_eq!(budget.check_usage(27), ContextStatus::Warning);
        assert_eq!(budget.check_usage(40), ContextStatus::Critical);
    }
}
