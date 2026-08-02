---
layout: default
title: DEC-A-017-retire-zed-host
---

# DEC-A-017 — Retire Zed / ACP host path (non-goal)

- **Status:** Accepted
- **Date:** 2026-08-02
- **Supersedes in part:** DEC-A-010, DEC-A-012, DEC-A-013, DEC-A-014 (Zed/ACP host track), DEC-A-015
- **Depends on:** DEC-A-014 (harness-first product track remains)

## Context

Older ADRs treated Zed + `ade acp` as coding “eyes” for a multi-host OS. That path conflicted with the product non-goal: ADE is a **local harness / Desktop + CLI control plane**, not an editor integration or fork program. Scrutiny W3 briefly polished `hosts/zed` against that intent.

## Decision

1. **Product hosts:** ADE Desktop + `ade` CLI only.
2. **Non-goals:** Zed soft shell, Open-in-Zed, `ade acp`, editor host packs, Zed fork ladder L1–L4.
3. **Remove** `hosts/zed`, `open_in_zed`, `ade acp` / `crates/acp` from the product tree.
4. Historical ADRs 010–015 remain as history; their Zed/ACP host clauses are **superseded** by this decision.

## Consequences

- Docs and `AGENTS.md` describe Desktop + CLI only.
- Progressive-ui / Workspaces no longer mention Open in Zed.
- Mission Control and editor forks stay deferred non-goals.
