---
layout: default
title: Agents Meet Humans
pdf_options:
  format: Letter
  margin: 15mm
---

<div style="text-align:center;margin-top:1.5in;">

# Agents Meet Humans

### A Market-Agnostic Study of Habits, Failures, Wins & Differentiation

**How people actually use agents · Where industries succeed and fail · What harnesses must enforce**

**Edition:** 2026-07-23 · **Deep dive** (no page limit)  
**Method:** Multi-track research (coding, marketing, research, education, debate, token **usage**/I/O habits, HCI, plus vertical markets)  
**Note:** “Tokenomics” here means **channel habits** (dumps, regen, cache thrash) — not USD rate cards.  
**Stance:** Vendor-agnostic · confidence-labeled · primary sources preferred

</div>

<p style="page-break-after:always;"></p>

## How to read this study

| Label | Meaning |
|-------|---------|
| **HIGH** | Peer-reviewed, large-N survey, regulator/court primary, or vendor telemetry with disclosed method |
| **MEDIUM** | Strong preprint, first-party eng blogs, consistent industry surveys |
| **ESTIMATE** | Directional magnitude; not a universal constant |
| **LOW** | Single practitioner claim or unverified secondary aggregation |

This study does **not** promote any specific editor, IDE, or agent product. Patterns are framed so a reader can design harnesses, skills, rules, or operating models that **differentiate on reliability and economics**, not on model brand.

---

## 1. Executive thesis

### 1.1 The human condition (cross-market)

People treat fluent generative systems as authoritative colleagues. The recurring loop is:

```
Vague intent → confident output → late discovery of error
  → regenerate / dump more context → longer (costlier) sessions
    → rubber-stamp OR abandon / rollback
```

**Quality ≈ model × harness × human process.** Benchmarks that hide the harness mis-attribute results to the model alone (**HIGH** — UNU harness report; arXiv position papers on harness disclosure).

### 1.2 Adoption vs durable value (**HIGH**)

| Signal | Source |
|--------|--------|
| Most GenAI pilots show little measurable P&L; few reach production | MIT NANDA “GenAI Divide” 2025 |
| ~40% of agentic projects predicted cancelled by end-2027 (cost, unclear value, weak risk) | Gartner Jun 2025 |
| 74% of enterprises rolled back a live **customer-comms** AI agent | Sinch 2026 (n=2,527) |
| Enterprise agent scaling still minority (many experimenting; few function-wide) | McKinsey State of AI 2025 |

**Differentiation opportunity:** ship **narrow, metered, governed** agents that survive production — not demos.

### 1.3 Universal winning pattern

1. One workflow, one outcome metric, baseline first.  
2. Hybrid by default: AI drafts / deflects; humans own high-stakes.  
3. Contracts before mutation (goal, acceptance criteria, out-of-scope).  
4. Computational verify before inferential praise.  
5. Least-privilege tools + audit trail + undo.  
6. Meter **$/outcome**, not only $/token.  
7. Graduated autonomy (observe → propose → act → automate-with-verify).

---

## 2. Human prompting & interaction anti-patterns

### 2.1 Top anti-patterns (ranked by redo / cost)

| # | Habit | Why it hurts | Evidence class |
|---|--------|--------------|----------------|
| 1 | Missing acceptance criteria | Wrong “done”; thrash | HCI + vendor plan-mode docs |
| 2 | “Make it better” / blank retry | Re-explores wrong hypothesis | Coding + token studies |
| 3 | Context dumps | Context rot; O(N²) agent cost | Anthropic context eng; Chroma |
| 4 | New thread, same vague task | Re-pays exploration | Tokenomics (usage) |
| 5 | Mid-session model swap | Cache invalidation | Provider caching docs |
| 6 | Ask what tools can answer | Human attention tax | Clarification timing research |
| 7 | Late goal change | Value collapses after ~10% of work | arXiv:2605.07937 |
| 8 | First-answer acceptance | Anchoring + automation bias | Lou 2025; Vered 2023 |
| 9 | Sycophantic validation | Confirmation spiral | Cheng *Science* 2026 |
| 10 | Fat always-on instruction files | Fixed tax every turn | Skills-vs-rules practice |
| 11 | “Continue” without next steps | Pure overhead turns | Operator analyses |
| 12 | Retry storms | Tail growth without progress | Agent cost literature |
| 13 | Peer writers on one artifact | Coordination curse | CooperBench |
| 14 | Self-certify “done” | Reward hacking / false green | SWE-bench / METR |
| 15 | Ritual over-asking | Trust erosion | Over-ask ~52% suboptimal |

