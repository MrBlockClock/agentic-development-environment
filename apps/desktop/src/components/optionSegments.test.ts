import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  NEXT_ACTIONS_SCHEMA,
  parseNextActionsPayload,
  splitOptionSegments,
} from "./optionSegments.ts";

describe("parseNextActionsPayload", () => {
  it("accepts ade.next-actions/v1 with string items", () => {
    const parsed = parseNextActionsPayload(
      JSON.stringify({
        schema: NEXT_ACTIONS_SCHEMA,
        title: "Pick a path",
        items: ["Plan first", "Apply now"],
      }),
    );
    assert.ok(parsed);
    assert.equal(parsed.title, "Pick a path");
    assert.equal(parsed.items.length, 2);
    assert.equal(parsed.items[0]?.prompt, "Plan first");
  });

  it("accepts label/prompt objects via ade.options fence hint", () => {
    const parsed = parseNextActionsPayload(
      JSON.stringify({
        items: [
          { label: "Queue PLAN", prompt: "Queue the PLAN into tasks" },
          { label: "Apply next", prompt: "Claim and apply the next task" },
        ],
      }),
      "ade.options",
    );
    assert.ok(parsed);
    assert.equal(parsed.items[0]?.label, "Queue PLAN");
    assert.equal(parsed.items[0]?.prompt, "Queue the PLAN into tasks");
  });

  it("rejects plain json without schema", () => {
    const parsed = parseNextActionsPayload(
      JSON.stringify({ items: ["a", "b"] }),
      "json",
    );
    assert.equal(parsed, null);
  });
});

describe("splitOptionSegments", () => {
  it("prefers structured fence over regex", () => {
    const text = [
      "Here is a plan.",
      "",
      "```ade.next-actions",
      JSON.stringify({
        schema: NEXT_ACTIONS_SCHEMA,
        title: "Next",
        items: [
          { label: "Structured A", prompt: "Do A carefully" },
          { label: "Structured B", prompt: "Do B carefully" },
        ],
      }),
      "```",
      "",
      "Thanks.",
    ].join("\n");

    const segments = splitOptionSegments(text);
    const options = segments.filter((s) => s.kind === "options");
    assert.equal(options.length, 1);
    if (options[0]?.kind !== "options") throw new Error("expected options");
    assert.equal(options[0].title, "Next");
    assert.equal(options[0].items[0]?.prompt, "Do A carefully");
  });

  it("keeps non-overlapping prose lists as fallback", () => {
    const text = [
      "```ade.next-actions",
      JSON.stringify({
        schema: NEXT_ACTIONS_SCHEMA,
        items: ["Fence one", "Fence two"],
      }),
      "```",
      "",
      "What next?",
      "1. Prose one",
      "2. Prose two",
    ].join("\n");
    const options = splitOptionSegments(text).filter((s) => s.kind === "options");
    assert.equal(options.length, 2);
    if (options[0]?.kind !== "options" || options[1]?.kind !== "options") {
      throw new Error("expected two option blocks");
    }
    assert.equal(options[0].items[0]?.label, "Fence one");
    assert.equal(options[1].items[0]?.label, "Prose one");
  });

  it("falls back to prose option lists", () => {
    const text = [
      "Some preamble.",
      "What would you like to do?",
      "1. Inspect the plan",
      "2. Switch to Apply",
      "",
      "Thanks.",
    ].join("\n");
    const segments = splitOptionSegments(text);
    const options = segments.filter((s) => s.kind === "options");
    assert.equal(options.length, 1);
    if (options[0]?.kind !== "options") throw new Error("expected options");
    assert.equal(options[0].items.length, 2);
    assert.equal(options[0].items[0]?.label, "Inspect the plan");
  });
});
