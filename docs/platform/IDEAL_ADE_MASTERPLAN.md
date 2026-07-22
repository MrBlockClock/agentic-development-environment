# Ideal ADE Masterplan

**Schema:** `ade.ideal-masterplan/v1`  
**Status:** Research-backed product plan · I1–I6 Ideal spine shipped · 2026-07-20  
**Companion canvas:** Ideal-ADE-masterplan.canvas.tsx

## Positioning

ADE is an **Agent Development Environment** available on **Browser and Desktop**.

**Audit is a feature**, not the product. It helps new users and “evaluate existing”
flows under **Trust & Health**. The product spine is:

1. **Harness runtime** (Model + scaffolding)
2. **Agent studio** (do work)
3. **Project setup** (recipes, rules, skills)
4. **Trust & Health** (audit, verify, spend)
5. **Integrations** (keys, MCP)

## Research synthesis (2025–2026)

| Topic | Implication for ADE |
|-------|---------------------|
| Agent = Model + Harness (ETCLOVG) | Name and expose Execution, Tooling, Context, Lifecycle, Observability, Verification, Governance |
| Plan → Execute → Verify | Agent must not self-certify; verify ladder / tests gate “done” |
| Progressive disclosure (Skills) | T0 AGENTS.md · T1 catalog · T2 skill body · T3 refs |
| First 5 minutes UX | Kill blank canvas; guided first wins; autonomy dial; artifact-first |
| Cursor / Claude patterns | Plan mode, `/init`-style understand, specific prompts, verifiable goals |

## Early goal: dogfood + Dev/Debug

ADE must **build itself**: workspace = ADE repo, agent + verify develop the product.
**Dev/Debug mode** exposes harness internals (traces, budgets, ToolEffect, leases,
rebuild-lock warnings). I1 ships toggle stub + dogfood Home starter; I2 ships full
Dev panels; I4 ships “Improve ADE” guided win.


## Ideal IA

- **Home** — brand, composer, guided starters, continue last handoff
- **Agent studio** — turns, autonomy dial, budgets, plan gate
- **Terminal** (post-Ideal **T1**) — interactive PTY in Desktop; autonomy-gated
- **Project setup** — recipes, rules, skills, AGENTS.md
- **Trust & Health** — audit, verify, spend, continuity
- **Settings** — surface mode, agent defaults, provider presets (local)
- **Integrations** — BYOK keys, MCP
- Enterprise profiles / SSO (held)

Editor/extensions: coexist via **Open in Cursor/VS Code**; full Code OSS fork is a held non-goal (see Development Plan **E1**).

## Autonomy dial

Observe → Propose → Act → Automate  

Must be visible on every Agent turn. Research: one failure without dial-back → abandonment.

## Prompting contract

1. Short start prompt (phase + safety)
2. AGENTS.md + scoped `.ade/rules`
3. Skill catalog always; full skills on demand (`activate_skill` in Ideal)
4. Scrubbed handoff
5. **Permissions/budgets enforced in harness code**, not only in prompts

## Roadmap phases

| Phase | Intent |
|-------|--------|
| **I1** | Product spine reset — Home = do work; Audit demoted; Dev/Debug stub; dogfood starter |
| **I2** | Named Harness Runtime — budgets, traces, full Dev/Debug panels, rebuild-lock warn |
| **I3** | Prompting v2 — activate_skill, shrink T0 |
| **I4** | New-user activation — guided wins incl. Improve ADE (self-build) |
| **I5** | Eval gold-set (≥10 → 50 tasks) — dogfood gold tasks included |
| **I6** | Beautiful UI min-max — Guided/Power/Dev modes |

## Early goal: dogfood + Dev/Debug

ADE must **build itself**: workspace = ADE repo; agent + verify develop the product.
**Dev/Debug** exposes harness internals. I1 = toggle + dogfood starter; I2 = full panels;
I4 = “Improve ADE” guided win. One write agent per checkout; warn on binary locks.

## Non-goals

- Committing real API keys to git
- Enterprise SSO/RBAC until foundation Ideal UI lands
- Treating Audit as the default home metaphor
- Racing a full VS Code / Code OSS fork for extensions (companion + Terminal first)

## Provider keys (local testing)

Use BYOK via OS keychain or local env import:

```powershell
# In gitignored .env (never commit):
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
ADE_IMPORT_ENV_KEYS=1
```

On desktop/CLI startup with `ADE_IMPORT_ENV_KEYS=1`, ADE copies non-empty
`OPENAI_API_KEY` / `ANTHROPIC_API_KEY` / `ADE_<PROVIDER>_API_KEY` into the
OS keychain for profile `local`. Keys are never written into the repo.