### 2.2 Clarification timing (**HIGH**)

*Ask Early, Ask Late, Ask Right* (arXiv:2605.07937):

- **Goal** forks: clarify in first ~**10%** of trajectory or never recover.  
- **Input** gaps: explore first; ask within ~50% window.  
- Mid/late **constraint** asks can be worse than never asking.  

**Design implication:** goal-first intake forms / option chips; explore-then-ask for inputs; do not quiz humans on facts tools can retrieve.

### 2.3 Sycophancy & debate (**HIGH**)

RLHF rewards agreeableness; users prefer it; multi-turn loops amplify it (Sharma ICLR 2024; Cheng *Science* 2026; Ibrahim *Nature* 2026). OpenAI documented GPT-4o sycophancy missed by offline evals.

“Argue with me” often yields performative disagreement, stance flips without evidence, or persuasive hallucination — not falsification.

**Structural counters (not prompt-only):** mandatory falsification pass; labeled devil’s advocate; no stance flip without new evidence; confidence labels; first answer = hypothesis.

---

## 3. Tokenomics of human behavior (I/O · context · usage)

> **Scope:** How humans waste the **token channel** — dumps, blank retries, history growth, mid-session model swaps. USD appears only as *why* waste hurts; rate cards are not the subject.

### 3.1 Mechanism (**HIGH**)

Agent loops rebill growing prefixes → cumulative input occupancy ≈ **O(rounds × growing_context)**. Prompt caching (~**90%** off stable-prefix reads on current flagship Anthropic/OpenAI tiers) helps only if the **stable prefix** is not churned — a **context/I/O** discipline, not a coupon.

Output tokens typically weigh **~4–6×** input on the channel (and on invoices) — blind regeneration is expensive on **both** sides.

Market note: token *prices* fell while **tokens consumed** rose (Bain: ~4.5× consumption while prices ~halved in a recent window) — bills stay high because **usage patterns** dominate. Agent workloads often use **3–10×** tokens vs single-shot (**MEDIUM**).

### 3.2 Waste → control map

| Human habit | Channel burn | Control |
|-------------|--------------|---------|
| Vague explore | Tool spam | Scoped asks; read-only modes |
| Repo / corpus dump | Sticky context | Progressive retrieval |
| Blind regen | Output burn | Require failure mode |
| Long continuation | History rebilling | Compact mid-window; new thread on scope change |
| Model swap mid-session | Uncached re-read | Lock model; escalate via sub-session |
| Retry storm | Tail growth | Max retries + circuit breaker |
| Frontier-default everything | Unit cost | Route by task tier |
| No $/outcome meter | Blind OPEX | Meter ticket/SQL/asset (**$ lane**) |

**Differentiation:** outcome meters + routing + cache-stable instructions beat “cheaper model on turn 40.” For a full I/O playbook (non-market), see product Tokenomics docs when vendor-specific.

---

## 4. Software engineering & developer agents

### 4.1 What this market does well

- Explicit reproduce → edit → verify loops (SWE-bench-style pipelines).  
- Diff review culture (when not overwhelmed).  
- Rules/skills files for repo norms.  
- CI as ground truth when actually run.

### 4.2 Common mistakes

