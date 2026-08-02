# ADE — Agent Contract

Canonical contract for the ADE project (dogfooding).

## Product DNA (do not blur)

ADE is a **local harness / Agent Development Environment** — Desktop + CLI control plane, not an IDE fork or editor soft shell.

| Layer | Owns | Must not own |
|-------|------|----------------|
| **Model** | Reasoning + tool_use proposals | Filesystem, secrets, false “no tools” claims |
| **Harness / loop** | Tool schemas, ToolEffect auth, budgets, compaction | Pixel UI |
| **Hosts** | ADE Desktop + `ade` CLI | Zed / VSCodium / editor forks / ACP soft shell (**DEC-A-017**) |
| **Eng-goal product** | Outcome + scope + verify-as-done | Endless chat theater |

**Critical path:** harness / multi-agent Orchestrator (**DEC-A-014**). External editor hosts are a **non-goal** (**DEC-A-017**).

**Autonomy:** Suggest ≈ planner/inspect · Apply ≈ worker under leases · Automate ≈ Apply + required verify.

**Apply contract (G1):** Act/Automate tools require an active eng-goal with acceptance criteria + out-of-scope + verify pointer (or ≤3 clarify resolutions / logged waive). Suggest stays inspect-only without a contract.

**Spend honesty (H1):** Non-zero $/MTok when session/daily caps are set (or confirm unmetered / `ADE_ALLOW_UNPRICED=1`). Reserves use estimated message size, not full context window. Trust shows **used / reserved / remaining**; ledger rows show reserved − actual (Δ). Missing provider usage on priced turns falls back to the reserve estimate (never $0).

**Slots (H2):** Suggest = Planner (no write leases / task claims); Apply/Automate = Worker. Backend enforces `slot_gate`. Claimed Apply heartbeats at TTL/3 (`task_heartbeat`). Act/Automate with a ready queue requires claim (`claim_gate`) or audited waive (`.ade/tasks/queue-waives.jsonl`). **Verify (judge)** = Verifier slot (sensors-first; no write leases).

**Model profiles (H3):** `ade.model-profile/v1` under `.ade/model-profiles/`. Router annotates each turn with profile + “why this model” (no silent mid-task swap). Effort floors and tool-effect deny masks apply.

**Risk HITL (G2):** Secrets / infra / migrate / publish require explicit confirm even under Apply/Automate (`risk_gate:`). Waives log to `.ade/risk/waives.jsonl`.

**Channel (C1–C4):** Mask tool blobs; boundary capsule @~70% occupancy (`ade.boundary-capsule/v1`); write-before-compact to `.ade/continuity/last-write.json` (intent·decisions·paths·failing·next·verify); optional `ade__compact_context` (SelfCompact rubric). Thrift resume via Desktop Continue or `ade handoff resume` (host `next_safe`, no paste).

**Lease conflict (H4):** Blocked Apply offers Wait · Isolate · Rotate lease · Suggest.

**Gold races (H5):** `ade eval --gold` includes dual-writer, wrong-slot, spend honesty, occupancy compact, risk/publish, contract gate, Isolate worktree, model router (g52–g60). H2 depth: heartbeat / claim_gate / verifier (g66–g68). Invoice Δ: g69. Continuity thrift: g70–g71. E1 envelopes: g72. C4 SelfCompact rubric: g73. Sprint D vision/PDF: g74–g76. M2 Office extract: g77–g78. M2 audio transcript: g79–g80.

**Compaction gold (C5):** g61–g65 measure mask/capsule savings + fidelity and format fertility (compact JSON beats prose; invented ciphers lose).

**Dogfood polish (W5):** Lease/spend/slot failures use feed CTAs (not alerts); Continuity strip stays visible while busy; Debug chips for Continuity/Isolate; gold dogfood on g52–g65.

**Turn law:** every turn ends in the feed (`completed` | `failed` | `cancelled`).

**Action envelopes (E1):** Authorized tools persist effect·paths·autonomy·risk under `.ade/continuity/last-actions.json`; feed + Trust Audit show them.

**SelfCompact (C4):** `ade__compact_context` requires a reason; stuck/debugging mid-derivation is rejected; T0 nudges when to fire.

**Next:** Critical harness + Composer Media closed. Prefer dogfood/gold green; Mission Control deferred; editor hosts remain non-goals. See Master Gameplan · ADE-next-phase canvas.

Roadmaps: `docs/research/ADE-Master-Gameplan.md`, `docs/platform/ORCHESTRATION_ENG_GOAL_PLAN.md`, `docs/platform/IDEAL_ADE_DEVELOPMENT_PLAN.md`.

## Authority Order

1. Law/security/human direction
2. CI, tests, schemas
3. This AGENTS.md
4. Global guidance + `.ade/rules/` (workspace wins body; deny-writes union)
5. Global + `.ade/skills/` on demand
6. Task/issue acceptance criteria
7. Provider/adapter files
8. Chat memory

## Rules & Skills

- **ADE runtime:** `.ade/rules/*.mdc` + `.ade/skills/*/SKILL.md`
- **Cursor IDE mirror:** `.cursor/rules` + `.cursor/skills` — keep in sync via `scripts/sync-cursor-guidance.ps1` (author in `.ade/`, then sync)
- Phases: rule `golden-path` · Verify gates: skill `verify-ladder`
- Stack/JD scrutiny: skill `scrutiny-council` · agents `.cursor/agents/*-scrutiny.md` · `docs/platform/SCRUTINY_AGENTS.md`
- Profiles: `.ade/profiles/*.toml` + `active-profile.txt`

## Golden Path

- **Root:** `C:\Dev\ade` · **Rust:** stable · **Node:** v22.14.0
- Mirror CI (see `CONTRIBUTING.md`): `sync-cursor-guidance.ps1 -Check` · `cargo fmt --check` · `cargo clippy --workspace --exclude ade-desktop-app --all-targets -- -D warnings` · `cargo test --workspace --exclude ade-desktop-app` · `ade eval --gold` · Desktop `npm run build` + `test:unit`

## Security

- NEVER read/quote `.env`, `*.pem`, `*.key`, `*credentials*.json`, or secrets
- NEVER commit two write-capable agents on one checkout
- NEVER merge/deploy without human approval
- NEVER disable security tools to "make AI work"
