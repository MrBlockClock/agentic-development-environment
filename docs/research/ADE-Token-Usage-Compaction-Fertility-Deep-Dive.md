---
pdf_options:
  format: Letter
  margin: 14mm
---

<div style="text-align:center;margin-top:0.9in;">

# Token Usage, Compaction & Fertility

### Deep Dive — Languages · Formats · Custom ADE Languages · Hard Limits

**What actually shrinks the channel · What models can still use · What ADE/harnesses can enforce**

**Edition:** 2026-07-23 · **Schema:** `ade.compaction-research/v1`  
**Method:** Parallel research (compaction science · fertility · custom DSL limits) + primary-source spot checks  
**Companions:** *ADE Tokenomics* (I/O playbook) · Continuity `ade.handoff/v1` · Effort B0–B4  
**Stance:** Fertility ≠ quality · compaction ≠ free lunch · inventing “machineese” is usually the wrong bet

</div>

<p style="page-break-after:always;"></p>

## 0. How to read this paper

| Label | Meaning |
|-------|---------|
| **HIGH** | Vendor docs, peer-reviewed / arXiv with clear method, or multi-source corroborated |
| **MEDIUM** | Strong first-party engineering blog or single-study ranking |
| **ESTIMATE** | Workload-dependent; measure on ADE gold before treating as law |

**Two different compressions people confuse:**

| Kind | Question | Owner |
|------|----------|-------|
| **Fertility** | Same *text* → fewer/more **tokens** (tokenizer) | Frozen for API models |
| **Compaction** | Same *mission* → fewer tokens **kept in the window** (drop/summarize/externalize) | Harness / ADE |

Custom “ADE languages” mostly try to hack fertility. Harnesses win by hacking **compaction + structure**.

---

## 1. Executive verdict

1. **Best compaction for coding agents is hybrid (HIGH/ESTIMATE):** clear/mask stale **tool blobs** early; keep recent turns + paths/errors **verbatim**; at semantic boundaries (or ~50–80% window) write a **structured capsule**; put durable state on **disk** (`.ade/`), not in chat immortality.  
2. **Observation masking often matches or beats LLM summarization** on SWE-bench-style agents (**HIGH** — Complexity Trap, arXiv:2508.21433). Don’t pay for a summary when a stub + re-fetch works.  
3. **Structured sections beat freeform prose** for continuity probes (**HIGH** — Factory / Cursor / Anthropic pattern). Ratio alone is a bad quality proxy (~98% token removal can still lose artifacts).  
4. **Natural-language fertility (English-centric BPE):** English densest; Chinese/Japanese ~1.9–2.3×; some languages **12–15×** (**HIGH** — Petrov et al., NeurIPS 2023). That is **channel cost**, not “write ADE docs in Chinese to save tokens.”  
5. **Formats as LLM text:** TSV/CSV ≫ compact JSON ≫ YAML/TOML ≫ pretty JSON ≫ XML/HTML soup (**MEDIUM**). Pay for XML/JSON **delimiters** when reliability > fertility.  
6. **Code fertility (directional, MEDIUM):** Python densest among common stacks; Rust/C/Go denser in *characters* but often **worse** in tokens (OOD syntax + verbose types). Prefer **diffs / hunks**, not “rewrite the module in denser language.”  
7. **Inventing a private ADE language for token savings is a weak bet (HIGH).** Frozen tokenizers fragment rare glyphs; models expand/explain opaque codes; SQL/diff/JSON work because training is saturated. Treat “ADE language” as **contract syntax** (capsule schema, path index, catalog IDs) — not steganographic BPE.  
8. **Hard limit:** You cannot invent new BPE merges for Claude/GPT. Soft neural codecs (GIST/ICAE-class) need **matched open-weight** train/serve pairs — not pasteable into closed APIs.  
9. **ADE today:** Continuity capsules + soft system budgets + Effort output caps — **strong thrift spine**; missing auto **history** compaction, tool-blob clearing policy, and measured fertility gold.  
10. **Next ADE bets (ranked):** tool-result clearing → structured capsule @ boundary/70% → write-before-compact to `.ade/` → SelfCompact-style rubric tool → fertility bench — **not** a private cipher.

---

## 2. Token usage in agent loops (why compaction exists)

Each tool round typically **re-sends** system + history + prior observations:

```
occupancy_shape ≈ O(rounds × growing_context)
```

Stale tool blobs and dumps become a **permanent tax**. Compaction is how the harness resets that tax without abandoning the mission.

**Cached tokens still occupy the window** — caching changes price, not capacity (*ADE Tokenomics*).

