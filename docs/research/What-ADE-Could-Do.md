---
pdf_options:
  format: Letter
  margin: 15mm
---

<div style="text-align:center;margin-top:1.2in;">

# What ADE Could Do

### Product Capability Paper for This Monorepo

**Shipped spine → H-track bets → user-facing futures**  
**Companion:** *Ideal ADE: Applying Human–Agent Market Findings*  
**Basis:** Orch G0–G4 · Effort B0–B4 · DEC-A-014 · live `.ade/`  
**Edition:** 2026-07-23

</div>

<p style="page-break-after:always;"></p>

**Type:** Product capability paper (this codebase)  
**Audience:** Maintainers deciding H-track bets  
**Date stance:** 2026-07 — *this* ADE, not a generic agent IDE

---

## Abstract

ADE already ships more harness spine than most “AI IDE” demos: Suggest / Apply / Automate, leases, Isolate worktrees, Verify G0–G5, Continuity capsules, SpendGuard reserve/reconcile, eng-goals, skills/rules/profiles packs, and a Desktop control plane. What it **could** do next is close the gap to the ideal ADE: honest dollars, contract-before-Apply, slot Orchestrator truth, model profiles, richer lease UX, and action envelopes in the trace — without waiting on Zed ACP.

This paper states **what ADE can become as a product**, ranked by leverage against habits the market study named, with shipped evidence and concrete next bets.

---

## 1. Who ADE is today (one paragraph)

ADE is a **local harness / multi-host agent OS**: Desktop is the control plane; crates under `agents` / `workflow` / `core` are the brain; `.ade/` on disk is shared identity. Autonomy maps Suggest ≈ Propose, Apply ≈ Act under leases, Automate ≈ Act + required verify. Orch G0–G4 and Effort B0–B4 are done. VSCodium is retired. Zed ACP is scaffold (`ade acp --probe` only). Critical path is harness depth (DEC-A-014), not editor wrapping.

---

## 2. What ADE already can do (shipped leverage)

### 2.1 Run agents under a turn law

- Every turn ends `completed` | `failed` | `cancelled` in the feed.  
- Budgets, Continue / Fix&retry, thrift Continuity — Effort B0–B4.  
- **Could mean for users:** fewer blank-retry death spirals; failures are actionable.

### 2.2 Separate plan from mutate

- Suggest queues PLAN / tasks; Apply claims one under leases.  
- Isolate Apply → `.ade/worktrees/{task}`.  
- **Could mean:** parallel work without peer-chat chaos; one writer per path when dogfooded.

### 2.3 Gate risk with ToolEffect + rules

- Authority classifies effects; `.ade/rules` (secrets, money, build-risk, generated-deny, …) always-on.  
- Skills (verify-ladder, handoff-continuity, accidental-data-loss-prevention, …) activate on demand.  
- **Could mean:** safety without dumping every procedure into every prompt.

### 2.4 Verify outside the generator

- VerifyRunner G0–G5; Automate requires verify-on-complete.  
- Gold harness (~51 tasks) via `ade eval --gold`.  
- **Could mean:** self-certify is structurally harder than in chat-only tools.

### 2.5 Continuity instead of paste thrash

- Capsules (`ade.handoff/v1`), host `next_safe_command`, resume thrift.  
- **Could mean:** long missions survive budget stops without dumping the thread.

### 2.6 Meter spend (basic)

- Reserve / reconcile, session + daily caps, Trust Spend UI, integer micros money helpers.  
- **Could mean:** hard stops exist — honesty of the $ number is the gap (see §4).

### 2.7 Eng-goals as first-class objects

- Create / list / active / run under `.ade/goals/`.  
- Desktop Role strip + intent terminalization (G0–G3).  
- **Could mean:** contract spine exists; hard gate before Apply is the unlock.

### 2.8 Desktop as control plane

- Home composer, Guided/Power/Dev, Terminal, Monaco editor + sensitive paths, Workspaces, Browser, Trust, Atlas, Recipes, BYOK + model picker.  
- **Could mean:** full agent OS UX without claiming “we are an IDE.”

---

## 3. Gap to ideal (honest scorecard)

