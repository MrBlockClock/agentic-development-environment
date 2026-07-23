---
pdf_options:
  format: Letter
  margin: 14mm
---

<div style="text-align:center;margin-top:0.85in;">

# ADE Master Gameplan

### How to build ADE from the research stack

**Harness OS · Tokenomics · Compaction · Contracts · Slots · Honest $ · Optional eyes**

**Edition:** 2026-07-23 · **Rev:** Wave 5 dogfood polish · **Schema:** `ade.master-gameplan/v1`  
**Canvas:** `ADE-master-gameplan.canvas.tsx`  
**Locked by:** DEC-A-010 / 013 / 014 / 015 · AGENTS.md DNA  
**Inputs:** Ideal ADE · What ADE Could Do · Tokenomics · Compaction/Fertility · Market study · Harness brief  
**Status:** Waves **0–5** + post-wave deepen (**H2 depth · invoice Δ · Continuity thrift · E1 · C4**) **shipped**. Zed stays L1 (DEC-A-015). Mission Control chrome **deferred**.

</div>

<p style="page-break-after:always;"></p>

## 0. One sentence

Build ADE as a **local harness OS** that makes the research wins unavoidable — contracts before Apply, envelopes for side effects, leases for writers, verify outside the generator, honest spend, Continuity + compaction for the token channel — with Desktop as control plane and Zed as **optional eyes**, never identity.

---

## 1. North star (non-negotiable)

| Principle | Source |
|-----------|--------|
| Quality ≈ **model × harness × human process** | Harness brief / market study |
| Product identity = **harness**, not IDE fork | DEC-A-014 · AGENTS.md |
| Suggest ≈ Propose · Apply ≈ Act under leases · Automate ≈ Act + verify | DNA |
| Planner ≠ Worker ≠ Verifier · no peer democracy | CooperBench · DEC-A-014 |
| Tokenomics ≠ $ · Effort ≠ SpendGuard ≠ window | Tokenomics paper |
| Compaction = harness job · private “ADE language” = avoid | Compaction deep-dive |
| Side effects consume **authorized envelopes** | Ideal ADE |

**Equation for builders:**

```
Ship discipline as product objects — not as prompt pleading.
```

---

## 2. Research → product map

| Research finding | Product object | Track |
|------------------|----------------|-------|
| Underspec → redo | Eng-goal + ≤3 clarify **before Apply** | **G1** |
| Dual writers / peer tax | Slot Orchestrator + leases + claim-one | **H2** |
| Self-certify | Verify ladder + Automate gate | Have → deepen |
| Context dump / O(N×rounds) | Tool-blob clearing + capsules @ boundary | **C1–C2** |
| Fertility ≠ invent cipher | Catalog IDs · diffs · compact JSON | **C** / Tokenomics |
| Model roulette | `ade.model-profile/v1` + router | **H3** |
| Opaque $ | SpendGuard honesty | **H1** |
| Review habituation | Risk-tiered HITL | **G2** |
| Continuity > paste | Capsules + thrift + host next_safe | Have → **C3** |
| Editor chrome temptation | Soft shell after H+C dogfood | **Z?** |

---

## 3. Baseline (already shipped — do not rebuild)

| Area | Status |
|------|--------|
| Orch G0–G4 | Suggest / Apply / Isolate / eng-goals / Role strip |
| Effort B0–B4 | Rounds + output caps · honesty · Continuity Continue |
| Leases + worktrees + task claim | On-disk truth · H4 conflict CTAs |
| Verify G0–G5 + Automate verify-on-complete | Shipped |
| Rules / skills / pack profiles · T0–T3 activate | Shipped |
| Continuity `ade.handoff/v1` | Shipped (thrift resume · last-write · CLI `handoff resume`) |
| SpendGuard | Honesty gate · invoice-class Trust (used/reserved/remaining) · ledger Δ |
| Contract / risk gates | G1 eng-goal · G2 risk HITL |
| Slots / profiles | H2 role gates + heartbeats + claim_gate + Verifier session · H3 router |
| Channel | C1–C5 mask / capsules / SelfCompact / fertility gold |
| Desktop Ideal spine | Control plane · Wave 5 failure CTAs / dogfood chips · Verify (judge) |
| ACP / Zed | Soft shell (`ade acp`) · DEC-A-015 stay L1 |
| Gold | **73/73** · H5/C5/H2 · invoice · thrift · E1 · C4 |

