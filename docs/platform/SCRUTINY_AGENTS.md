---
layout: default
title: Scrutiny agents
---

# Scrutiny agents — stack fields + JD alignment

**Status:** Operating catalog · 2026-07-24  
**Invoke:** Cursor `.cursor/agents/<name>.md` · skill `.ade/skills/scrutiny-council`  
**Authority:** Product DNA in `AGENTS.md` wins over career/JD pressure. Vision docs are aspirational only.

---

## 1. End-goal (what “done” means)

| Horizon | Goal | Must remain true |
|---------|------|------------------|
| **Shipped DNA** | Local Rust harness + Desktop control plane: spend honesty, verify G0–G5, leases/slots, continuity, MCP + OS vault | Not an IDE fork; not a SaaS portal clone |
| **Near product** | Ideal spine: Home / Agent / Trust / Integrations; Suggest → Apply → Automate with verify-as-done | Browser dual-path where contracted; Desktop owns secrets/MCP |
| **Career / JD** | Portfolio proof of **platform/ops engineering**: APIs, secrets, connectors, verify rigor, React+TS surfaces, cloud-adjacent integrations | JD skills transfer **into** ADE surfaces — ADE does **not** become Enspire |

**Verdict:** Reflect the JD where it is rational (ops discipline, React+TS, Azure/Stripe/GitHub as *connectors*, Auth0-class *secret discipline*). Do **not** adopt JD product shape (multi-tenant portals, Auth0 as ADE login, Stripe as ADE monetization, Azure Functions as ADE runtime).

---

## 2. Stack under scrutiny (ADE DNA)

| Layer | Tech | Primary scrutiny agent | Failure mode to catch |
|-------|------|------------------------|------------------------|
| Harness OS | Rust workspace | `rust-harness-scrutiny` | Silent tool effects, clippy debt, phantom path fixes |
| Control plane | Tauri 2 + IPC | `tauri-desktop-scrutiny` | Capability over-grant, secrets in frontend |
| Surface | React 19 / Vite 6 / Tailwind 4 | `react-desktop-ui-scrutiny` | Generic AI chrome, cards-in-hero, dual-path drift |
| Progressive IA | Setup / Agent / Trust | `progressive-ui-scrutiny` | Simple-mode resurrection, nav clutter |
| HTTP | Axum API | `axum-api-scrutiny` | Browser/Desktop route parity gaps, `ApiError` RA traps |
| Ledger | SQLite / Turso | `sqlite-ledger-scrutiny` | Unmetered spend, missing Δ, soft deletes without audit |
| Secrets | OS key vault | `mcp-secrets-scrutiny` | Env leaks, vault not injected into MCP, `.env` commits |
| Tools | MCP host | `mcp-secrets-scrutiny` | Unapproved servers, missing envKeys |
| Verify | G0–G5 + gold | `verify-gold-scrutiny` | Claiming green while Problems/E0xxx live |
| Spend | H1 metering | `spend-honesty-scrutiny` | $0 on priced turns, reserve ≠ estimate story |
| Orchestration | Leases / slots H2–H4 | `leases-slots-scrutiny` | Dual writers, wrong slot writes |
| Continuity | C1–C5 capsules | `continuity-channel-scrutiny` | Lost handoff, compact without write-before |
| Plugins | Wasmtime | `wasm-plugins-scrutiny` | Unbounded host calls, no capability mask |
| E2E | Playwright | `playwright-e2e-scrutiny` | Stale preview port, missing new UI asserts |
| Product DNA | Positioning | `dna-anti-ide-scrutiny` | Cursor/Zed clone features without harness value |
| Connectors | Integrations catalog | `integrations-connectors-scrutiny` | Fake “connected”, brand-only tiles |
| Diagnostics | Problems / RA | `problems-diagnostics-scrutiny` | Phantom crates, red left in Problems |
| Career lens | Platform/ops JD | `jd-platform-ops-scrutiny` | JD-bleed into DNA; missing transferable proof |
| Recipes | Stack Fit | `recipe-stack-fit-scrutiny` | Wrong trust contract / G5 evidence story |
| ADR / layout | Decisions | `architecture-adr-scrutiny` | Undocumented forks, REPO_LAYOUT drift |

**Council:** `scrutiny-council` — picks the subset for the change set and returns a single severity-ranked report.

---

## 3. JD map (rational vs irrational)

Context: platform/ops–style needs around **Azure SWA/Functions, Auth0/Entra, Stripe, Postgres, React+TS monorepo** (Enspire-class work), plus recruiter-visible GitHub.

| JD need | Rational ADE reflection | Irrational bleed (reject) |
|---------|-------------------------|---------------------------|
| React + TypeScript | Desktop apps quality, dual-path IPC types, a11y | Rebuild Enspire portal IA inside ADE Home |
| Secrets / identity hygiene | OS vault, Keys, risk HITL, no secret quotes | Require Auth0/Entra login for local ADE |
| Payments literacy | Stripe as **Integration** connector + spend honesty H1 | Stripe Checkout as ADE’s business model |
| Cloud ops | Azure/AWS/GCP in Integrations + MCP recipes | Host ADE agent loop on Azure Functions |
| Postgres / data honesty | Ledger accuracy, migrations discipline, audit rows | Replace SQLite DNA with cloud Postgres day-one |
| CI / verify culture | G0–G5, gold races, clear-problems | Skip gates to ship demo chrome |
| GitHub visibility | Public ADE + Integrations GitHub connector | Private-only dogfood with no portfolio story |
| Multi-tenant portals | Out of ADE DNA (belongs to Enspire) | “Admin / Business / Client portal” surfaces in ADE |

**JD scrutiny bot (`jd-platform-ops-scrutiny`) asks every milestone:**

1. Does this change strengthen a transferable platform skill *without* changing product DNA?
2. Can a recruiter see evidence (public repo, Integrations, Verify, Trust spend)?
3. If the change only serves Enspire product shape, park it outside ADE.

---

## 4. How to run

```text
# Full council on current diff
@scrutiny-council

# Field bots (examples)
@rust-harness-scrutiny
@spend-honesty-scrutiny
@jd-platform-ops-scrutiny
```

Or skill: read `.ade/skills/scrutiny-council/SKILL.md` and invoke the listed agents for the touched layers.

**Output contract (every bot):**

1. **Scope** — files / DNA clauses touched  
2. **Findings** — Critical / High / Medium / Nit (path:line when possible)  
3. **JD note** — transferable / neutral / bleed risk  
4. **Verify** — gates or gold ids that must still pass  

---

## 5. Non-goals

- Replacing human merge approval  
- Auto-fixing without review  
- Putting career/JD content into shipped user-facing ADE copy  
- Blurring `ADE_PRODUCT_VISION.md` with DNA in `AGENTS.md`

---

## Cross-links

- DNA: [`AGENTS.md`](../../AGENTS.md)  
- Stack Fit: [ADE_STACK_FIT](ADE_STACK_FIT.html)  
- Vision (aspirational): [ADE_PRODUCT_VISION](ADE_PRODUCT_VISION.html)  
- Competitive: [COMPETITIVE_RESEARCH_PACK](COMPETITIVE_RESEARCH_PACK.html)
