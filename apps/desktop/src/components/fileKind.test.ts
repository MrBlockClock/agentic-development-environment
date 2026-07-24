import assert from "node:assert/strict";
import { test } from "node:test";
import {
  fileKindFromName,
  isFileLikeHref,
  isUnderWorkspace,
  packagePromptWithAttachments,
  parseAttachedBlock,
  refuseAttachmentReason,
  makeAttachment,
} from "./fileKind.ts";

test("fileKindFromName maps common extensions", () => {
  assert.equal(fileKindFromName("a.PDF"), "pdf");
  assert.equal(fileKindFromName("shot.png"), "image");
  assert.equal(fileKindFromName("main.rs"), "code");
  assert.equal(fileKindFromName("notes.md"), "text");
  assert.equal(fileKindFromName("bundle.zip"), "archive");
  assert.equal(fileKindFromName("noext"), "other");
});

test("refuseAttachmentReason blocks secrets and executables", () => {
  assert.ok(refuseAttachmentReason(".env"));
  assert.ok(refuseAttachmentReason("id_rsa.pem"));
  assert.ok(refuseAttachmentReason("setup.exe"));
  assert.equal(refuseAttachmentReason("readme.md"), null);
});

test("packagePromptWithAttachments appends Attached block", () => {
  const out = packagePromptWithAttachments("Hello", [
    makeAttachment({ path: "C:\\Dev\\a.pdf", name: "a.pdf" }),
  ]);
  assert.match(out, /Attached:/);
  assert.match(out, /a\.pdf/);
});

test("packagePromptWithAttachments annotates images as binary", () => {
  const out = packagePromptWithAttachments("What is this?", [
    makeAttachment({ path: ".ade/inbox/shot.webp", name: "shot.webp" }),
  ]);
  assert.match(out, /image\//);
  assert.match(out, /vision unavailable/);
});

test("packagePromptWithAttachments notes vision parts when capable", () => {
  const out = packagePromptWithAttachments(
    "What is this?",
    [makeAttachment({ path: ".ade/inbox/shot.webp", name: "shot.webp" })],
    { visionCapable: true },
  );
  assert.match(out, /vision part/);
});

test("parseAttachedBlock round-trips", () => {
  const packed = packagePromptWithAttachments("Hi", [
    makeAttachment({ path: "/tmp/x.png" }),
  ]);
  const parsed = parseAttachedBlock(packed);
  assert.equal(parsed.text, "Hi");
  assert.equal(parsed.attachments.length, 1);
  assert.equal(parsed.attachments[0]?.path, "/tmp/x.png");
});

test("isFileLikeHref and workspace prefix", () => {
  assert.equal(isFileLikeHref("https://x.com/a.pdf"), true);
  assert.equal(isFileLikeHref("https://x.com/page"), false);
  assert.equal(isUnderWorkspace("C:\\Dev\\ade\\foo.rs", "C:\\Dev\\ade"), true);
  assert.equal(isUnderWorkspace("C:\\Other\\x.rs", "C:\\Dev\\ade"), false);
});