**Three-tier memory (production consensus, MEDIUM–HIGH):**

```
Tier 0  Working context     — recent turns, active paths, live tool results
Tier 1  Session compact     — structured capsule / anchored summary
Tier 2  External memory     — .ade/goals, leases, NOTES, verify evidence, capsules on disk
```

Write durable facts to Tier 2 **before** Tier 1 compresses (write-before-compact). Otherwise compaction deletes what you never externalized.

---

## 3. Compaction science — mechanisms & evidence

### 3.1 Mechanism taxonomy (**HIGH**)

| Mechanism | What it does | Fidelity profile |
|-----------|--------------|------------------|
| **Observation masking / tool clearing** | Stub or drop old tool results; keep reasoning/actions | Near-lossless for *chain*; lossy for blob contents (re-fetchable) |
| **Abstractive summary** | Rewrite history → prose/sections | Lossy; drift across regenerations |
| **Anchored iterative merge** | Extend a fixed structured doc; don’t regenerate from scratch | Less drift than full rewrite (**MEDIUM–HIGH**, Factory) |
| **Opaque vendor compact** | Encrypted carry-forward item | Not inspectable (OpenAI Responses) |
| **Extractive token prune** | Keep high-utility tokens (LongLLMLingua-class) | Strong for RAG/QA; transfer to agents **MEDIUM** |
| **Fixed memory overwrite** | Slot rewrite (MemAgent-class) | Near-lossless on trained QA; needs training |
| **SelfCompact** | Model invokes compaction **tool** + **rubric** for when | Adaptive timing; training-free scaffold |

### 3.2 When to compact — threshold vs decision (**HIGH**)

Fixed token thresholds (~50–80% window, or vendor defaults like Anthropic ~150k / min 50k) are **structure-blind**: they can fire mid-derivation and force expensive reconstruction.

**SelfCompact** (arXiv:2606.23525): compaction **tool** + rubric (*fire* when sub-task resolved / converging; *suppress* mid-derivation or stuck). Reported: matches/exceeds fixed-interval quality at **30–70%** lower per-question token cost; up to **+18.1** points on math vs no-summary baseline across six benches / seven models.

**Implication for ADE:** prefer **semantic boundaries** (task claim complete, verify passed, Suggest→Apply handoff) *plus* a safety threshold — not threshold alone.

### 3.3 Measured quality (selected **HIGH**)

| Finding | Source |
|---------|--------|
| Observation Masking ≈ or > LLM-Summary on SWE-agent / SWE-bench Verified (e.g. Qwen3-Coder-480B **54.8%** mask vs **53.8%** summary; Gemini Flash thinking mask **36.4%** vs summary **31.4%**) | Complexity Trap, arXiv:2508.21433 |
| Cursor RL self-summary: **−50%** compaction error, summaries ~**1/5** tokens (~1k vs >5k prompt baseline) at 40k/80k triggers | Cursor self-summarization (2026) |
| CompactionRL: GLM-4.5-Air **+7.0** SWE-bench Verified under fixed budgets | arXiv:2607.05378 |
| ACON guideline optimization: **26–54%** peak-token cut while preserving success | arXiv:2510.00615 |
| Anthropic context editing **+29%**; editing+memory **+39%**; 100-turn search **−84%** tokens | Anthropic context management (2025) |
| Factory continuity probes: structured iterative > regenerate; artifact scores still weak (~2.2–2.5/5) for all vendors; ~**99%** ratio ≠ quality | Factory evaluating compression (2025) |
| LongLLMLingua: up to **+21.4%** NQ with ~**4×** fewer tokens (RAG/QA) | ACL 2024 |

**Rule:** Manage degradation (sometimes *improve* vs raw rot). Never claim compaction is quality-neutral without measurement.

### 3.4 What to keep vs drop (coding agents — **HIGH** pattern)

| Keep (verbatim or structured) | Drop / clear / externalize |
|-------------------------------|----------------------------|
| User goal / eng-goal id | Full file dumps already applied |
| Decisions & constraints | Redundant tool stdout |
| Failing tests / error excerpts | KB articles, draft essays |
| Paths / lease ids / owned_paths | Stale search hit lists |
| Next steps / open questions | Processed observation blobs (re-fetchable) |
| Recent N turns / last accessed files | Entire peer-agent transcripts |

Claude Code-style pattern: summarize critical details + keep a **small** set of recently accessed files.

---

## 4. Fertility — languages & formats

### 4.1 Fertility ≠ quality (**HIGH**)

