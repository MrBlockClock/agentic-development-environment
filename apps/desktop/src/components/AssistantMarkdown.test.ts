import assert from "node:assert/strict";
import { test } from "node:test";
import {
  normalizeAssistantMath,
  safeMarkdownHref,
} from "./assistantMarkdownPrep.ts";

test("normalizeAssistantMath converts display and inline TeX delimiters", () => {
  const src = String.raw`See \[E=mc^2\] and \(a+b\).`;
  const out = normalizeAssistantMath(src);
  assert.match(out, /\$\$E=mc\^2\$\$/);
  assert.match(out, /\$a\+b\$/);
});

test("normalizeAssistantMath converts math fences", () => {
  const src = "```math\nx = 1\n```";
  const out = normalizeAssistantMath(src);
  assert.match(out, /\$\$\s*x = 1\s*\$\$/);
});

test("safeMarkdownHref allows http(s) and blocks javascript", () => {
  assert.equal(safeMarkdownHref("https://example.com"), "https://example.com");
  assert.equal(safeMarkdownHref("mailto:a@b.c"), "mailto:a@b.c");
  assert.equal(safeMarkdownHref("#anchor"), "#anchor");
  assert.equal(safeMarkdownHref("javascript:alert(1)"), undefined);
  assert.equal(safeMarkdownHref("data:text/html,hi"), undefined);
});
