//! Compaction / fertility gold helpers (C5).
//!
//! Measures harness channel efficiency with the same chars÷4 heuristic used
//! elsewhere for *assembly* estimates — never for billing. Stops cargo-cult
//! “compression ratios” without fidelity checks.

use crate::context_edit::{
    apply_boundary_compact, estimate_messages_tokens, mask_stale_tool_results,
    BoundaryCompactExtras,
};
use serde_json::{json, Value};

/// Sample facts used across format fertility comparisons.
pub fn sample_facts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("goal", "Ship C5 fertility gold"),
        ("path", "crates/agents/src/fertility.rs"),
        ("verify", "G3"),
        ("next", "ade eval --gold"),
        ("status", "active"),
    ]
}

pub fn format_as_verbose_prose(facts: &[(&str, &str)]) -> String {
    let mut out = String::from(
        "Here is a detailed narrative description of the current engineering state for your careful review:\n",
    );
    for (k, v) in facts {
        out.push_str(&format!(
            "Please note that the field known as \"{k}\" currently has the value \"{v}\". "
        ));
    }
    out.push_str("Thank you for attending to each of these important details individually.");
    out
}

pub fn format_as_pretty_json(facts: &[(&str, &str)]) -> String {
    let mut map = serde_json::Map::new();
    for (k, v) in facts {
        map.insert((*k).into(), json!(v));
    }
    serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_default()
}

pub fn format_as_compact_json(facts: &[(&str, &str)]) -> String {
    let mut map = serde_json::Map::new();
    for (k, v) in facts {
        map.insert((*k).into(), json!(v));
    }
    serde_json::to_string(&Value::Object(map)).unwrap_or_default()
}

pub fn format_as_tsv(facts: &[(&str, &str)]) -> String {
    let mut out = String::from("key\tvalue\n");
    for (k, v) in facts {
        out.push_str(&format!("{k}\t{v}\n"));
    }
    out
}

pub fn estimate_text_tokens(text: &str) -> u64 {
    crate::context::estimate_tokens(text) as u64
}

#[derive(Debug, Clone)]
pub struct FertilityRanking {
    pub tsv: u64,
    pub compact_json: u64,
    pub pretty_json: u64,
    pub verbose_prose: u64,
}

impl FertilityRanking {
    pub fn measure(facts: &[(&str, &str)]) -> Self {
        Self {
            tsv: estimate_text_tokens(&format_as_tsv(facts)),
            compact_json: estimate_text_tokens(&format_as_compact_json(facts)),
            pretty_json: estimate_text_tokens(&format_as_pretty_json(facts)),
            verbose_prose: estimate_text_tokens(&format_as_verbose_prose(facts)),
        }
    }

    /// Saturated formats beat verbose prose; compact ≤ pretty.
    pub fn order_ok(&self) -> bool {
        self.compact_json <= self.pretty_json
            && self.pretty_json < self.verbose_prose
            && self.tsv < self.verbose_prose
    }
}

/// Build a bloated multi-round tool transcript for compaction benches.
pub fn bloated_tool_transcript(rounds: usize, blob_chars: usize) -> Vec<Value> {
    let mut messages = vec![
        json!({"role":"system","content":"sys"}),
        json!({"role":"user","content":"implement fertility gold"}),
    ];
    for i in 0..rounds {
        messages.push(json!({
            "role":"assistant",
            "content": format!("working on step {i}"),
            "tool_calls":[{
                "id": format!("c{i}"),
                "function":{
                    "name":"fs__read_file",
                    "arguments": format!("{{\"path\":\"src/f{i}.rs\"}}")
                }
            }]
        }));
        messages.push(json!({
            "role":"tool",
            "tool_call_id": format!("c{i}"),
            "content": "y".repeat(blob_chars)
        }));
    }
    messages
}

#[derive(Debug, Clone)]
pub struct CompactionBench {
    pub full_tokens: u64,
    pub masked_tokens: u64,
    pub capsule_tokens: u64,
    pub mask_saved_pct: f64,
    pub capsule_saved_pct: f64,
    pub mask_preserved_ids: bool,
    pub capsule_has_sections: bool,
}

