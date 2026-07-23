//! Context editing helpers — truncate oversized tool blobs + mask stale rounds (C1)
//! + boundary / ~70% occupancy capsules (C2).

use serde_json::{json, Value};
use std::collections::HashSet;

/// Default max characters kept in the *model* tool message (UI may keep fuller text).
pub const DEFAULT_TOOL_RESULT_MAX_CHARS: usize = 12_000;

/// Default number of recent tool-rounds whose results stay verbatim in the model window.
pub const DEFAULT_TOOL_RESULT_KEEP_ROUNDS: usize = 2;

/// Safety-net occupancy before harness forces a structured boundary capsule (C2).
pub const DEFAULT_COMPACT_OCCUPANCY: f64 = 0.70;

/// Truncate a tool result for the growing chat transcript.
///
/// Returns `(content_for_model, was_truncated)`.
pub fn compact_tool_result_for_context(text: &str, max_chars: usize) -> (String, bool) {
    let max_chars = max_chars.max(256);
    let len = text.chars().count();
    if len <= max_chars {
        return (text.to_string(), false);
    }
    let keep = max_chars.saturating_sub(96);
    let head: String = text.chars().take(keep).collect();
    (
        format!(
            "{head}\n\n[ade.context-edit: truncated to {keep} of {len} chars; re-run tool if you need the tail]"
        ),
        true,
    )
}

pub fn tool_result_max_chars_from_env() -> usize {
    std::env::var("ADE_TOOL_RESULT_MAX_CHARS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|n: &usize| *n >= 256)
        .unwrap_or(DEFAULT_TOOL_RESULT_MAX_CHARS)
}

pub fn tool_result_keep_rounds_from_env() -> usize {
    std::env::var("ADE_TOOL_RESULT_KEEP_ROUNDS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(DEFAULT_TOOL_RESULT_KEEP_ROUNDS)
        .max(1)
}

pub fn compact_occupancy_from_env() -> f64 {
    std::env::var("ADE_COMPACT_OCCUPANCY")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|n: &f64| *n > 0.4 && *n < 0.95)
        .unwrap_or(DEFAULT_COMPACT_OCCUPANCY)
}

/// Estimate tokens for a message list (chars÷4 heuristic — assembly only, not billing).
pub fn estimate_messages_tokens(messages: &[Value]) -> u64 {
    let encoded = serde_json::to_string(messages).unwrap_or_default();
    crate::context::estimate_tokens(&encoded) as u64
}

pub fn occupancy_ratio(messages: &[Value], context_limit: u64) -> f64 {
    let limit = context_limit.max(1) as f64;
    estimate_messages_tokens(messages) as f64 / limit
}

pub fn should_compact_at_occupancy(messages: &[Value], context_limit: u64, threshold: f64) -> bool {
    if context_limit == 0 {
        return false;
    }
    occupancy_ratio(messages, context_limit) >= threshold
}

/// Structured thrift block written into the model window after a boundary compact.
#[derive(Debug, Clone)]
pub struct BoundaryCapsuleSummary {
    pub trigger: String,
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub occupancy_before: f64,
    pub summary: String,
}

/// Collapse older transcript into a structured capsule message; keep system + last K tool rounds.
///
/// Sections: intent · decisions · paths · failing · next · verify (Tokenomics C2).
pub fn apply_boundary_compact(
    messages: &[Value],
    keep_rounds: usize,
    trigger: &str,
    context_limit: u64,
    extras: BoundaryCompactExtras<'_>,
) -> (Vec<Value>, BoundaryCapsuleSummary) {
    let tokens_before = estimate_messages_tokens(messages);
    let occupancy_before = if context_limit == 0 {
        0.0
    } else {
        tokens_before as f64 / context_limit as f64
    };

    let system = messages
        .iter()
        .find(|m| m.get("role").and_then(Value::as_str) == Some("system"))
        .cloned()
        .unwrap_or_else(|| json!({"role":"system","content":""}));

    let recent = last_tool_rounds(messages, keep_rounds.max(1));
    let summary = format_boundary_summary(messages, trigger, extras);
    let mut next = vec![
        system,
        json!({
            "role": "user",
            "content": summary.clone(),
        }),
    ];
    next.extend(recent);
    let tokens_after = estimate_messages_tokens(&next);
    (
        next,
        BoundaryCapsuleSummary {
            trigger: trigger.into(),
            tokens_before,
            tokens_after,
            occupancy_before,
            summary,
        },
    )
}

#[derive(Debug, Clone, Default)]
pub struct BoundaryCompactExtras<'a> {
    pub intent: Option<&'a str>,
    pub decisions: &'a [String],
    pub paths: &'a [String],
    pub failing: Option<&'a str>,
    pub next: Option<&'a str>,
    pub verify: Option<&'a str>,
}

