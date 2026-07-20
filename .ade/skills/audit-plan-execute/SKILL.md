---
name: audit-plan-execute
description: >-
  Follow ADE AUDIT → PLAN → EXECUTE routing, PlanEnforcer, owned_paths, and
  handoff capsules. Use when planning work, executing phases, or multi-agent tasks.
---
# Audit → Plan → Execute

1. **AUDIT** — read-only discovery; produce scores/findings; no code edits.
2. **PLAN** — write `.ade/plan/last.json` via `ade plan` / desktop Plan; define phases, gates, owned_paths.
3. **EXECUTE** — only approved phases; respect owned_paths and path leases.

## Hard gates

- PlanEnforcer blocks risky agent mutations without `.ade/plan/last.json`.
- Never expand EXECUTE scope beyond the approved plan.
- Never run two write-capable agents on the same checkout paths.

## Continuity

Load latest handoff under `.ade/handoff/`; write a new capsule on completion with verify results folded in.