**Stance:** Amplify spine. Don’t reboot as an editor project.

---

## 4. Four parallel tracks (one critical path)

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌──────────┐
│  G · Gates  │  │  H · Harness│  │  C · Channel│  │  Z · Eyes│
│  contracts  │  │  slots/$/   │  │  tokenomics │  │  ACP/Zed │
│  HITL       │  │  profiles   │  │  compaction │  │  maybe   │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └────┬─────┘
       │                │                │              │
       └──────────┬─────┴────────┬───────┘              │
                  │  CRITICAL    │                      │
                  ▼              ▼                      ▼
            Wave 0–5 + deepen (non-Zed)     Done · MC deferred
```

**Critical path (DEC-A-014 + research ROI) — executed:**  
**G1** ‖ **H1** ‖ **C1** → **H2/H3/G2/C2/C3** → **H4/H5/C4/C5/E1** → **Z1/Z2 (L1)** → **W5** → **H2 depth** → **invoice Δ** → **Continuity thrift** → **E1/C4 depth**

**Now:** Critical harness tracks **closed** (Zed eyes stay L1; Mission Control chrome deferred). Dogfood + gold keep green.

Pick **one** primary eng focus per sprint; keep thin parallel slices if staffing allows.

---

## 5. Wave plan

### Wave 0 — Lock (done)

- Accept DEC-A-014  
- Research stack published (Ideal / Could Do / Tokenomics / Compaction / Market)  
- Dogfood Continuity + Isolate scripts green enough to build on  

### Wave 1 — Force multipliers (~2–4 weeks)

Kill the highest-cost failure modes from human/market studies.

| ID | Ship | Done when |
|----|------|-----------|
| **G1** | Contract before Apply | **Shipped** — AC+OOS+verify / clarify / waive |
| **H1** | SpendGuard honesty | **Deepened** — caps×rates gate (+ confirm unmetered); sane per-round reserve from message size / last usage (not full context); Desktop warn |
| **C1** | Tool-result clearing | **Deepened** — per-result truncate + cross-round mask (keep last K rounds, `ADE_TOOL_RESULT_KEEP_ROUNDS`) |

**Dogfood story:** “I cannot Apply into the void; I cannot burn $ I don’t see; long turns don’t drown in tool paste.”

### Wave 2 — Amplification (~3–5 weeks)

| ID | Ship | Done when |
|----|------|-----------|
| **H2** | Slot Orchestrator truth | **Shipped** — `SlotRole` Planner≠Worker≠Verifier; write/claim gates; claimed-task heartbeats (TTL/3); `claim_gate` when ready queue non-empty (+ waive jsonl); Desktop **Verify (judge)** |
| **H3** | Model profiles + router | **Shipped** — `ade.model-profile/v1` builtins + `.ade/model-profiles/`; route annotates Started (`profile_id` · `route_reason` · `slot`); effort floor + tool-effect deny; `ModelSelector` suggests by role |
| **C2** | Boundary + @~70% capsules | **Shipped** — occupancy safety net + structured `ade.boundary-capsule/v1`; persists handoff + `.ade/continuity/last-boundary.json`; feed `context_compacted` |
| **C3** | Write-before-compact | **Shipped** — `.ade/continuity/last-write.json` (intent·decisions·paths·failing·next·verify); boundary compact + turn end |
| **G2** | Risk-tiered HITL | **Shipped** — High/Critical for publish/infra/migrate/secrets; `risk_gate:` + Desktop approve&retry; waives → `.ade/risk/waives.jsonl` |

**Dogfood story:** “A slot crew with honest models and a clean window.”

### Wave 3 — Robustness (~2–4 weeks)

| ID | Ship | Done when |
|----|------|-----------|
| **H4** | Lease conflict UX | **Shipped** — strip CTAs Wait / Isolate / Rotate / Suggest; turnFailure actions |
| **H5** | Harness gold races | **Shipped** — g52–g60: dual-writer, slot gates, spend honesty, occupancy compact, risk/publish, contract gate, Isolate worktree, model router |
| **C4** | SelfCompact-style tool | **Shipped** — `ade__compact_context` + rubric gate (reject empty/stuck) · T0 nudge · shares C2 boundary path · g73 |
| **C5** | Compaction/fertility gold | **Shipped** — g61–g65: mask fidelity, capsule vs full, format fertility order, cipher-loses, section rubric (`fertility.rs`) |
| **E1** | Action envelopes in trace | **Shipped** — envelope on ToolCall · persist `.ade/continuity/last-actions.json` · feed label · Audit actions · g72 |

**Dogfood story:** “Production survival under parallel workers and long missions.”

### Wave 4 — Optional eyes (after Wave 2 dogfood)

| ID | Ship | Done when |
|----|------|-----------|
| **Z1** | ACP soft shell | **Shipped** — `ade acp` JSON-RPC stdio; Suggest/Apply/Automate; Desktop **Zed** open |
| **Z2** | Fork ladder review | **Shipped** — DEC-A-015: stay L1; promote L3/L4 only with written chrome-gap gate |

### Wave 5 — Dogfood polish (done · 2026-07-23)

Close the friction that blocks daily harness dogfood after Waves 1–4.

| ID | Ship | Done when |
|----|------|-----------|
| **P0** | Lease conflict → feed | **Shipped** — no `alert`; H4 CTAs via turnFailure |
| **P0** | Spend honesty CTAs | **Shipped** — Confirm unmetered · Open spend rates (+ `allowUnpriced` retry) |
| **P0** | Wrong-slot primary CTA | **Shipped** — `slot_gate` → **Switch to Apply** first |
| **P1** | Continuity strip | **Shipped** — visible while busy; Continue disabled until free |
| **P1** | Open-in-Zed errors | **Shipped** — surface in Desktop error, not `alert` |
| **P1** | Debug dogfood chips | **Shipped** — Continuity · Isolate (mirror Automate) |
| **P2** | Gold dogfood flags | **Shipped** — g54/g56/g60/g63/g65 dogfood in manifest + builtin |

**Dogfood story:** “Blocked Apply tells me what to do next; Continuity and Isolate are one chip away; gold marks what we live on.”

**Non-goal still:** Mission Control chrome, SSO/SCIM, VSCodium revival, private ADE cipher language, premature Zed fork.

---

## 6. Architecture target (build toward)

```
Human / Desktop control plane
        │
   Eng-goal · clarify · autonomy · Effort · Trust
        │
   Orchestrator (slots · queue · claim · heartbeats)
        │
   Model router (profiles)
        ├── Planner (Suggest)
        ├── Worker (Apply + leases ± Isolate)
        ├── Scout (Observe)
        └── Verifier (sensors first)
        │
   Authorize (ToolEffect · PLAN · envelopes · risk HITL)
        │
   Execute (FS / shell / MCP)
        │
   Observe → mask blobs · assemble context (T0–T3)
        │
   Verify G0–G5
        │
   Continuity / compact @ boundary · Spend ledger · .ade/ identity
        │
   Optional: Zed ACP eyes