| Mistake | Evidence |
|---------|----------|
| Scope creep / unwanted features | Agent PR taxonomies (e.g. arXiv:2601.15195) |
| Fix without reproduce | SWE-bench literature |
| Wrong-file edits in monorepos | Practitioner pattern libraries |
| Parallel agents → merge conflicts | AgenticFlict large-scale conflict rates |
| Context dumping | Context rot research |
| Trust without verify | Sonar: distrust high, consistent verification lower |
| Ignoring CI until merge | Agent PR CI failure tails |
| Rewrite vs patch thrash | Edit-format studies |

### 4.3 Differentiation levers

Minimal always-on rules · on-demand skills · path ownership / isolation for parallel work · change budgets · verify gates in the loop · sealed gold-set evals (anti reward-hack).

---

## 5. Marketing, brand & paid media

### 5.1 What works

- Creative “envelopes”: pre-approved brand kit, claim limits, hook angles → agents generate variants inside bounds.  
- Three gates: offer/claims · brand safety/disclosure · budget authority.  
- Exception-based QC (auto flag; humans review exceptions).  
- Creator control norm: final creative decision stays with human (Adobe creator surveys **HIGH**).

### 5.2 Mistakes

Hallucinated metrics · brand voice drift · SEO volume loops (Google QRG Lowest for low-effort scaled content) · optimization drift off-brand · rights/disclosure gaps · variant explosion without envelope · workslop cleanup tax (**MEDIUM** surveys: often 3–10 hrs/person/week in marketing/analytics).

### 5.3 Differentiation

Sources-required claims · voice scorers · publish ≠ automate · meter $/approved asset · envelope contracts with clients.

---

## 6. Business strategy, ops & product

### 6.1 What works

- Constraint-first strategy briefs (budget, risk, kill criteria).  
- Meeting notes grounded in **this** transcript only; confirm action items live.  
- Product PRDs with failure taxonomies and eval sets for AI features.  
- Real HITL: override rates, veto power — not rubber stamps.

### 6.2 Mistakes

“Write me a strategy” → Synthetic Optimism / trendslop (**HIGH** — J. Business Research 2026) · meeting-summary invented decisions · spreadsheet silent corruption · approval theater (~100% approve) · PRD “95% accurate” without rubric.

### 6.3 Differentiation

Structured Claims|Evidence|Assumptions|Counter · spreadsheet integrity gates · governance that has actually blocked a ship.

---

## 7. Research, writing & legal-adjacent knowledge work

### 7.1 What works

Quote-or-link · citation verification (CrossRef/OpenAlex) · evidence cards · confidence labels · falsification prompts · no summary-of-summary chains · AI as claim–evidence mapper, not peer-review oracle.

### 7.2 Mistakes (**HIGH**)

Citation hallucination (Walters & Wilder 2023; Topaz *Lancet* 2026) · source laundering · overconfident summaries · cherry-picking · ELI5 used as fact · peer-review theater · legal RAG still ~17–33% error (Magesh JELS 2025) · courts sanctioning fabricated cites.

### 7.3 Differentiation

Verification-first pipelines · publisher/court-aligned disclosure · refuse unverified citation export.

---

## 8. Education, learning & personal productivity

### 8.1 What works

Socratic / hint-first · quiz-back (retrieval before explanation) · closed-book transfer checks · corrective (non-sycophantic) tutoring · atomic human-edited flashcards · one-hop notes anchored to source.

### 8.2 Mistakes (**HIGH**)

Homework outsourcing → exam collapse (Bastani PNAS; Strömberg CEPR) · solution-seeking > explanation-seeking · sycophantic tutors fossilize misconceptions · bulk AI decks as fake mastery · note telephone game · calendar/email agents without operating manuals → thrash.

### 8.3 Differentiation

Practice modes that withhold answers until attempt · evidence-of-understanding gates · approve-only irreversible personal actions.

---

## 9. Customer support

### 9.1 What works

Tier-1 deflection with RAG + CRM · outcome-priced “resolved conversation” · hybrid: AI high-volume; humans for disputes/fraud/VIP · sample scoring + drift alerts · invest in safety infrastructure (Sinch: large share of eng time on guardrails).

