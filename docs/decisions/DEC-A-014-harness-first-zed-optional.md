---
layout: default
title: DEC-A-014-harness-first-zed-optional
---

# DEC-A-014 — Harness-first; Zed host is optional path

- **Status:** Accepted
- **Date:** 2026-07-23
- **Depends on:** DEC-A-010, DEC-A-013
- **Canvas:** ADE-master-gameplan.canvas.tsx (master) · ADE-harness-multiagent-gameplan.canvas.tsx (H-detail)

## Context

Orch G0–G4 and Effort B0–B4 are shipped. The next leverage is **harness depth** (model routing, multi-slot orchestration product, SpendGuard, evals)—not editor wrapping. Zed ACP remains valuable but is no longer the critical path.

## Decision

1. **Primary product track:** ADE harness / multi-agent Orchestrator (Desktop + CLI + workers).
2. **Zed / ACP / soft shell / fork ladder:** **Maybe path** — resume when harness dogfood demands better coding eyes, or as a parallel stretch after H-track milestones.
3. **Per-model amplification:** First-class **model profiles** (traits → autonomy defaults, tool masks, effort floors, spend ceilings, slot eligibility).
4. **Multi-agent robustness:** Planner ≠ Worker ≠ Verifier; leases + worktrees + task claim remain truth; no flat peer democracy; Mission Control only after slot truth exists.

## Consequences

- Near-term eng priority: H1 SpendGuard depth **or** H2 slot Orchestrator + model router (pick one primary; other follows).
- `ade acp` soft shell (Z1) is optional eyes; do not block harness PRs on ACP fidelity.
- Fork ladder review: DEC-A-015 — stay L1 unless written chrome gaps force L3/L4.
- Docs/canvases that treated Zed wrap as primary are subordinate to this ADR.