```

**Eyes change. Brain does not.**

---

## 7. Channel strategy (Tokenomics + Compaction) — build rules

1. **Thin always-on · catalog · activate** — already partially shipped; tighten always_apply bloat.  
2. **Mask before summarize** — Complexity Trap.  
3. **Structured capsules** — intent · decisions · paths · failing · next · verify.  
4. **Diffs / hunks / compact JSON** — not full dumps; not private stego language.  
5. **Effort counts output**; provider usage bills $; chars÷4 is assembly-only.  
6. **Compact at semantic boundaries**; ~70% safety net.  
7. **English + saturated formats** for operator I/O (fertility≠quality; don’t translate to “save tokens”).

---

## 8. Success metrics (ship when these move)

| Metric | Direction |
|--------|-----------|
| Apply without contract (or waive) | → 0 |
| Automate complete without verify | → 0 |
| Dual-writer lease violations | → 0 |
| Spend UI vs invoice delta | → 0 · Trust shows used/reserved/remaining + ledger Δ |
| Continuity resume without paste | ↑ · thrift prompt + last-write + host next_safe |
| Peak tokens / successful Apply (C1–C2) | ↓ |
| Compaction artifact retention (paths/failing) | ↑ |
| Gold green incl. race cases | required |
| Mid-task silent model swap | → 0 |

---

## 9. 90-day sequencing (as executed → next)

| Window | Plan | Outcome |
|--------|------|---------|
| D1–14 | G1 primary · C1/H1 thin | **Done** — contract gate live |
| D15–35 | H1 + C1 + G1 dogfood | **Done** — honesty + mask |
| D36–55 | H2 core · H3 start · C2 | **Done** — role gates + capsules |
| D56–75 | H3 · C3 · G2 | **Done** — router · write-before-compact · risk HITL |
| D76–90 | H4/H5 · C4/C5 · Z1/Z2 · W5 · H2 depth | **Done** — gold 68 · ACP L1 · heartbeats/claim_gate/Verifier |
| D91+ | Invoice Δ · Continuity thrift · E1/C4 depth | **Done** — g69–g73 · thrift · envelopes · SelfCompact rubric |

**Next sprint**  
Primary: keep gold/dogfood green. **Deferred:** Mission Control chrome · further Zed beyond L1 (DEC-A-015).

---

## 10. Team / work splitting (even if solo)

| Hat | Owns |
|-----|------|
| Gates | G1–G2, eng-goal UX, clarify chips |
| Orchestrator | H2, H4, leases/claim heartbeats |
| Money | H1 SpendGuard |
| Router | H3 profiles |
| Channel | C1–C5, Continuity polish |
| Eval | H5 + C5 gold |
| Eyes | Z1/Z2 done — L1 only unless ADR |
| Dogfood | Wave 5 polish · scripts under `scripts/dogfood-*.ps1` |

Solo next: dogfood + gold green — Mission Control / Zed fork stay deferred.

---

## 11. Anti-goals (explicit)

- Cursor/Zed clone as identity  
- Mission Control before slot truth  
- Flat peer agent swarms  
- Blocking harness on ACP  
- Private ADE “machineese” for tokenization  
- Caps that show $0 while tokens burn  
- Automate that greenlights on model self-report  
- Dumping full chat into Continuity  

---

## 12. Decision log hooks

| If… | Then… |
|-----|-------|
| Invoice trust blocks overnight runs | Prioritize H1 over H2 |
| Dual-writer dogfood fails weekly | Prioritize H2 over H3 |
| Context rot kills long Apply | Prioritize C1/C2 |
| Underspec redo dominates | Prioritize G1 (default) |
| Coding eyes block harness dogfood | Open Z1 early (exception) |
| Someone proposes ADE stego language | Point to compaction paper §5 — reject |

---

## 13. Artifact index

| Artifact | Role |
|----------|------|
| This PDF | Master build plan |
| `ADE-master-gameplan.canvas.tsx` | Interactive board |
| Ideal ADE / What ADE Could Do | Normative + capability |
| Tokenomics / Compaction deep-dives | Channel science |
| Agents Meet Humans | Market habits |
| DEC-A-014 | Priority law (harness-first) |
| DEC-A-015 | Fork ladder — stay L1 |
| `evals/gold/manifest.json` | Gold 65 + dogfood flags |
| Prior `ADE-harness-multiagent-gameplan` | H-track detail |

---

## 14. Closing

Waves **0–5** plus post-wave deepen (**H2 depth · invoice Δ · Continuity thrift · E1 envelopes · C4 SelfCompact**) closed the non-Zed research seats that matter.

**Executed order:** contracts · honest $ · clear the window · slot truth · profiles · compact well · eyes (L1) · dogfood polish · H2 depth · invoice Δ · Continuity thrift · envelopes · SelfCompact.  
**Deferred:** Mission Control chrome · Zed L3/L4 fork (needs ADR).

---

*End · `ade.master-gameplan/v1` · 2026-07-23 · critical path closed (non-Zed)*
