---
layout: default
title: TOKEN ECONOMICS RESEARCH
---

# Economic Tokenization — Science, Evidence, ADE Mapping

**Schema:** `ade.token-econ-research/v1`  
**Status:** Research synthesis · 2026-07-22 · **vocab revised 2026-07-23**  
**Canvas:** `ADE-token-economics-research.canvas.tsx`  
**Companions:** `EFFORT_TURN_BUDGET_PLAN.md` · `money.rs` · `spend.rs`  
**Product PDF (Tokenomics = I/O·context·usage):** `docs/research/ADE-Tokenomics-IO-Context-Usage.md`  
**Deep dive (compaction · fertility · custom language limits):** `docs/research/ADE-Token-Usage-Compaction-Fertility-Deep-Dive.md`

## Vocabulary lock

| Say | Mean |
|-----|------|
| **Tokenomics** | Input composition, output discipline, context hygiene, usage patterns |
| **SpendGuard / $ metering** | USD caps, rates, reserve/reconcile, invoice honesty |
| **Effort** | Per-turn rounds + **output** token gas tank |

Do not use “tokenomics” for pricing tables alone.

## Thesis

A tokenizer is a **discrete information channel** into the model. Vendors
**meter that channel**. Hosts like ADE cannot rewrite BPE/Unigram for a hosted
model — they can (1) choose models by fertility on their corpus, (2) structure
prompts for cache hits, (3) thrift agent loops, and (4) account money with
integer micros against **provider-reported** usage.

**Compression is not quality.** EMNLP 2024 (“Tokenization Is More Than
Compression”) finds no clear corpus-token-count ↔ accuracy link across many
configs. Intrinsic metrics (Rényi efficiency, Zouhar ACL 2023) correlate in
some MT settings but **can be gamed** (LREC-COLING 2024 counterexamples).
Extrinsic eval remains the judge.

## Economic facts with strong evidence

1. **Output tokens cost ~3–8× input** on major APIs → constrain outputs first.
2. **Prompt caching** can cut input cost ~40–90% when the stable prefix is
   large, first, and cacheable; dynamic tool junk must not break the prefix.
3. **Agentic multiply** — system prompt × rounds dominates bills before useful
   work; host-side Continuity thrift beats hoping the model “discovers” less.
4. **chars÷4 is not a bill** — use only for local compaction heuristics.

## Ranked methods (host-enforceable)

| Rank | Method | Why |
|------|--------|-----|
| 1 | Bill provider usage only | Invoice identity |
| 2 | Exact Money (micros + ceil) | No float undercharge |
| 3 | Reserve → reconcile | Hard caps without silent overrun |
| 4 | Cache-stable prompt order | Structural input discount |
| 5 | Orthogonal budgets (rounds / out-tokens / $) | Right failure mode |
| 6 | Fertility-aware model pick | Same text, fewer tokens |
| 7 | Train-time tokenizer pick | N/A for ADE consumer path |

## ADE today

**Shipped:** `Money` micros, `cost_for_tokens` ceiling, SpendGuard
reserve/reconcile, Effort tool-round + output-token caps, Continuity dogfood
with host `next_safe_command`.

**Gaps:** Budget stops labeled Provider (B0); free $0 rates weaken caps;
pessimistic full-context reserve each round; no cache token ledger fields;
chars÷4 unlabeled in UI; no fertility gold bench.

## Proof experiments

See canvas §7 (E1–E5): invoice match, cache honesty, fertility bench, agent
thrift, cap orthogonality.
