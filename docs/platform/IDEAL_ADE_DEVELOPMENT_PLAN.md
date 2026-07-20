# Ideal ADE Development Plan

**Schema:** `ade.ideal-dev-plan/v1`  
**Status:** Active · I6 complete · Ideal spine shipped · 2026-07-20  
**Canvas:** Ideal-ADE-development-plan.canvas.tsx  
**Depends on:** IDEAL_ADE_MASTERPLAN.md · COMPETITIVE_RESEARCH_PACK.md · protocol scorecard

## North star

Browser + Desktop **Agent Development Environment**: local harness control plane,
do-work Home, verify-as-truth, BYOK honesty, progressive disclosure.
Coexist with Cursor/Claude — do not become another IDE fork.

**Audit is a Trust feature, not the brand.**

## Early goal: ADE builds itself + Dev/Debug

| Goal | Meaning |
|------|---------|
| **Dogfood** | ADE workspace = ADE repo. Agent turns + verify ladder develop ADE. |
| **Dogfood profile** | One-click “Open ADE on itself” / local profile points at repo root. |
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
| WP19 | Gold-set ≥10 deterministic harness tasks (+ dogfood); stretch ~48 builtin |
| WP20 | `ade eval --gold` CLI + `evals/gold/manifest.json` |
| WP21 | CI job runs gold-set and fails on regression |

## I6 work packages — done

| WP | Work |
|----|------|
| WP22 | Guided / Power / Dev surface modes (persist; Guided hides Power nav) |
| WP23 | Brand-first Home (ADE hero) + Dev surface forces Dev/Debug chrome |

## Held

- SSO/SCIM/RBAC · Public EXECUTE HTTP · VS Code fork race · Signed marketplace · Cloud sync default

## Verify every phase

`cargo fmt` · `clippy -D warnings` · targeted tests · `tsc --noEmit` · prefer `ade verify` through G3 · `ade eval --gold`.
