import assert from "node:assert/strict";
import { test } from "node:test";
import { evaluateTurnFailure } from "./turnFailure.ts";

test("classifies OpenCode HTTP 500 and prefers alt model autofix", () => {
  const advice = evaluateTurnFailure({
    error:
      'Provider error: opencode -> https://opencode.ai/zen/v1 returned HTTP 500 Internal Server Error: {"type":"error","error":{"type":"error","message":"Internal server error"}}',
    providerId: "opencode",
    model: "big-pickle",
    baseUrl: "https://opencode.ai/zen/v1",
  });
  assert.equal(advice.kind, "provider_5xx");
  assert.ok(advice.autoFix);
  assert.equal(advice.autoFix?.id, "retry_alt_model");
  if (advice.autoFix?.id === "retry_alt_model") {
    assert.notEqual(advice.autoFix.model, "big-pickle");
  }
});

test("classifies auth as Keys (no autofix)", () => {
  const advice = evaluateTurnFailure({
    error: "Provider error: unauthorized HTTP 401 invalid api key",
    providerId: "opencode",
    model: "big-pickle",
    baseUrl: "https://opencode.ai/zen/v1",
  });
  assert.equal(advice.kind, "provider_auth");
  assert.equal(advice.autoFix, null);
  assert.ok(advice.actions.some((action) => action.id === "open_keys"));
});

test("local unreachable switches provider", () => {
  const advice = evaluateTurnFailure({
    error:
      "cannot reach http://127.0.0.1:31415/v1/chat/completions — is the local gateway running?",
    providerId: "freellm",
    model: "auto",
    baseUrl: "http://127.0.0.1:31415/v1",
  });
  assert.equal(advice.kind, "provider_unreachable");
  assert.ok(advice.autoFix);
  assert.equal(advice.autoFix?.id, "switch_provider");
});

test("classifies tool-round limit and raises effort", () => {
  const advice = evaluateTurnFailure({
    error: "Budget exhausted: agent exceeded the 8-round tool-call limit",
    providerId: "opencode",
    model: "deepseek-v4-flash-free",
    baseUrl: "https://opencode.ai/zen/v1",
    effort: "low",
  });
  assert.equal(advice.kind, "tool_round_limit");
  assert.ok(advice.autoFix);
  assert.equal(advice.autoFix?.id, "raise_steps");
  if (advice.autoFix?.id === "raise_steps") {
    assert.equal(advice.autoFix.effort, "medium");
    assert.ok(advice.autoFix.maxSteps >= 16);
  }
});

test("budget stop with handoff prefers continue_handoff autofix", () => {
  const advice = evaluateTurnFailure({
    error: "Budget exhausted: agent exceeded the 8-round tool-call limit",
    providerId: "opencode",
    model: "deepseek-v4-flash-free",
    baseUrl: "https://opencode.ai/zen/v1",
    effort: "low",
    handoffAvailable: true,
  });
  assert.equal(advice.kind, "tool_round_limit");
  assert.equal(advice.autoFix?.id, "continue_handoff");
});

test("legacy Provider-labeled round limit still classifies", () => {
  const advice = evaluateTurnFailure({
    error: "Provider error: agent exceeded the 16-round tool-call limit",
    providerId: "opencode",
    model: "deepseek-v4-flash-free",
    baseUrl: "https://opencode.ai/zen/v1",
    effort: "medium",
  });
  assert.equal(advice.kind, "tool_round_limit");
  assert.equal(advice.autoFix?.id, "raise_steps");
});

test("classifies token output budget as token_budget", () => {
  const advice = evaluateTurnFailure({
    error: "Budget exhausted: agent exceeded the 8000-token output budget (used 8000)",
    providerId: "opencode",
    model: "deepseek-v4-flash-free",
    baseUrl: "https://opencode.ai/zen/v1",
    effort: "low",
  });
  assert.equal(advice.kind, "token_budget");
  assert.equal(advice.autoFix?.id, "raise_steps");
  assert.ok(advice.actions.some((action) => action.id === "continue_handoff"));
});

test("budget stop offers continue_handoff action", () => {
  const advice = evaluateTurnFailure({
    error: "Budget exhausted: agent exceeded the 16-round tool-call limit",
    providerId: "opencode",
    model: "deepseek-v4-flash-free",
    baseUrl: "https://opencode.ai/zen/v1",
    effort: "low",
  });
  assert.equal(advice.kind, "tool_round_limit");
  assert.ok(advice.actions.some((action) => action.id === "continue_handoff"));
});

test("classifies contract_gate Apply block", () => {
  const advice = evaluateTurnFailure({
    error:
      "Authorization error: contract_gate: Act tools blocked until an active eng-goal has acceptance criteria, out-of-scope, and verify pointer",
    providerId: "opencode",
    model: "big-pickle",
    baseUrl: "https://opencode.ai/zen/v1",
  });
  assert.equal(advice.kind, "contract_gate");
  assert.equal(advice.autoFix, null);
  assert.ok(advice.actions.some((action) => action.id === "define_goal"));
  assert.ok(advice.actions.some((action) => action.id === "switch_suggest"));
});

test("slot_gate primary CTA is Switch to Apply", () => {
  const advice = evaluateTurnFailure({
    error: "slot_gate: Planner cannot acquire write leases",
    providerId: "opencode",
    model: "big-pickle",
    baseUrl: "https://opencode.ai/zen/v1",
  });
  assert.equal(advice.kind, "lease_conflict");
  assert.equal(advice.actions[0]?.id, "switch_apply");
});

test("spend honesty offers confirm unmetered", () => {
  const advice = evaluateTurnFailure({
    error:
      "spend_honesty: set Input/Output $/MTok to match your provider, or confirm unmetered.",
    providerId: "opencode",
    model: "big-pickle",
    baseUrl: "https://opencode.ai/zen/v1",
  });
  assert.equal(advice.kind, "spend_cap");
  assert.ok(advice.actions.some((action) => action.id === "confirm_unmetered"));
  assert.ok(advice.actions.some((action) => action.id === "open_spend_rates"));
});

test("another-agent write lease surfaces H4 CTAs", () => {
  const advice = evaluateTurnFailure({
    error:
      "lease conflict: another agent (abcd1234) holds a write lease on .ade/dogfood",
    providerId: "opencode",
    model: "big-pickle",
    baseUrl: "https://opencode.ai/zen/v1",
  });
  assert.equal(advice.kind, "lease_conflict");
  assert.ok(advice.actions.some((action) => action.id === "wait_refresh"));
  assert.ok(advice.actions.some((action) => action.id === "enable_isolate"));
  assert.ok(advice.actions.some((action) => action.id === "rotate_lease"));
});

test("claim_gate offers Apply next and waive", () => {
  const advice = evaluateTurnFailure({
    error: "claim_gate: 2 queued task(s) — Apply next or waive",
    providerId: "opencode",
    model: "big-pickle",
    baseUrl: "https://opencode.ai/zen/v1",
  });
  assert.equal(advice.kind, "lease_conflict");
  assert.ok(advice.actions.some((action) => action.id === "apply_next"));
  assert.ok(advice.actions.some((action) => action.id === "waive_queue"));
});