**EMNLP 2024** — Schmidt et al., *Tokenization Is More Than Compression* (arXiv:2402.18376): PathPiece minimizes corpus token count; **no clear CTC ↔ accuracy** link across 64 trained LMs. Pre-tokenization and vocab construction matter more than “fewest tokens.”

**Zouhar et al., ACL 2023:** Rényi efficiency correlates with MT quality in some settings; **LREC-COLING 2024** shows Rényi can be **gamed** while quality drops.

**ADE rule:** Optimize fertility only inside a **quality floor** measured on gold.

### 4.2 Natural languages (English-centric BPE — **HIGH**)

Relative fertility vs English on FLORES-style setups (Petrov et al., NeurIPS 2023; GPT-4/`cl100k` line):

| Language | ≈ × English tokens |
|----------|-------------------|
| English | 1.00 |
| Portuguese / Spanish / German / French / Italian | ~1.5–1.6 |
| Chinese | ~1.9 |
| Japanese | ~2.3 |
| Vietnamese | ~2.5 |
| Arabic | ~3.0 |
| Extremes (some low-resource) | **12–15×** |

**Ahia et al., EMNLP 2023:** up to ~**5×** among 22 languages; over-fragmented languages also get worse utility.

**Character-level models flip the story** (CANINE can make Chinese cheaper than English) — irrelevant for ADE’s API frontier path.

**Do not:** translate product English → Chinese to “save tokens.” You pay fertility *and* often quality/OOD. **Do:** keep ADE operator language = language the **model is strongest at** for coding (usually English for current frontier coding SKUs).

### 4.3 Code languages (directional — **MEDIUM**)

No Petrov-class peer paper; cross-lang token-density measurements suggest approximate order for *same task*:

| Language | ≈ relative tokens | Note |
|----------|-------------------|------|
| Python | 1.00 | Dense, high BPE support |
| JavaScript | ~1.3 | |
| TypeScript / Java | ~1.45 | |
| Go / Rust | ~1.55–1.6 | Types + syntax can fragment |
| C | ~1.8 | |

**Harness implication:** Don’t rewrite Rust→Python for tokens. Emit **unified diffs / JSON Patch**, not whole files. Fertility gains from “language swap” are dominated by **I/O shape**.

### 4.4 Data & markup formats (**MEDIUM**)

| Format (as LLM text) | Density | Use when |
|----------------------|---------|----------|
| **TSV / CSV** | Best for flat tables | Tabular tool results |
| **Markdown tables** | Very good | Human+model readable tables |
| **Compact JSON** | Good | Nested structured state |
| **YAML / TOML** | OK–worse than compact JSON (~15–25% on nested) | Config familiarity |
| **Pretty-printed JSON** | Worse | Debug only — don’t rebill pretty |
| **XML / HTML tags** | Verbose | **Deliberate** boundaries (Anthropic-style) |
| **Raw HTML soup** | Worst | Strip/clean first |
| **MessagePack / protobuf binary** | Not for paste | Decode to compact text first |

**Reliability tax:** XML/JSON tags cost tokens but reduce parse ambiguity. ADE capsules should prefer **compact JSON or tight tagged sections** — not invent sigils.

### 4.5 Cross-tokenizer myths

| Claim | Verdict |
|-------|---------|
| Newer OpenAI tokenizers always use *more* tokens | **False** for many non-EN scripts (GPT-4o: large *reductions* vs prior) |
| Claude +~30% vs GPT-4o on some Python | **MEDIUM** third-party; measure yourself |
| “+30% Opus vs prior” blog claims | **LOW** until vendor-verified |

**Always** count with the **same** provider tokenizer as inference.

---

## 5. Could ADE invent its own language?

### 5.1 What people hope for

A dense “machineese”: short glyphs, stego codes, private BPE — so every turn ships fewer tokens with equal meaning.

### 5.2 Hard limits (**HIGH**)

1. **Frozen tokenizer** on hosted APIs — you cannot add merges or reserved “ADE tokens.”  
2. **Rare strings often *increase* fertility** — unknown glyphs → byte fallback → more tokens, not fewer.  
3. **Training support:** SQL, unified diff, JSON, regex, SVG paths work because corpora are saturated. Private ciphers are **OOD**.  
4. **Expansion tax:** models “explain” opaque codes → **output** tokens explode (often the expensive side).  
5. **Window still counts tokens** even if humans can’t read them.  
6. **Soft neural codecs** (GIST, ICAE, 500xCompressor-class) live in continuous/KV space with **matched training** — not pasteable into Claude/GPT chat.  
7. Telegraphic English / BabelTele-class densification tops out around ~**2–3.5×** with fidelity claims that are **pair-dependent (MEDIUM)** — not a free 10× codec.