| Ideal principle | This ADE | Gap |
|-----------------|----------|-----|
| Envelope per side effect | ToolEffect + leases + PLAN ≈ partial | Persist full envelope in turn / Continuity |
| Contract before Apply | Goals exist; Apply not hard-gated | Eng-goal / clarify gate |
| Slot hierarchy product | Role strip + claim-one | Slot Orchestrator + heartbeats |
| Honest spend | Caps + ledger | Rates / reserve / cache honesty (H1) |
| Model profiles | Guidance packs; ModelSelector stub | `ade.model-profile/v1` + router (H3) |
| Lease conflict UX | Enforcement + strip helper | Actionable CTAs (H4) |
| Risk-tiered HITL | Autonomy dial + PLAN approve | Policy object for secrets/infra/migrate |
| Editor eyes | ACP probe only | Optional after H1–H3 (DEC-A-014) |
| Continuity | Strong | Defaults / dogfood polish |
| Verify | Strong | Contract-linked cmds; race gold (H5) |

---

## 4. What ADE could do next (product bets)

Ranked by leverage for *this* repo vs market habits.

### Bet 1 — SpendGuard honesty (H1)

**Could do:** Trust Spend that matches invoice class — real rates (no silent $0), sane reserve, cache fields when known, clear used / reserved / remaining.

**Kills:** “I thought I had budget” + opaque $/outcome.  
**Depends on:** existing ledger + money helpers.  
**Not:** Mission Control charts.

### Bet 2 — Contract gate before Apply

**Could do:** Refuse Act-class tools until active eng-goal has AC + OOS + verify pointer — or ≤3 clarify chips resolve.

**Kills:** underspec → expensive mutate → redo (largest human waste in the study).  
**Depends on:** existing goal store + next-actions chips.  
**Ship shape:** Suggest freely; Apply blocked with CTA “define goal.”

### Bet 3 — Slot Orchestrator truth (H2)

**Could do:** Bind Suggest=Planner, Apply=Worker, verify=Judge to task claim + heartbeats; make dual-writer impossible even when UI is misused.

**Kills:** peer multi-agent tax / two workers on one path.  
**Depends on:** leases + tasks registries already on disk.  
**Defer:** Mission Control until slots are real.

### Bet 4 — Model profiles + router (H3)

**Could do:** `ade.model-profile/v1` — autonomy ceiling, tool mask, effort floor, spend ceiling, slot eligibility; router replaces `ModelSelector → None`.

**Kills:** model roulette mid-task; wrong-model-for-role spend.  
**Depends on:** ModelPicker + SessionProfile stubs.  
**User-visible:** “why this model” chip on turn start.

### Bet 5 — Lease conflict product UX (H4 slice)

**Could do:** Blocked Apply explains who holds what; CTAs: wait · Isolate · rotate lease agent.

**Kills:** silent conflict / red badge with no path.  
**Depends on:** AgentSessionStrip + registry.

### Bet 6 — Authorized action envelopes in the trace

**Could do:** Persist effect · paths · lease · autonomy · plan/goal id per tool grant into turn events and Continuity.

**Kills:** unauditable side effects; weak replay.  
**Unlocks:** regulated / enterprise packs later without redesign.

### Bet 7 — Harness gold races (H5)

**Could do:** Expand gold with dual-writer, wrong-slot, budget stop, spend-cap halt, Isolate merge regressions.

**Kills:** “works in demo, fails under load.”  
**Depends on:** existing 51-task gold + `ade eval --gold`.

### Bet 8 — Reproduce-first + evidence-graded research skills

**Could do:** On-demand skills that force repro / evidence before patch or claim.

**Kills:** sycophantic “looks good” research and coding thrash.  
**Cheap:** markdown + activate path already exists.

### Bet 9 — Risk-tiered HITL policy

**Could do:** Secrets / infra / migrate / publish force Propose or human confirm even under Automate; log waives.

**Kills:** review habituation on high-blast paths.  
**Depends on:** ToolEffect + build-risk / secrets rules.

### Bet 10 — Zed ACP soft shell (Z?, after H1–H3)

**Could do:** Full ACP stdio mapping turns → Zed Agent Panel; Open-in-Zed for coding eyes.

**Does not:** redefine product identity; DEC-A-014 keeps this optional.  
**Fork ladder:** only if ACP+Desktop cannot close written chrome gaps.

---

## 5. Narrative product futures (what users experience)

### Future A — “Contracted Apply”

User states intent → ADE opens eng-goal → ≤3 clarify → Suggest plans → Apply claims leased task → verify → Continuity if budget bites. **Feeling:** less thrash, more finished work per dollar.

### Future B — “Honest Trust”

User sees spend that matches provider reality; caps halt; Continue resumes thrift without paste. **Feeling:** agent OS you can run overnight without invoice shock.

### Future C — “Slot crew”

Planner proposes; Worker mutates one claim; Verifier grades; conflicts offer Isolate. **Feeling:** multi-agent that doesn’t step on itself.

