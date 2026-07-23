---
pdf_options:
  format: Letter
  margin: 15mm
---

<div style="text-align:center;margin-top:1.0in;">

# ADE Tokenomics

### Input · Output · Context · Usage Strategies

**Not a pricing paper.** Dollar metering is SpendGuard — see companions.  
**This paper:** how tokens *move* through the harness — what to send, what to emit, what to keep in the window, when to compact / hand off / escalate.

**Edition:** 2026-07-23 · **Schema:** `ade.tokenomics/v1`  
**Inputs:** `TOKEN_ECONOMICS_RESEARCH.md` · Effort B0–B4 · T0–T3 assembly · Continuity · Anthropic context eng / caching docs · Chroma context rot  
**Deep dive companion:** [*Token Usage, Compaction & Fertility*](./ADE-Token-Usage-Compaction-Fertility-Deep-Dive.md) — languages, formats, custom-language limits, compaction science

</div>

<p style="page-break-after:always;"></p>

## 0. Vocabulary (lock this)

| Term | Means | Does *not* mean |
|------|-------|-----------------|
| **Tokenomics** | Strategies for **input composition**, **output discipline**, **context window hygiene**, and **usage patterns** across turns | Invoice math, USD rates, SpendGuard honesty |
| **Effort budget** | Per-turn gas: tool **rounds** + **output** tokens | “How smart the model is” or $ caps |
| **SpendGuard** | Session/daily **$** hard stops; reserve → reconcile | Context compaction policy |
| **Context** | What is in the model window *this round* (system + history + tools) | Disk state / `.ade/` identity |
| **Cache-stable prefix** | Ordered static bytes that prompt-caching can hit | “Any long prompt” |
| **Fertility** | Same text → fewer/more tokens across tokenizers/models | Quality by itself (EMNLP 2024: compression ≠ accuracy) |

**Orthogonal tanks (always):**

```
tool rounds  ≠  output tokens  ≠  context occupancy  ≠  $ spend
```

Wrong failure mode = wrong fix. Empty Effort → Continue / thrift. Full window → compact / Continuity. Empty $ → SpendGuard halt. Don’t swap models to “fix” a context problem.

---

## 1. Thesis

Tokens are the **discrete channel** between host and model. ADE cannot rewrite a hosted tokenizer. It *can* decide:

1. **What enters** the channel (rules, skills, files, history, tool junk).  
2. **What leaves** (verbosity, diffs vs essays, tool spam).  
3. **What persists** across rounds (growing transcript vs capsule).  
4. **When the loop thrifts** (host `next_safe_command`, handoff) vs pays for rediscovery.

**Quality ≈ model × harness × human process** still holds. Tokenomics is the harness’s **context & I/O discipline** — the middle term applied to the channel.

---

## 2. The agent-loop shape (why strategy matters)

Each tool round typically **re-sends** a growing prefix (system + history + prior tool results). Mechanism:

```
cost_shape ≈ O(rounds × growing_context)   // not O(rounds)
```

So:

- Dumping “helpful” context early **taxes every later round**.  
- Unbounded tool-result accumulation is **quadratic in practice**.  
- Blank regenerate / “continue” without a next step buys **another full rebill** of the same rot.

**ESTIMATE (workload-dependent):** multi-round coding agents often use **3–8×** tokens vs a single-shot call before useful completion. Measure on ADE gold / dogfood — don’t treat as law.

**Cached tokens still occupy the window.** Caching changes **provider price**, not capacity. Tokenomics cares about **occupancy and recall**; SpendGuard cares about **invoice class**.

---

## 3. Input strategies (what to send)

### 3.1 Progressive disclosure (HIGH pattern)

| Layer | Content | When |
|-------|---------|------|
| **T0** | Tiny start contract (~300–400 tok); points at `activate_skill` | Always |
| **Always-on rules** | Deny/invariants only (secrets, money, build-risk) | Always, soft-budget truncated |
| **Catalog** | Skill *names* + one-liners | Always (cheap index) |
| **T2 / match** | Keyword / always_apply skill bodies | Only when matched — keep thin |
| **T3 activate** | Full skill + capped `references/` | On tool call mid-turn |
| **Handoff** | Capsule summary, not full chat paste | On Continuity resume |

**Anti-pattern:** stuffing every procedure into always-on rules. That is **context dump with better branding**.

### 3.2 Cache-stable ordering (HIGH — provider docs)

Put **static** content first; **dynamic** last:

```
system / T0
  → tool schemas (stable)
    → AGENTS.md + scoped rules (stable-ish)
      → skill catalog (stable)
        → eng-goal / owned paths
          → handoff capsule
            → user turn
              → tool results (most volatile)
```

**Breaks the prefix (avoid early):** timestamps, random IDs, per-turn “now”, churning model ids, unsorted file dumps, “session UUID” in system text.

