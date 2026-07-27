---
layout: default
title: ADE GUIDANCE ATLAS PLAN MAP
---

# ADE Guidance, Atlas, and Plan Map

**Schema:** `ade.guidance-atlas-plan/v1`  
**Status:** Product + implementation contract · 2026-07-20

## Positioning

ADE guidance is **local-first** and **dual-scoped**:

| Scope | Meaning |
|-------|---------|
| **Global** | Machine ADE home — personal denies and skills that follow you across workspaces |
| **Workspace** | Current checkout `.ade/` — project-specific authority |

**Atlas** is ADE’s Obsidian-like dual-layer graph (Authority + Work), projected from files and live audit/plan — not an Obsidian vault sync.

**Plan Map (Trust Route)** visualizes AUDIT → PLAN → EXECUTE → VERIFY with a phase DAG, owned paths, and gates.

Scenarios are **profiles** (pack filters), not a third “Patterns” taxonomy.

## Path layout

| Scope | Path |
|-------|------|
| Global rules | `<ade-home>/guidance/rules/*.mdc` |
| Global skills | `<ade-home>/guidance/skills/<name>/SKILL.md` |
| Global audit cache | `<ade-home>/guidance/audit/latest.json` |
| Global profiles | `<ade-home>/guidance/profiles/<id>.toml` |
| Active profile id | `<ade-home>/guidance/active-profile.txt` |
| Workspace rules/skills | `{root}/.ade/rules`, `{root}/.ade/skills` |
| Workspace profiles | `{root}/.ade/profiles/<id>.toml` |

`<ade-home>` resolves to `%LOCALAPPDATA%/ade` on Windows when set, else the ADE data base (`APPDATA` / XDG / `~/.local/share/ade`) without the `ADE_ENV` subdirectory so guidance persists across environments.

Optional frontmatter: `pack: <profile-pack-id>` (untagged items always load unless filtered).

## Merge algorithm

1. Load global rules + workspace rules.
2. Index by file stem; **workspace replaces global** for prompt body on conflict.
3. **Deny-writes union** — any matching deny from either scope still blocks.
4. Skills: merge catalogs; workspace wins on name; `always_apply` if **either** is always (safer).
5. Active profile: keep items whose `pack` is listed (or untagged); untagged **deny** rules always keep.

## Audits

| Kind | Scores | UI |
|------|--------|-----|
| Workspace | L0–L11 on `workspace_root` | Health + Plan Map |
| Global / Machine | ADE install: guidance dirs, preferred workspace pointer, config data dir, Turso URL presence | Health “Machine” strip + Atlas Global hub |

Global audit does not replace workspace audit.

## Plan Map visual language

1. Spine: AUDIT → PLAN → EXECUTE → VERIFY  
2. Center: phase DAG from `depends_on`  
3. Nodes: title, gate chips, owned_path count; blockers amber  
4. Inspector: owned paths, gates, `requires_human`  
5. Score ribbon: `score_before` / `score_max` (+ handoff delta when present)

## Atlas graph model

**Authority nodes:** Global hub, Workspace hub, AGENTS.md, Rules, Skills, Profile.  
**Work nodes:** Audit findings, Plan phases, Verify gates, Handoffs.  
**Edges:** `contains`, `denies_write`, `derived_from`, `depends_on`, `verified_by`.

Interaction: pan/zoom, click → preview pane, layer toggles (Authority / Work / Both), Global vs Workspace tint.

## Non-goals

- Obsidian app sync or vault plugins  
- Separate Patterns product type  
- Cloud multi-tenant graph  
- Auto-injecting all global skill bodies (catalog merge; T2 unchanged)

## Success criteria

- Two workspaces share Global denies with distinct workspace skills  
- Plan Map shows dependencies and owned paths without leaving Plan  
- Atlas jumps Global rule → workspace skill → plan phase in ≤2 clicks  
- Simple surface unchanged (Guidance / Atlas / Plan Map stay Full/Debug)