### Future D — “Profiled models”

Cheap / strong / tool-light profiles bound to slots and spend ceilings. **Feeling:** amplification without roulette.

### Future E — “Optional Zed eyes”

Same `.ade/` identity; Desktop control plane; Zed for code browsing when ACP is ready. **Feeling:** harness OS with better eyes — not “we forked an editor.”

---

## 6. What ADE should not try to be (near-term)

| Temptation | Why not (for this ADE) |
|------------|-------------------------|
| Cursor/Zed clone as identity | DNA + DEC-A-014: harness first |
| Flat swarm of peer agents | CooperBench-style peer tax; leases already reject this |
| Enterprise Mission Control first | Needs slot truth + envelopes first |
| Vertical SaaS (sales/support) as core | Packs later; coding harness is the wedge |
| Blocking all progress on ACP | Scaffold only; must not gate H1–H3 |

---

## 7. Capability map — today → could

```
TODAY                          COULD (H-track)
─────                          ───────────────
Suggest / Apply / Automate  →  + contract gate + risk HITL
Leases + Isolate            →  + conflict CTAs + heartbeats
Verify G0–G5 + Automate     →  + eng-goal-linked cmds + race gold
Continuity + thrift         →  + default polish
Spend caps + ledger         →  + invoice-honest SpendGuard
Goals + Role strip          →  + hard gate before Act tools
Guidance packs              →  + model profiles + router
ToolEffect + PLAN           →  + persisted action envelopes
ACP --probe                 →  + soft shell (optional, later)
Gold ~51                    →  + multi-agent / budget races
```

---

## 8. Recommended sequencing (this repo)

Per DEC-A-014: pick **one** primary of H1 or H2, then the other; H3 rides with H2; H4/H5 continuous; Z after dogfood.

```
Now     H1 SpendGuard honesty  OR  H2 Slot Orchestrator
Next    Contract-before-Apply (pairs with either)
Then    H3 model profiles + router
Parallel  H4 lease UX · H5 gold races · envelope-in-trace · skills
Later   Zed ACP M2 soft shell
Held    Mission Control · SSO/SCIM · hard fork
```

**Highest combined ROI:** H1 + contract gate — dollars and redo-loops are the two market wounds ADE is already positioned to close.

---

## 9. Success criteria for “ADE could do this”

Ship a dogfoodable story:

1. **Cannot Apply without a contract** (or explicit waive logged).  
2. **Cannot Automate-complete without verify**.  
3. **Cannot dual-write** a leased path (slot + lease).  
4. **Spend UI within tolerance of invoice** for a known model.  
5. **Continuity resume** after budget stop without paste.  
6. **Gold green** including at least one dual-writer / wrong-slot case.  

When those six hold, ADE is not “an IDE with chat” — it is a **harness OS** that applies the market study for real.

---

## 10. Conclusion

**What ADE could do:** become the local agent OS that makes the market study’s winning pattern unavoidable — contracts, envelopes, leases, verify, honest spend, Continuity, profiled models — while keeping Desktop as control plane and Zed as optional eyes.

**What ADE already is:** halfway there on spine; short on honesty, gates, and slot productization.

**What to build:** H1 or H2 first, contract gate close behind, profiles next, ACP last among equals.

---

## Appendix A — Shipped evidence (paths)

| Capability | Path hint |
|------------|-----------|
| Turn / autonomy / authority | `crates/agents/src/{turn,autonomy,authority}.rs` |
| Skills / goals / spend / handoff | `crates/agents/src/{skills,goal,spend,handoff}.rs` |
| Leases / verify / tasks | `crates/workflow/src/{parallel,verify,tasks}.rs` |
| Guidance profiles | `crates/core/src/guidance.rs` · `.ade/profiles/` |
| Rules / skills on disk | `.ade/rules/` · `.ade/skills/` |
| Desktop control plane | `apps/desktop/` |
| ACP scaffold | `crates/acp/` |
| Decisions | `docs/decisions/DEC-A-010` … `014` |
| DNA | `AGENTS.md` |

## Appendix B — Relation to papers

| Paper | Role |
|-------|------|
| *Agents Meet Humans* | Agnostic market / habit evidence |
| *Ideal ADE From Market Study* | Normative architecture |
| *What ADE Could Do* (this) | Instantiation on this monorepo |
| *ADE Tokenomics* | I/O · context · usage strategies (not $) |
| Harness / amplification brief | Harness · MAS · verify · $ metering adjacent |
