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