### 5.3 Feasibility matrix for ADE

| Rank | Option | Call | Why |
|------|--------|------|-----|
| 1 | Catalog + ID refs (skills/rules by id) | **Best** | Already ADE DNA (T0–T3) |
| 2 | Structured handoff capsules (`ade.handoff/v1`) | **Best** | Continuity shipped; tighten sections |
| 3 | Diff-only / hunk I/O | **Excellent** | Pretraining saturated |
| 4 | Path / symbol indexes | **Excellent** | Replace dumps |
| 5 | Compact JSON capsules + verify pointers | **Excellent** | Fertility + parseability |
| 6 | Mild telegraphic rewrite inside known English | **Conditional** | ~2×; test fidelity |
| 7 | Train open-weight compressor↔consumer | **Niche** | Only if ADE hosts both ends |
| 8 | Private stego / invented BPE language | **Avoid** | OOD + expansion + no merge control |

### 5.4 What “ADE language” *should* mean

**Contract syntax**, not a cipher:

```
ade.handoff/v1 {
  goal_id, decisions[], failing[], paths[],
  next_steps[], verify_cmds[], lease_ids[]
}
```

Plus wire habits models already know: **unified diff**, **JSON Patch**, **`skill:<id>`** activation, **`path:`** indexes.

Optional later: open-weight **session compressor** that emits the same capsule schema — still not a new natural language.

### 5.5 Arguments FOR a custom language (steelman)

- Multi-agent handoffs could share a closed wire format with harness validation + NL escape hatch (**LOW** until measured).  
- Soft codecs are real science — if ADE someday ships a local model pair (**MEDIUM** for that path only).  
- Mild abbreviation conventions inside English (TE-class) can help if evaluated (**MEDIUM**).

### 5.6 Arguments AGAINST (decisive for API ADE)

- Structure already available in English+JSON+diff beats rare glyph soup (**HIGH**).  
- Expansion and OOD destroy the savings (**HIGH**).  
- Engineering time is better spent on **masking + capsules + boundaries** (see §7).

---

## 6. Real limits — LLMs · ADE · harnesses

| Layer | Can do | Cannot do |
|-------|--------|-----------|
| **LLM (API)** | Understand saturated formats; summarize; follow schemas | Learn your private cipher without fine-tune; escape context rot forever |
| **Tokenizer** | Fixed fertility map | Accept ADE-invented merges |
| **Harness** | Mask blobs; enforce schemas; externalize; Effort caps; Continuity | Make 200k window attend perfectly; bill fewer tokens than provider counts |
| **ADE Desktop** | Control plane for compact/continue; disk identity | Replace model weights |
| **Compaction** | Manage occupancy & drift | Be lossless for all artifacts (Factory: artifact trail stays weak) |
| **Fertility tricks** | Prefer dense formats / diffs | Guarantee higher task success |

**Context rot** remains: larger windows under-attend mid-context. Compaction recovers **attention budget**, not omniscience.

**Metacognition gap:** models are weak at knowing *when* their own context is rotting unprompted — scaffolds must supply the rubric (SelfCompact).

---

## 7. ADE mapping — today vs deep-dive bets

### 7.1 Shipped (spine)

| Piece | Role |
|-------|------|
| `ade.handoff/v1` + thrift Continuity | Tier-1 capsule + host `next_safe_command` |
| Soft system budgets (`context.rs`) | T0/rules/skills/handoff truncation |
| T0–T3 + `activate_skill` | Catalog > body (fertility + disclosure) |
| Effort = rounds + **output** tokens | Output discipline |
| Leases / goals / verify on disk | Tier-2 external memory seeds |

### 7.2 Gaps vs state of the art

| Gap | Research pointer |
|-----|------------------|
| No automatic **tool-result clearing** | Anthropic context editing; Complexity Trap masking |
| No mid-thread **history** compaction @~70% / boundary | SelfCompact; Cursor; vendor compact APIs |
| Capsule sections not yet a hard checklist artifact trail | Factory artifact weakness |
| No fertility / compaction **gold** metrics | Measure mask vs summary vs capsule |
| No SelfCompact **rubric tool** | arXiv:2606.23525 |
| chars÷4 unlabeled | Heuristic only |

### 7.3 Recommended ADE roadmap (Token Usage / Compaction track)

