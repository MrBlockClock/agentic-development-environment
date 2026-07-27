---
layout: default
title: IDEAL ADE DEVELOPMENT PLAN
---

# Ideal ADE Development Plan

**Schema:** `ade.ideal-dev-plan/v1`  
**Status:** Active · Ideal spine shipped · **Multi-host north star** (DEC-A-010) · Orch **G0–G4** · Effort **B0–B4** · 2026-07-22  
**Canvas:** `ADE-master-gameplan.canvas.tsx` (live) · archive: `Ideal-ADE-development-plan` · `ADE-orchestration-eng-goal` · `ADE-multihost-gameplan` under `canvases/_archive/`  
**ADRs:** `docs/decisions/DEC-A-010` … `DEC-A-012` · Layout: `docs/architecture/REPO_LAYOUT.md`  
**Depends on:** IDEAL_ADE_MASTERPLAN.md · COMPETITIVE_RESEARCH_PACK.md · protocol scorecard · **ORCHESTRATION_ENG_GOAL_PLAN.md**

## North star

**Cursor-shaped job · Rust agent OS · multi-host eyes.** ADE is the harness control plane
(Suggest/Apply, leases, verify, Continuity, spend, Orchestrator). Coding surfaces are hosts:

| Host | Role |
|------|------|
| ADE Desktop (Tauri) | Harness UI |
| Zed | Primary editor via `ade acp` (ACP) |
| VSCodium | Open VSX companion (“Open in…”) |

Do **not** fork Electron/VS Code/Zed as ADE core. Coexist with Cursor — do not become another IDE fork (see **E1** + DEC-A-010).

**Audit is a Trust feature, not the brand.**

## Multi-host phases (forward)

See canvas **ADE-master-gameplan** (live) · archive **ADE-multihost-gameplan** — M0 identity · M1 harness · M2 `ade acp` · M3 Open-in · M4 Orchestrator · M5 cohesion.

## Early goal: ADE builds itself + Dev/Debug

| Goal | Meaning |
|------|---------|
| **Dogfood** | ADE workspace = ADE repo. Agent turns + verify ladder develop ADE. |
| **Dogfood profile** | One-click “Open ADE on itself” / local profile points at repo root. **Shipped** (Desktop command + Home CTA; persists under `%LOCALAPPDATA%/ade/workspace-root.txt`). |
| **Dev/Debug mode** | Power chrome: turn traces, prompt budgets, ToolEffect, leases, raw verify, rebuild-lock warnings. |
| **Self-build recipe** | Guided win: “Improve ADE” → plan → execute owned paths → verify G2/G3. |

**Constraints:** one write-capable agent per checkout; warn when binaries lock rebuilds; no EXECUTE-over-HTTP for self-build until auth is solid.

Dev/Debug maps primarily to **I2** (done) with an **I1** toggle stub + dogfood Home starter.

## End-goal DoD

| Pillar | Done |
|--------|------|
| Product | Home = do work; Audit under Trust; Browser↔Desktop same IA |
| Harness | Budgets, leases, ToolEffect, verify gates “done” |
| Prompting | T0–T3 + `activate_skill` |
| Activation | First 5 min → artifact win; autonomy dial always visible |
| Dogfood | ADE develops ADE in Dev/Debug without leaving the product |
| Quality | Gold-set evals gate harness changes |

## Phases (~12 weeks sequential)

| Phase | Name | Duration | Ships |
|-------|------|----------|-------|
| **I1** | Spine reset | 1–2 wks | Home/Agent first; Audit demoted; capability badges; Dev/Debug toggle stub + dogfood starter |
| **I2** | Harness UI + Dev/Debug | 2–3 wks | Autonomy dial, budgets, traces, verify-on-complete, full Dev panels, rebuild-lock warn |
| **I3** | Prompting v2 | 2 wks | activate_skill; shrink T0 |
| **I4** | Activation + self-build | 2 wks | 3 guided wins incl. Improve ADE |
| **I5** | Eval harness | 2–3 wks | Gold-set 10→50; CI gate; dogfood gold tasks |
| **I6** | UI min-max | 2 wks | Guided/Power/Dev modes; brand-first polish |

## I1 work packages — done

