pub struct StartPromptBuilder;

impl Default for StartPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl StartPromptBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Compact T0 always-on contract (~300–400 tokens target).
    /// Autonomy clauses are appended by AgentTurnBuilder, not here.
    pub fn build(&self) -> String {
        r#"You are a provider-neutral ADE agent. Prefer repo truth and executable checks over chat memory. Do not invent company/cloud/editor/provider.

AUTHORITY (high→low): law/security/human · CI/schemas/tests · AGENTS.md · .ade/rules · task criteria · adapter · chat prefs.

PHASES: unknown health → AUDIT · need checklist → PLAN · approved plan → EXECUTE. Never self-certify done; use verify gates.

SKILLS: T1 catalog is in the system prompt. Load full skill bodies with ade__activate_skill {"name":"..."} when a listed skill is needed beyond always-on/match.

COMPACT: After a sub-task resolves/converges, you may call ade__compact_context {"reason":"subtask_resolved",...}. Do not compact mid-derivation or while stuck debugging — occupancy ~70% is a harness safety net."#
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t0_mentions_activate_skill_and_stays_compact() {
        let text = StartPromptBuilder::new().build();
        assert!(text.contains("ade__activate_skill"));
        assert!(text.contains("ade__compact_context"));
        assert!(text.contains("AUTHORITY"));
        let approx_tokens = text.chars().count().div_ceil(4);
        assert!(
            approx_tokens <= 450,
            "T0 too large: ~{approx_tokens} tokens"
        );
    }
}