fn format_boundary_summary(
    messages: &[Value],
    trigger: &str,
    extras: BoundaryCompactExtras<'_>,
) -> String {
    let intent = extras
        .intent
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("continue current eng-goal under thrift Continuity")
        .to_string();
    let decisions = if extras.decisions.is_empty() {
        infer_decision_hints(messages)
    } else {
        extras.decisions.to_vec()
    };
    let paths = if extras.paths.is_empty() {
        infer_path_hints(messages)
    } else {
        extras.paths.to_vec()
    };
    let failing = extras.failing.unwrap_or("(none noted)").trim();
    let next = extras
        .next
        .unwrap_or("Resume from this capsule; re-run tools for cleared blobs.")
        .trim();
    let verify = extras.verify.unwrap_or("(unchanged)").trim();

    format!(
        "ade.boundary-capsule/v1\n\
trigger: {trigger}\n\
intent: {intent}\n\
decisions:\n{}\n\
paths:\n{}\n\
failing: {failing}\n\
next: {next}\n\
verify: {verify}\n\
note: Earlier tool blobs were cleared; durable facts live under .ade/continuity + handoff.",
        bullet_lines(&decisions),
        bullet_lines(&paths),
    )
}

fn bullet_lines(items: &[String]) -> String {
    if items.is_empty() {
        return "- (none)".into();
    }
    items
        .iter()
        .take(12)
        .map(|s| format!("- {s}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn infer_decision_hints(messages: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for msg in messages.iter().rev().take(24) {
        if msg.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let text = msg.get("content").and_then(Value::as_str).unwrap_or("");
        for line in text.lines().take(3) {
            let t = line.trim();
            if t.len() > 12 && t.len() < 160 {
                out.push(t.to_string());
            }
            if out.len() >= 4 {
                return out;
            }
        }
    }
    out
}

fn infer_path_hints(messages: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for msg in messages {
        if let Some(calls) = msg.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                if let Some(raw) = call
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(Value::as_str)
                {
                    if let Ok(v) = serde_json::from_str::<Value>(raw) {
                        if let Some(p) = v.get("path").and_then(Value::as_str) {
                            let p = p.trim();
                            if !p.is_empty() && !out.iter().any(|x| x == p) {
                                out.push(p.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn last_tool_rounds(messages: &[Value], keep_rounds: usize) -> Vec<Value> {
    let mut round_starts: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let role = messages[i]
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("");
        let has_calls = messages[i]
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if role == "assistant" && has_calls {
            round_starts.push(i);
            let mut j = i + 1;
            while j < messages.len()
                && messages[j].get("role").and_then(Value::as_str) == Some("tool")
            {
                j += 1;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    if round_starts.is_empty() {
        return Vec::new();
    }
    let start_idx = if round_starts.len() <= keep_rounds {
        round_starts[0]
    } else {
        round_starts[round_starts.len() - keep_rounds]
    };
    messages[start_idx..].to_vec()
}

/// Stub tool results outside the last `keep_rounds` assistant+tool rounds.
///
/// Keeps system/user/assistant text; replaces older `role=tool` content with a
/// short re-fetch hint. Preserves `tool_call_id`.
pub fn mask_stale_tool_results(messages: &[Value], keep_rounds: usize) -> Vec<Value> {
    let keep_rounds = keep_rounds.max(1);
    let mut rounds: Vec<Vec<usize>> = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let role = messages[i]
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("");
        let has_calls = messages[i]
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if role == "assistant" && has_calls {
            let mut tool_idxs = Vec::new();
            let mut j = i + 1;
            while j < messages.len()
                && messages[j].get("role").and_then(Value::as_str) == Some("tool")
            {
                tool_idxs.push(j);
                j += 1;
            }
            if !tool_idxs.is_empty() {
                rounds.push(tool_idxs);
            }
            i = j;
            continue;
        }
        i += 1;
    }

    if rounds.len() <= keep_rounds {
        return messages.to_vec();
    }

    let mut mask: HashSet<usize> = HashSet::new();
    let drop_count = rounds.len() - keep_rounds;
    for tool_idxs in rounds.iter().take(drop_count) {
        for idx in tool_idxs {
            mask.insert(*idx);
        }
    }

    messages
        .iter()
        .enumerate()
        .map(|(idx, msg)| {
            if !mask.contains(&idx) {
                return msg.clone();
            }
            let tool_call_id = msg
                .get("tool_call_id")
                .cloned()
                .unwrap_or_else(|| json!(""));
            json!({
                "role": "tool",
                "tool_call_id": tool_call_id,
                "content": "[ade.context-edit: earlier tool result cleared to free context; re-run the tool if you still need the full output]"
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_results_pass_through() {
        let (out, truncated) = compact_tool_result_for_context("hello", 100);
        assert_eq!(out, "hello");
        assert!(!truncated);
    }

    #[test]
    fn long_results_truncate_with_marker() {
        let big = "x".repeat(500);
        let (out, truncated) = compact_tool_result_for_context(&big, 200);
        assert!(truncated);
        assert!(out.contains("ade.context-edit"));
        assert!(out.chars().count() < 500);
    }

    #[test]
    fn masks_old_tool_rounds_keeps_last_k() {
        let messages = vec![
            json!({"role":"system","content":"s"}),
            json!({"role":"user","content":"u"}),
            json!({"role":"assistant","tool_calls":[{"id":"1"}]}),
            json!({"role":"tool","tool_call_id":"1","content":"OLD_A"}),
            json!({"role":"assistant","tool_calls":[{"id":"2"}]}),
            json!({"role":"tool","tool_call_id":"2","content":"OLD_B"}),
            json!({"role":"assistant","tool_calls":[{"id":"3"}]}),
            json!({"role":"tool","tool_call_id":"3","content":"NEW_C"}),
            json!({"role":"assistant","tool_calls":[{"id":"4"}]}),
            json!({"role":"tool","tool_call_id":"4","content":"NEW_D"}),
        ];
        let masked = mask_stale_tool_results(&messages, 2);
        assert!(masked[3]["content"]
            .as_str()
            .unwrap()
            .contains("ade.context-edit"));
        assert!(masked[5]["content"]
            .as_str()
            .unwrap()
            .contains("ade.context-edit"));
        assert_eq!(masked[7]["content"].as_str().unwrap(), "NEW_C");
        assert_eq!(masked[9]["content"].as_str().unwrap(), "NEW_D");
        assert_eq!(masked[3]["tool_call_id"].as_str().unwrap(), "1");
    }

    #[test]
    fn boundary_compact_shrinks_and_keeps_structure() {
        let mut messages = vec![
            json!({"role":"system","content":"sys"}),
            json!({"role":"user","content":"do the thing"}),
        ];
        for i in 0..6 {
            messages.push(json!({"role":"assistant","tool_calls":[{"id": format!("{i}"), "function":{"name":"fs__read_file","arguments": format!("{{\"path\":\"src/a{i}.rs\"}}")}}]}));
            messages.push(
                json!({"role":"tool","tool_call_id": format!("{i}"), "content": "x".repeat(800)}),
            );
        }
        assert!(should_compact_at_occupancy(&messages, 2_000, 0.70));
        let (next, summary) = apply_boundary_compact(
            &messages,
            2,
            "occupancy_70",
            2_000,
            BoundaryCompactExtras {
                intent: Some("ship C2"),
                decisions: &["mask first".into()],
                paths: &["src/lib.rs".into()],
                failing: None,
                next: Some("continue Apply"),
                verify: Some("G3"),
            },
        );
        assert!(summary.tokens_after < summary.tokens_before);
        assert!(summary.summary.contains("ade.boundary-capsule/v1"));
        assert!(summary.summary.contains("intent: ship C2"));
        assert_eq!(next[0]["role"], "system");
        assert!(next[1]["content"]
            .as_str()
            .unwrap()
            .contains("ade.boundary-capsule/v1"));
        assert!(next.len() < messages.len());
    }
}