| WP | Work |
|----|------|
| WP1 | Nav: Home · Agent · Setup · Trust · Integrations |
| WP2 | Home: composer + starters (incl. dogfood) + continue handoff |
| WP3 | Trust: Audit · Plan · Verify · Continuity |
| WP4 | Browser badges for Desktop-only |
| WP5 | Dev/Debug toggle stub |
| WP6 | Docs/canvas updated for dogfood |

## I2 work packages — done

| WP | Work |
|----|------|
| WP6 | Autonomy dial Observe→Propose→Act→Automate; persist preference |
| WP7 | Step / token / $ caps; hard stop in AgentTurnService |
| WP8 | Verify-on-complete (required for Automate) |
| WP9 | Trace panel (turn timeline + ToolEffect) |
| WP10 | Leases visible + rebuild-lock warn |

Manual note: live Automate + G3 dogfood turn in Desktop remains an operator check, not a code blocker.

## I3 work packages — done

| WP | Work |
|----|------|
| WP11 | Host `activate_skill` tool (ReadOnly) |
| WP12 | Catalog-first skill injection (T1; bodies via match/always/activate) |
| WP13 | Shrink T0 start prompt + point at activate_skill |
| WP14 | Rules/Skills editor activation state badges |
| WP14b | T3: `activate_skill` loads `references/*.md` (capped) |

## I4 work packages — done

| WP | Work |
|----|------|
| WP15 | Understand project guided win writes `.ade/artifacts/understand-project.md` |
| WP16 | Run verify from Home; mark win without opening Audit |
| WP17 | Improve ADE starter opens Agent + marks self-build win |
| WP18 | Persist guided wins in `.ade/artifacts/guided-wins.json` |

## I5 work packages — done

| WP | Work |
|----|------|
| WP19 | Gold-set ≥10 deterministic harness tasks (+ dogfood); stretch **50** builtin |
| WP20 | `ade eval --gold` CLI + `evals/gold/manifest.json` |
| WP21 | CI job runs gold-set and fails on regression |

## I6 work packages — done

| WP | Work |
|----|------|
| WP22 | Guided / Power / Dev surface modes (persist; Guided hides Power nav) |
| WP23 | Brand-first Home (ADE hero) + Dev surface forces Dev/Debug chrome |

## Held

- SSO/SCIM/RBAC · Public EXECUTE HTTP · **Full VS Code / Code OSS fork** · Signed marketplace · Cloud sync default

## Next phases (post Ideal)

| Phase | Name | Ships |
|-------|------|-------|
| **N1** | Local auth honesty | Browser token setup UX; clear 401s; never hardcode tokens; scope honesty (no EXECUTE HTTP) |
| **N2** | Browser↔Desktop Simple path | Capability matrix; Guided funnel that does not lie |
| **N3** | Dogfood Automate acceptance | Automate + owned paths + verify-on-complete → G3 checklist |
| **T1** | **Desktop Terminal** | Interactive PTY panel in Desktop; agent-visible cwd; autonomy-gated run; lease-aware when writing |
| **N4** | Continuity resume | Home “Continue last handoff” → Agent |
| **N5** | Atlas / Plan Map depth | Pan/zoom (or equiv); ≤2-click Guidance↔Atlas↔Plan |
| **N6** | Stack Fit depth | Stronger Fit defaults/`why`; Fit smoke tests |
| **N7** | Trust surfaces v2 | Ignore-drift UI; Spend under Trust; Audit log viewer |
| **C1** | Persisted chat transcript | Multi-turn Home/Agent history on disk (per workspace); clear-on-send already in-session |
| **A1** | Desktop multi / parallel agents | Wire `leaseAgentId` + task queue in UI; ≥1 write agent per checkout still enforced |
| **E1** | IDE shell exploration (research) | Decide companion vs embed vs host; **no fork commitment** — see section below |
| **E2** | Monaco editor spike | Desktop Editor nav; workspace text read/write; SensitivePathPolicy; no VS Code extensions |
| **W1** | Environment + Workspaces IA | Rename audit surface; open/adopt/switch folders; Home attachment honesty |

### T1 work packages — Desktop Terminal (priority)

