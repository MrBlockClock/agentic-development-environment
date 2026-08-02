import assert from "node:assert/strict";
import { test } from "node:test";
import {
  looksLikeHttpUrl,
  parseGitHubTicket,
  parseHttpUrl,
  urlChipLabel,
} from "./urlAttach.ts";
import {
  makeAttachment,
  packagePromptWithAttachments,
} from "./fileKind.ts";

test("looksLikeHttpUrl accepts single urls only", () => {
  assert.equal(looksLikeHttpUrl("https://example.com/a"), true);
  assert.equal(looksLikeHttpUrl("http://localhost:3000"), true);
  assert.equal(looksLikeHttpUrl("not a url"), false);
  assert.equal(looksLikeHttpUrl("https://a.com\nhttps://b.com"), false);
});

test("parseGitHubTicket extracts owner/repo#n", () => {
  const issue = parseGitHubTicket(
    "https://github.com/MrBlockClock/agentic-development-environment/issues/12",
  );
  assert.ok(issue);
  assert.equal(issue?.label, "MrBlockClock/agentic-development-environment#12");
  assert.equal(issue?.kind, "issue");

  const pr = parseGitHubTicket("https://github.com/foo/bar/pull/3");
  assert.equal(pr?.kind, "pull");
  assert.equal(pr?.label, "foo/bar#3");
});

test("url chip label prefers ticket then host/path", () => {
  assert.equal(urlChipLabel("https://github.com/a/b/issues/1"), "a/b#1");
  const parsed = parseHttpUrl("https://docs.example.com/guide");
  assert.ok(parsed);
  assert.match(urlChipLabel(parsed!.url), /docs\.example\.com/);
});

test("ticket attachment packages for the model", () => {
  const ticket = makeAttachment({
    path: "https://github.com/acme/app/issues/9",
    name: "acme/app#9",
    kind: "ticket",
  });
  assert.equal(ticket.kind, "ticket");
  const packed = packagePromptWithAttachments("See", [ticket]);
  assert.match(packed, /ticket acme\/app#9/);
});