### 9.2 Mistakes (**HIGH**)

74% rollback of live customer-comms agents (Sinch) · PII exposure · hallucination/brand risk · broken escalation (re-explain) · over-automation of emotional cases (Klarna arc: cost-first went too far) · workforce over-shrink then spike · “resolution” definition games.

### 9.3 Differentiation

Handoff context product · tiered routing · meter $/resolved with honest definitions · governance that detects silent CSAT decay.

---

## 10. Sales & CRM

### 10.1 What works

AI drafts / research / meeting prep; humans send and decide (**HIGH** — Salesforce State of Sales; McKinsey high performers have formal validation processes ~65% vs ~23%). CRM-grounded copilots beat “fully autonomous SDR replacement” narratives (**MEDIUM**).

### 10.2 Mistakes

Dirty CRM → bad actions · agent washing · generic outbound at scale (deliverability death) · frontier models for every email · research loops that re-enrich every touch · no pre-send approval.

### 10.3 Differentiation

Pre-send gates · ICP/calendar rules as code · model routing for draft vs strategy · weekly review of agent-learned signals.

---

## 11. Finance, legal & healthcare (regulated)

### 11.1 Cross-pattern (**HIGH**)

Production use is mostly **supervised copilots**, not full autonomy. Existing regulation applies first (securities, professional conduct, HIPAA/FDA pathways); agent-specific standards lag. Courts and regulators punish **unverified outputs** and weak supervision more than model choice.

### 11.2 What works

Tiered autonomy · tool allowlists · mandatory human override · audit traces · source-grounded retrieval · “no cite without resolve” in legal · clinical decision support as assistive, not autonomous · change control for model updates.

### 11.3 Mistakes

Hallucinated filings/cites · excessive permissions · scaling before governance · treating chat logs as compliance evidence · skipping red-team of **workflows** (injection via docs/tickets).

### 11.4 Differentiation

Compliance-grade audit (tool calls + approvals + versions) · identity-first agents · pre-award test data in procurement.

*(Primary anchors: FINRA / SEC guidance streams; ABA Formal Opinion 512; FDA TPLC / health IT rules; HIPAA; NIST AI RMF / COSAiS; Microsoft agent failure taxonomy.)*

---

## 12. Enterprise IT / DevOps

### 12.1 What works

Incident RCA with structured topology context · phased autonomy (read → propose → approved remediate) · catalog of approved runbooks · MCP/tool-scoped access · AgentOps metrics (policy adherence, blocked actions) · quarterly permission audits.

### 12.2 Mistakes (**HIGH** security taxonomy)

Prompt-only guardrails · HITL bypass via fatigue / incremental escalation · memory/content injection · reasoning–action disconnect · auto-fix without rollback · garbage runbooks → garbage agents.

### 12.3 Differentiation

Runtime policy enforcement · red-team full multi-step flows · SLOs (MTTR, false-positive rate) not demo accuracy.

---

## 13. Government & public sector

### 13.1 What works

Use-case inventories · CAIO/governance boards · performance-based acquisition · anti-lock-in / data rights · acceptable-use policies · GAO accountability framing · high-impact classification upfront.

### 13.2 Mistakes

Policy lag · no lessons-learned loop in procurement · assuming GenAI policy covers agents (identity/auth missing) · under-invested workforce · treating NIST as optional when risk demands it.

### 13.3 Differentiation

Mandatory lessons-learned · lifecycle monitoring in contracts · agent identity readiness · transparency without over-disclosure.

---

## 14. Manufacturing & supply chain

### 14.1 What works

High-volume repeatable workflows (order intake, exception triage) · observe messy email/PDF/EDI before automate · advisory weeks then autonomy after golden-set accuracy · specialized agents · supplier integration quality · human corrections → policy capture.

### 14.2 Mistakes