Minimum cacheable prefix is commonly ~**1,024** tokens (provider-dependent). Write tax (cache write > base input) means **low hit rate can cost more** than no cache — measure hits.

### 3.3 Retrieval beats dump (HIGH)

| Instead of | Do |
|------------|-----|
| Paste whole module | Search → symbol → short excerpt |
| “Here’s the repo” | Scoped paths + lease owned_paths |
| Re-paste last 40 turns | Continuity capsule + thrift resume |
| Always-on 8k skill bible | Catalog + `activate_skill` |

### 3.4 ADE soft input budgets (shipped)

From `context.rs` defaults (chars÷4 **heuristic only** — never bill on this):

| Bucket | Soft tokens |
|--------|-------------|
| Always-on | 400 |
| Rules | 2,400 |
| Skills | 6,000 |
| Handoff | 500 |
| Status | Green / Warn@85% / Critical |

**Gap:** soft truncation of **system assembly** ≠ mid-thread **history** compaction. History still grows unless Continuity / human discipline intervenes.

---

## 4. Output strategies (what to emit)

### 4.1 Constrain outputs first (HIGH)

On frontier coding APIs, **output is typically ~4–6× input** per token. Thinking/extended reasoning is usually billed as output-class. Therefore:

1. Cap **max output / Effort dial** before micro-optimizing a 200-token system trim.  
2. Prefer **structured short acts** (diff, tool call, next_actions chips) over essays.  
3. Verifier slots should emit **pass/fail + evidence pointers**, not regenerated patches.

### 4.2 ADE Effort = output + rounds gas (shipped)

| Effort | Tool rounds (typical) | Output cap (typical) |
|--------|----------------------|----------------------|
| Low | 16 | ~4k |
| Med | 24 | ~16k |
| High | 32 | unlimited (provider limit) |

**Implementation intent (`session.rs`):** Effort counts **generated (output) tokens only**. Counting input each round would re-bill the system prompt and kill Apply mid-task — that is correct **tokenomics**, not a loophole.

Apply / Automate / Continuity resume floor **Med+** so thrift resumes aren’t starved.

### 4.3 Output anti-patterns

| Anti-pattern | Better |
|--------------|--------|
| Narrate every tool | Tool call + one-line why |
| Paste full file “for clarity” | Patch / hunk only |
| Self-certify in prose | Run verify; quote sensor |
| “Here’s everything I considered” | Decision + next_actions ≤3 |
| Regenerate whole answer on typo | Targeted fix turn |

---

## 5. Context window strategies (what to keep)

### 5.1 Context rot (HIGH — Anthropic / Chroma line of work)

Larger windows ≠ reliable use of mid-context. Models **under-attend** buried instructions and facts as occupancy grows. “Just use 200k” is not a strategy.

**Tokenomics response:** keep the **working set small and fresh**; move durable state to disk (`.ade/goals`, leases, capsules), not to chat immortality.

### 5.2 Compaction policy (ideal)

| Trigger | Action |
|---------|--------|
| ~**70%** window (or Warn→Critical trend) | Compact: drop stale tool blobs; keep decisions, paths, failing tests, open questions |
| Scope change | **New thread** + capsule link — don’t stretch one transcript across two jobs |
| Budget exhaust (rounds/out) | Continuity capsule; host may run `next_safe_command`; thrift resume |
| Dual concern (explore + mutate) | Split Scout vs Worker sessions — don’t mix discovery dumps into Apply history |

**ADE today:** Continuity + compaction **metrics** on capsules; **not** automatic mid-thread history compaction. That is a first-class Tokenomics gap.

### 5.3 What belongs on disk vs in-window

| On disk (`.ade/`) | In-window |
|-------------------|-----------|
| Goals, leases, tasks, rules, skills bodies | Active goal summary |
| Capsules / chat thread archive | Last decisions + open blockers |
| Verify evidence pointers | Failing command + excerpt |
| Spend ledger | “Approaching cap” chip only |

---

## 6. Usage strategies (how humans + harness spend turns)

### 6.1 Good turn shapes

| Shape | When | Tokenomics effect |
|-------|------|-------------------|
| **Clarify ≤3** | Underspec before Apply | Avoids expensive wrong mutate |
| **Suggest → Apply claim-one** | Multi-step work | Planner history ≠ worker history |
| **Verify gate** | Automate / done claims | Sensors replace essay self-grades |
| **Continue + thrift** | Effort empty mid-mission | Pays for *progress*, not rediscovery |
| **Isolate worktree** | Parallel / risky Apply | FS isolation; don’t duplicate context dumps |

### 6.2 Bad usage (human × loop)

| Habit | Channel effect |
|-------|----------------|
| Blank “try again” | Full rebill; same rot |
| Context dump “to be safe” | Permanent tax every round |
| Mid-session model swap | Cache miss + distribution shift |
| New chat, same vague task | Re-pays exploration |
| Continue with no next step | Pure overhead turn |
| Paste prior chat into Continuity | Defeats thrift; rebuilds dump |