| WP | Work | Status |
|----|------|--------|
| WP41 | PTY backend (Desktop): spawn shell in workspace cwd; stream stdout/stderr; stdin | **Done** (`pty_spawn` / `write` / `resize` / `kill`) |
| WP42 | UI: xterm-style panel (Dedicated nav); resize; copy; clear | **Done** (Terminal nav + `TerminalView`) |
| WP43 | Autonomy + ToolEffect: Propose inspect shell; Act/Automate full shell; show in Live activity | **Done** (`shell::run_command` + inspect allowlist on Propose) |
| WP44 | Rebuild-lock / dangerous-command warn; optional “Open in system terminal” escape hatch | **Done** (deny list + rebuild hint + System terminal button) |

### N4 work packages — Continuity resume

| WP | Work | Status |
|----|------|--------|
| WP47 | `handoff_resume` IPC + `HandoffResume` / `resume_user_prompt` | **Done** |
| WP48 | Home Continue last handoff CTA → auto-submit Agent turn | **Done** |
| WP49 | Environment Continuity panel Continue → Home | **Done** |
| WP49b | `scripts/dogfood-continuity.ps1` (host next_safe + act resume, max_steps≥16) | **Done** |

### N5 work packages — Atlas / Plan Map depth

| WP | Work | Status |
|----|------|--------|
| WP50 | Shared `GraphCanvas` pan/zoom (wheel + drag + Fit) | **Done** |
| WP51 | Atlas + Plan Map use GraphCanvas; inspector jump actions | **Done** |
| WP52 | ≤2-click trail Guidance ↔ Atlas ↔ Plan (focus phase/node) | **Done** |

### N6 work packages — Stack Fit depth + Settings

| WP | Work | Status |
|----|------|--------|
| WP53 | Stronger Fit `why` (match + mismatch) + `FitAnswers::suggested()` host/repo defaults | **Done** |
| WP54 | Fit smoke tests (reorder, regulated, rust/http, mismatch why, never drop) | **Done** |
| WP55 | Recipes UI: suggested defaults, persist Fit, amber mismatch chips, auto top-match | **Done** |
| WP56 | Settings nav/view (surface, autonomy, effort, caps, provider presets) | **Done** |

### N7 work packages — Trust surfaces v2

| WP | Work | Status |
|----|------|--------|
| WP57 | Ignore-drift UI (alignment chips + Repair ignores) | **Done** |
| WP58 | Spend under Trust (daily usage vs caps + Caps → Settings) | **Done** |
| WP59 | Audit log viewer (handoffs + ledger + Export JSON); Trust nav in Standard | **Done** |

### C1 work packages — Persisted chat transcript

| WP | Work | Status |
|----|------|--------|
| WP45 | Persist turns under `.ade/chat/thread.json` + reopen last thread per workspace; Clear chat | **Done** |

### A1 work packages — Desktop multi / parallel agents

| WP | Work | Status |
|----|------|--------|
| WP46 | Persist agent UUID; Apply acquires/renews/releases leases; pass `leaseAgentId`; Standard session strip; Debug enqueue/claim | **Done** |

### E1 — IDE shell options (explore only)

Goal: get **editor + extensions** without abandoning ADE’s harness identity.

| Option | What you get | Cost / risk | Fit for ADE |
|--------|--------------|-------------|-------------|
| **0. Companion** | “Open in Cursor / VS Code” + ADE Terminal + Agent | Low | **Default now** — matches north star |
| **1. ADE Terminal only** | Interactive shell inside Desktop (T1) | Medium | **Shipped** |
| **2. Monaco embed** | Light file edit; **no** VS Code extensions | Medium | **E2 now** — owned-path / handoff text |
| **3. code-server / OpenVSCode Server** | Near-full VS Code UI + many extensions in browser/webview | High (separate process, auth, updates) | Later only if leaving ADE for editor is proven friction |
| **4. Eclipse Theia** | IDE shell; partial extension compat | High | Only if we need an in-house IDE platform |
| **5. Code OSS / VS Code fork** | Cursor-class surface | Multi-year; marketplace/licensing; fights research “refuse fork war” | **Held** — not Ideal |

**Practical recommendation:** Keep **Open in Cursor/VS Code** for extensions. **T1 Terminal** shipped. Ship **E2 Monaco** as a thin Desktop Editor (not an IDE). Revisit option 3 only after E2 dogfood.