impl CompactionBench {
    pub fn run(keep_rounds: usize) -> Self {
        let messages = bloated_tool_transcript(6, 1_500);
        let full = estimate_messages_tokens(&messages);
        let masked = mask_stale_tool_results(&messages, keep_rounds);
        let masked_tokens = estimate_messages_tokens(&masked);
        let (capsule_msgs, summary) = apply_boundary_compact(
            &messages,
            keep_rounds,
            "c5_bench",
            8_000,
            BoundaryCompactExtras {
                intent: Some("Ship C5 fertility gold"),
                decisions: &["mask before summarize".into()],
                paths: &["crates/agents/src/fertility.rs".into()],
                failing: None,
                next: Some("ade eval --gold"),
                verify: Some("G3"),
            },
        );
        let capsule_tokens = estimate_messages_tokens(&capsule_msgs);

        let mut preserved = true;
        for msg in &masked {
            if msg.get("role").and_then(Value::as_str) != Some("tool") {
                continue;
            }
            if msg.get("tool_call_id").and_then(Value::as_str).is_none() {
                preserved = false;
            }
        }
        // Last keep_rounds tool contents stay verbatim (not the cleared stub).
        let last_tool = masked
            .iter()
            .rev()
            .find(|m| m.get("role").and_then(Value::as_str) == Some("tool"));
        if let Some(tool) = last_tool {
            let content = tool.get("content").and_then(Value::as_str).unwrap_or("");
            if content.contains("ade.context-edit") {
                preserved = false;
            }
        }

        let sections_ok = summary.summary.contains("ade.boundary-capsule/v1")
            && summary.summary.contains("intent:")
            && summary.summary.contains("paths:")
            && summary.summary.contains("next:")
            && summary.summary.contains("verify:");

        let mask_saved = if full == 0 {
            0.0
        } else {
            (full.saturating_sub(masked_tokens) as f64 / full as f64) * 100.0
        };
        let capsule_saved = if full == 0 {
            0.0
        } else {
            (full.saturating_sub(capsule_tokens) as f64 / full as f64) * 100.0
        };

        Self {
            full_tokens: full,
            masked_tokens,
            capsule_tokens,
            mask_saved_pct: mask_saved,
            capsule_saved_pct: capsule_saved,
            mask_preserved_ids: preserved,
            capsule_has_sections: sections_ok,
        }
    }
}

/// Invented opaque encodings lose fertility vs compact saturated formats.
pub fn invented_opaque_loses_to_compact_json(facts: &[(&str, &str)]) -> bool {
    let compact = format_as_compact_json(facts);
    let mut opaque = String::from("⟦ADE_CIPHER_v0⟧");
    for (k, v) in facts {
        opaque.push_str(&format!("⟨{k}⟩⟪{v}⟫✧"));
    }
    estimate_text_tokens(&opaque) > estimate_text_tokens(&compact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fertility_order_saturated_formats() {
        let rank = FertilityRanking::measure(&sample_facts());
        assert!(
            rank.order_ok(),
            "tsv={} compact={} pretty={} prose={}",
            rank.tsv,
            rank.compact_json,
            rank.pretty_json,
            rank.verbose_prose
        );
    }

    #[test]
    fn compaction_bench_saves_and_keeps_fidelity() {
        let bench = CompactionBench::run(2);
        assert!(bench.masked_tokens < bench.full_tokens);
        assert!(bench.capsule_tokens < bench.full_tokens);
        assert!(bench.mask_saved_pct > 10.0);
        assert!(bench.capsule_saved_pct > 20.0);
        assert!(bench.mask_preserved_ids);
        assert!(bench.capsule_has_sections);
    }

    #[test]
    fn inventing_cipher_is_not_a_win() {
        assert!(invented_opaque_loses_to_compact_json(&sample_facts()));
    }
}