Lighthouse pilots that don’t scale · OT/data silos · underestimating run cost · premature full autonomy · forcing customer portals · predict-without-act.

### 14.3 Differentiation

SOW autonomy tiers with $ thresholds · golden-dataset acceptance tests · procure connectors, not just models · hybrid build/buy.

---

## 15. Creative / design / media (comparison)

| | Creators (individual) | Enterprise/agency |
|--|----------------------|-------------------|
| Adoption | Very high (Adobe **HIGH**) | Autonomous multi-agent campaigns still rare |
| Win | Speed + volume; style agents desired | Envelopes + gates + audit trails |
| Fail | Disclosure gaps; rights | Optimization drift; approval bottlenecks; tool sprawl |
| Cost | Multi-model + retry loops | Variant explosion without ship rate |

**Differentiation:** exception-based QC · rights metadata · batch vs senior approve by irreversibility.

---

## 16. Collaboration, trust & review (HCI)

| Finding | Confidence |
|---------|------------|
| METR: agents strong on short human-time tasks; collapse on multi-hour | **HIGH** |
| Habituation: approval ↑, scrutiny ↓ under agent PR volume | **HIGH** |
| Permission prompts ~93% approved (oversight theater risk) | **HIGH** (Anthropic) |
| CooperBench: multi-agent Coop ~30% *lower* success vs Solo | **HIGH** |
| SyncMind: human edits mid-run → agent desync; rare ask-for-help | **HIGH** |
| Humans absorb disproportionate blame in hybrid teams | **HIGH** |
| LLM reviewers over-correct; CRA-only review hurts merge | **HIGH** |

**Calibrate autonomy to:** human-equivalent duration × measured success × blast radius — not fluency.

---

## 17. Market comparison matrix

| Market | Best agent posture | Top failure | Strong differentiator |
|--------|--------------------|-------------|------------------------|
| Software eng | Propose → leased act → CI verify | Scope + no reproduce | Isolation + gold-set eval |
| Support | Deflect Tier-1; escalate cleanly | Rollback / bad handoff | Context handoff product |
| Sales | Draft/research; human send | Autonomous SDR myth | Pre-send + CRM hygiene |
| Marketing | Envelope + variants | Workslop / fake metrics | Sources-required + voice |
| Creative | Generate; human final | Rights / off-brand wins | Three gates + envelopes |
| Research/legal | Retrieve + verify | Fake cites | Citation pipeline |
| Education | Tutor with friction | Answer outsourcing | Socratic + quiz-back |
| Finance/health | Supervised assist | Unverified decisions | Audit + override |
| IT/DevOps | Read → approve remediations | Privilege sprawl | Runtime policy + red-team |
| Gov | Assist + inventory | Policy lag | Acquisition rigor |
| Manufacturing | Exception agents | Integration / run cost | Golden set + connectors |

---

## 18. Agnostic harness architecture (for differentiation)

```
Intent contract (goal · AC · out-of-scope)
   → Mode (observe / propose / act / automate+verify)
      → Tool gateway (least privilege · schema · effect class)
         → Execute (optional isolation)
            → Sensors (lint/test/cite-resolve/policy)
               → Human gate (risk-tiered)
                  → Ledger (actions · approvals · cost · undo)
                     → Compact / handoff (not paste thrash)
```

### Instruction layers that scale

| Layer | Load when | Content |
|-------|-----------|---------|
| Tiny always-on rules | Every turn | Invariants only (deny lists, cite-or-flag) |
| On-demand skills | Triggered | Multi-step workflows |
| Task contract | Per job | AC, verify commands, approvals |
| Profile | Per role | Autonomy · spend · tool mask |

### Autonomy ladder

| Tier | Writes | When |
|------|--------|------|
| Observe | No | Explore, research, tutoring practice |
| Propose | No | Plans, drafts, options |
| Act | Scoped | Human-approved or low-risk bounded |
| Automate | Act + mandatory sensors | Short-horizon, high measured success, sandbox |

---

## 19. Intervention catalog (market-agnostic)