### 6.3 Multi-agent tokenomics

Peer agents that **restate** each other’s context multiply occupancy without multiplying truth (CooperBench coordination tax is quality; tokenomics adds **N× rebilled prefixes**).

**ADE contract:** Planner ≠ Worker ≠ Verifier; **claim-one**; leases. Share **structured task + capsule**, not full peer transcripts.

---

## 7. Fertility & model choice (same text, different tokens)

Tokenizers differ. Newer families can emit **~+30%** tokens for identical text (vendor notes — verify per card). **Fertility** = tokens per unit meaning on *your* corpus.

| Do | Don’t |
|----|-------|
| Pick models with fertility + quality on ADE gold | Assume fewer tokens ⇒ better |
| Lock model for a session’s cache prefix | Silent swap mid-task |
| Route Scout to small / local | Use frontier for every `ls` |

**EMNLP 2024 reminder:** tokenization-as-compression does **not** reliably predict accuracy. Fertility is an **I/O efficiency** knob inside a quality floor — not a free lunch.

---

## 8. Playbook — best ways (ranked for ADE)

Host-enforceable first; prompt pleading last.

| Rank | Practice | Why |
|------|----------|-----|
| 1 | **Thin always-on + activate skills** | Stops permanent input tax |
| 2 | **Cache-stable prompt order** | Hits when providers cache; keeps prefix sane even without $ |
| 3 | **Orthogonal Effort (rounds + out-tokens)** | Right stop; Continuity path |
| 4 | **Continuity thrift + host next_safe** | Progress outside rediscovery loops |
| 5 | **Contract / clarify before Apply** | Wrong work is the worst token spend |
| 6 | **Retrieval over dump** | Protects every future round |
| 7 | **Output discipline** (short acts, verify not essays) | Output is the expensive side of the channel |
| 8 | **Compact @~70% / scope-split threads** | Fights rot and O(N×rounds) |
| 9 | **Model lock + profiled routing** | Cache + role-fit |
| 10 | **Provider usage for truth; chars÷4 for assembly only** | Don’t optimize a fake meter |

**SpendGuard** sits beside this list for **$** — it does not replace any row above.

---

## 9. ADE scorecard — shipped vs Tokenomics gaps

| Capability | Today | Gap |
|------------|-------|-----|
| T0–T3 + `activate_skill` | Shipped | Tighten always_apply / keyword bloat |
| Soft system budgets + Warn/Critical | Shipped | History compaction @~70% |
| Effort rounds + output caps + honesty | B0–B4 done | — |
| Continuity + host thrift | Shipped | Default auto-compact trigger |
| Cache-stable order | Documented | Enforce + cache-hit telemetry |
| chars÷4 assembly | Shipped | Label in UI; never bill |
| Fertility bench | Planned | Gold fertility suite |
| Model profiles / lock | Stub / manual picker | `ade.model-profile/v1` |
| Output style policy | Effort maxTokens | Stronger “short act” host norms |
| $ honesty | Separate track (H1) | Not Tokenomics core |

---

## 10. Design rules (normative for ADE)

1. **Tokenomics ≠ Spend.** Never name a $ feature “tokenomics.”  
2. **Disk is cheap; window is sacred.** Persist goals/leases/evidence; keep working set small.  
3. **Static first, volatile last.** Protect the prefix.  
4. **Catalog > body.** Bodies on activate.  
5. **Output before input micro-trims.** Cap essays and thinking.  
6. **Count output for Effort; count provider usage for $.**  
7. **Empty tank → Continuity, not Provider error, not silent model swap.**  
8. **Compact or split before rot.** ~70% or scope change.  
9. **Planner history ≠ Worker history.**  
10. **Measure on gold / dogfood** before claiming multipliers.

---

## 11. Relation to other papers

| Document | Role |
|----------|------|
| **This paper** | I/O · context · usage strategies |
| `TOKEN_ECONOMICS_RESEARCH.md` | Internal science notes + $ adjacency |
| *Agents, Harnesses & …* brief | Harness / MAS / verify; $ pricing as *adjacent* |
| *Ideal ADE* / *What ADE Could Do* | Product architecture & bets |
| *Agents Meet Humans* | Agnostic human habits (incl. dump / regen) |
| Effort / SpendGuard plans | Orthogonal tanks |

---

## 12. One-page cheat sheet

```
INPUT     thin rules · catalog · activate · retrieve · static→dynamic
OUTPUT    short acts · Effort caps · verify>prose · no dump-on-regen
CONTEXT   working set small · disk for durable · compact@~70% · split scope
USAGE     clarify→Suggest→Apply claim-one → verify → Continuity thrift
NEVER     mid-task model roulette · paste thrash · bill on chars÷4
ORTHOGONAL  rounds ≠ out-tokens ≠ window ≠ $
```

---

*End of ADE Tokenomics · `ade.tokenomics/v1` · 2026-07-23*