### E2 work packages — Monaco editor spike

| WP | Work | Status |
|----|------|--------|
| WP60 | IPC `workspace_read_text` / `workspace_write_text` (under root, SensitivePathPolicy, size cap) | **Done** |
| WP61 | Desktop **Editor** nav + Monaco panel (open/save; language from extension) | **Done** |
| WP62 | DesktopRequired + capability matrix; docs/canvas point at E2 | **Done** |
| WP63 | Handoff → Editor diff (changed_paths chips; DiffEditor vs git HEAD) | **Done** |

### N1 work packages

| WP | Work | Status |
|----|------|--------|
| WP24 | Export browser API token helpers + typed auth errors in `ipc.ts` | Done |
| WP25 | Browser API connection panel (save token, probe health/API, clear) | Done |
| WP26 | Honest scope copy: reads/writes need matching `ADE_API_TOKEN`; no agent/EXECUTE over HTTP | Done |

### N2 work packages

| WP | Work | Status |
|----|------|--------|
| WP27 | Capability matrix (`capabilities.ts`) + UI disclosure | Done |
| WP28 | DesktopRequired funnel for Agent / Keys / MCP in browser | Done |
| WP29 | Guided Home readiness: browser finishes stack+verify; Keys stays Desktop gate | Done |

### K1–K2 work packages

| WP | Work | Status |
|----|------|--------|
| WP30 | Keys Tier-0: provider `<select>` + recommended shortcuts + vault status | Done |
| WP31 | Shared `ModelPicker` + `GET /v1/models` (OpenCode Zen / FreeLLMAPI) | Done |
| WP32 | FreeLLMAPI provider preset (`freellm` → localhost:3001/v1) | Done |
| WP32b | K3 usage strip on Keys + Home (`spend_summary`) | Done |

### W1 work packages — Environment / Workspaces

| WP | Work | Status |
|----|------|--------|
| WP33 | Rename nav Workspace → Environment; audit copy + Fix with ADE | Done |
| WP34 | Workspaces view: Open / Create-Adopt / Recent / Switch + header Change… | Done |
| WP35 | Home + Agent attachment honesty (“Working in {folder}”) | Done |
| WP36 | Relax ADE root to `AGENTS.md`; persist preferred + recent under LOCALAPPDATA | Done |

### N3 work packages — Dogfood Automate

| WP | Work | Status |
|----|------|--------|
| WP37 | `docs/platform/N3_DOGFOOD_AUTOMATE.md` + canvas ADE-n3-dogfood (now `_archive/`) | Done |
| WP38 | `scripts/dogfood-automate.ps1` (Automate + owned `.ade/dogfood` + G3) | Done |
| WP39 | Debug Home chip “Dogfood Automate” | Done |
| WP40 | Live pass evidence on ADE repo (`scripts/dogfood-automate.ps1` exit 0) | Done |

## Orchestration / eng-goal (DNA alignment)

Canonical: [`ORCHESTRATION_ENG_GOAL_PLAN.md`](./ORCHESTRATION_ENG_GOAL_PLAN.md) · live `ADE-master-gameplan` · archive `ADE-orchestration-eng-goal`.

| Phase | Name | Status |
|-------|------|--------|
| **G0** | Turn terminalization (failed in-feed; no orphan YOU) | **Done** (Desktop host) |
| **G1** | Intent surface (mode in transcript; scope chip) | **Done** — Suggest/Apply + Workspace\|Home (`preferred_shell_cwd`) |
| **G2** | Eng-goal object under `.ade/goals/` | **Done** — GoalStore + active strip (Save/Run/Done) |
| **G3** | Suggest→PLAN/tasks · Apply→claim-one · Automate+verify | **Done** — Queue PLAN + Apply next on Home |
| **G4** | Parallel workers / worktrees in product UI | **Done** — Isolate Apply worktree (no Mission Control); dogfood `scripts/dogfood-isolate-apply.ps1` |

Naming: orch **G0–G4** ≠ verify ladder **G0–G4** ≠ Ideal **T1 Terminal**.

## Verify every phase

`cargo fmt` · `clippy -D warnings` · targeted tests · `tsc --noEmit` · prefer `ade verify` through G3 · `ade eval --gold`.