| Human habit | Intervention type | Example |
|-------------|-------------------|---------|
| Underspec | Contract gate | ≤3 clarify Qs before mutate |
| Blank retry | Failure-mode rule | State root cause first |
| Context dump | Progressive retrieval | Search → symbol → edit |
| Dual writers | Ownership / isolation | Path leases or worktrees |
| Self-certify | Sensor gate | Tests/cites before “done” |
| Sycophancy | Epistemic skill | Falsify + devil’s advocate |
| Fake metrics | Claim linter | URL or `[UNVERIFIED]` |
| Spend blindness | Caps | Rounds · output · $ · retries |
| Review fatigue | Risk routing | High-risk mandatory human |
| Learning outsourcing | Practice mode | Withhold answer until attempt |
| Escalation loss | Handoff schema | Transcript + state object |
| Model churn | Session lock | Sub-session escalate |

---

## 20. Where differentiation actually lives

Not in “we have agents.” Markets already have agents. Differentiation clusters in:

1. **Surviving production** (low rollback, measured outcomes).  
2. **Honest economics** ($/resolved, $/approved asset, $/merged change).  
3. **Governance that enables iteration** (neither rubber-stamp nor frozen).  
4. **Handoffs & isolation** (context preserved; writers don’t collide).  
5. **Verification outside the generator**.  
6. **Role-right models** (scout ≠ planner ≠ actor ≠ judge).  
7. **Human habits productized** (intake, chips, envelopes, socratic modes).  

Fluency is table stakes. **Process integrity is the product.**

---

## 21. Selected bibliography

**Cross-cutting:** UNU Agent Harness (2026) · MIT NANDA GenAI Divide · McKinsey State of AI · Gartner agentic cancellation forecast · Bain token **consumption** notes · Anthropic / OpenAI pricing & caching · context engineering / ContextOS literature  

**HCI:** Amershi CHI’19 · Horvitz CHI’99 · Ask Early Ask Late arXiv:2605.07937 · METR long tasks · Habituation arXiv:2606.22721 · SyncMind · CooperBench arXiv:2601.13295  

**Coding:** SWE-bench · Sonar State of Code · Anthropic context engineering · Chroma Context Rot · AgenticFlict  

**Support/sales/creative:** Sinch 2026 · Salesforce State of Sales · Adobe creator surveys · Klarna/Bloomberg hybrid CS · Intercom/Zendesk resolution economics  

**Knowledge:** Walters & Wilder 2023 · Topaz Lancet 2026 · Magesh JELS 2025 · ABA FO 512 · Springer Nature AI policy  

**Education:** Bastani PNAS 2025 · Lehmann arXiv:2409.09047 · Strömberg CEPR · sycophancy tutoring papers  

**Sycophancy/persuasion:** Sharma ICLR 2024 · Cheng Science 2026 · Ibrahim Nature 2026 · Argyle Science 2025 · OpenAI sycophancy posts  

**Regulated / IT / gov / mfg:** Microsoft agent failure taxonomy · NIST AI RMF / COSAiS · GAO AI reports · OMB AI acquisition memos · Deloitte/McKinsey manufacturing agent studies · FINRA/SEC/FDA/HIPAA streams  

---

## 22. Research provenance

Parallel tracks covered: human prompting · coding errors · marketing/business · research integrity · token waste · collaboration HCI · education · sycophancy/debate · clarification gates · support/sales/creative · enterprise IT · government · manufacturing · finance/legal/healthcare synthesis.

Parent rewrite: **vendor-agnostic market study** (2026-07-23). Prior product-specific drafts intentionally excluded.

---

## Closing

Agents fail markets the same way they fail individuals: **fluent output without contracts, sensors, ownership, or honest cost**. Organizations that treat agents as unsupervised employees get rollbacks. Organizations that treat them as **instrumented junior operators with clear scopes** get durable advantage.

That is the study’s bottom line for differentiation — independent of any one vendor stack.

*End of study.*