| Priority | Bet | Expected effect |
|----------|-----|-----------------|
| **C1** | Tool-result clearing / observation masking policy | Biggest cheap win; often ≥ summary quality |
| **C2** | Structured capsule compaction at **task boundary** + safety @~70% | Continuity upgrade; less drift than threshold-only |
| **C3** | Write-before-compact: goals/decisions/verify → `.ade/` every phase | Protects artifacts Factory shows everyone loses |
| **C4** | SelfCompact-style `compact_context` tool + rubric | Adaptive timing without fine-tune |
| **C5** | Gold: dual-writer free; mask vs summary vs capsule fidelity | Stops cargo-cult ratios |
| **C6** | Fertility bench on ADE corpus (EN capsules, diffs, JSON) | Catches tokenizer surprises |
| **Held** | Private ADE cipher / stego language | Fails hard-limit test |
| **Maybe** | Open-weight compressor pair | Only if ADE hosts both ends |

---

## 8. Practical playbook (operators + builders)

### Input

1. Catalog + IDs, not bodies.  
2. Compact JSON / MD tables for structured state; pretty-print only offline.  
3. Diffs/hunks, not full files.  
4. English (or model’s strongest coding language) for operator prose — don’t chase exotic fertility.  
5. Static → dynamic order (cache-stable).

### Compaction

1. Clear tool blobs first.  
2. Compact at **semantic boundaries**; threshold as backstop.  
3. Structured sections: intent · decisions · paths · failing · next · verify.  
4. Anchored merge > full regenerate when iterating capsules.  
5. Externalize before summarize.

### Output

1. Cap essays (Effort).  
2. Prefer tool calls + short acts.  
3. Verifiers emit pass/fail + pointers, not regenerated patches.

### Never

1. Invent opaque ADE glyphs for “compression.”  
2. Bill or optimize on chars÷4.  
3. Mid-task model swap to “fix” occupancy.  
4. Equate 99% compression ratio with success.

---

## 9. One diagram

```
                    ┌─ Fertility (tokenizer) ── frozen on API ─┐
 TEXT / DIFF/JSON ──┤                                         ├─→ TOKENS IN WINDOW
                    └─ Compaction (harness) ── ADE can own ────┘
                              │
              mask blobs · capsule · disk · Effort · Continuity
                              │
                    invent private language? ── usually NO
```

---

## 10. Conclusion

The real limits are clear:

- **LLMs** understand saturated languages and formats; they do not learn your cipher for free.  
- **Tokenizers** set fertility; ADE cannot rewrite them on Claude/GPT.  
- **Harnesses** own compaction — and that is where ADE should spend ambition.  
- **Best “ADE language”** is already half-built: **`ade.handoff/v1` + skill IDs + diffs + path indexes`**, tightened into a write-before-compact loop with masking and boundary triggers.

Compaction research says: **mask early, structure the summary, externalize always, time the compact by trajectory — not by inventing a denser tongue.**

---

## Appendix A — Selected primary sources

**Compaction / agents**  
- Anthropic: *Effective context engineering for AI agents* (2025); Compaction docs; Context management (+editing/memory).  
- OpenAI: Responses Compaction guide.  
- Cursor: *Training Composer for longer horizons* / self-summarization (2026).  
- Factory: *Evaluating Context Compression for AI Agents* (2025).  
- arXiv:2508.21433 — *The Complexity Trap* (observation masking).  
- arXiv:2606.23525 — *Self-Compacting Language Model Agents*.  
- arXiv:2607.05378 — CompactionRL.  
- arXiv:2510.00615 — ACON.  
- LongLLMLingua — ACL 2024.  
- MemAgent — arXiv:2507.02259.  

**Fertility / tokenization**  
- Petrov et al., NeurIPS 2023 — arXiv:2305.15425.  
- Ahia et al., EMNLP 2023.  
- Schmidt et al., EMNLP 2024 — *Tokenization Is More Than Compression* (arXiv:2402.18376).  
- Zouhar et al., ACL 2023 — Rényi efficiency.  
- Cognetta et al., LREC-COLING 2024 — Rényi gaming.  

**ADE internal**  
- `ADE-Tokenomics-IO-Context-Usage.md` · `TOKEN_ECONOMICS_RESEARCH.md` · `EFFORT_TURN_BUDGET_PLAN.md` · `crates/core/src/handoff.rs` · `crates/agents/src/context.rs`

## Appendix B — Confidence caveats

- Factory vendor ranking is first-party — treat as directional.  
- Code fertility table is **MEDIUM**; re-measure on ADE corpus.  
- LongLLMLingua/MemAgent transfer to full tool agents is **MEDIUM**.  
- SelfCompact numbers are paper-reported; dogfood on ADE gold before product claims.

---

*End · `ade.compaction-research/v1` · 2026-07-23*
