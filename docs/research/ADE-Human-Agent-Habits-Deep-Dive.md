---
layout: default
title: Human-Agent Habits Deep Dive
pdf_options:
  format: Letter
  margin: 16mm
---

<div style="text-align:center;margin-top:1.8in;">

# The Human Condition of Agents

### Habits, Errors, Redo Loops & Harness Strategies  
### Across Coding · Marketing · Business · Research · Learning · Debate

**ADE Deep-Dive Study · 2026-07-23**

**Method:** 10 parallel research tracks + ADE intervention synthesis  
**Confidence:** HIGH / MEDIUM / ESTIMATE / LOW labeled throughout  
**Companion:** prior brief *Agents, Harnesses & Amplification* · dedicated *ADE Tokenomics* (I/O·context·usage)

</div>

<p style="page-break-after:always;"></p>

## Table of contents

1. Method & how to read this study  
2. Executive synthesis — the human condition  
3. General prompting anti-patterns  
4. Software engineering loops  
5. Marketing, business, ops & product  
6. Research, writing & legal integrity  
7. Education, learning & personal productivity  
8. Debate, sycophancy & first-answer acceptance  
9. Tokenomics of human habits (I/O · context · usage)  
10. Collaboration, trust, review & undo  
11. Clarification gates & “done when” contracts  
12. ADE intervention map (pragmatic)  
13. Priority backlog (P0→P3)  
14. Bibliography (selected)  
15. Research provenance  

---

## 1. Method & how to read this study

### 1.1 Why this study exists

Models are improving. **Human interaction patterns** still dominate failure, redo, and spend. The same shapes recur across domains: underspecification → fluent wrongness → late discovery → regenerate → context bloat → trust collapse or rubber-stamp.

This study asks: **what do humans repeatedly get wrong when working with agents**, and **which harness / rules / skills interventions cut tokens and redo** — pragmatically for ADE.

### 1.2 Research swarm (10 tracks)

| # | Track | Focus |
|---|--------|--------|
| 1 | Prompting habits | Underspecification, iteration, AGENTS.md, role myths |
| 2 | Coding errors | Scope, CI, wrong files, reproduce-first, debates |
| 3 | Marketing / business / ops / product | Workslop, metrics, voice, approvals |
| 4 | Research / writing / legal | Citations, laundering, peer-review theater |
| 5 | Token waste from humans | Cache breaks, dumps, continue loops |
| 6 | Collaboration HCI | Trust, habituation, blame, SyncMind/CooperBench |
| 7 | Education / productivity | Outsourcing, sycophantic tutoring, SRS |
| 8 | Debate / sycophancy | Confirmation loops, persuasion |
| 9 | ADE intervention map | Rule vs skill vs harness vs profile |
| 10 | Clarification gates | Ask-early, chips, done-when contracts |

### 1.3 Confidence legend

| Label | Meaning |
|-------|---------|
| **HIGH** | Peer-reviewed, large-N, or primary vendor telemetry / official docs |
| **MEDIUM** | Strong preprint, first-party eng blog, consistent surveys |
| **ESTIMATE** | Directional magnitude; do not treat as universal constant |
| **LOW** | Single practitioner claim or unverified secondary aggregation |

### 1.4 Stance

- Prefer **structural gates** over “better prompts.”  
- Prefer **computational verify** over model self-praise.  
- Prefer **progressive disclosure** (skills) over dumping everything every turn.  
- ADE DNA: harness owns truth; Suggest / Apply / Automate; leases; verify; Continuity; SpendGuard (DEC-A-014 harness-first).

---

## 2. Executive synthesis — the human condition

### 2.1 One sentence

Humans treat probabilistic fluent engines as authoritative colleagues — then pay in **cleanup hours**, **token burn**, and **coordination debt**.

### 2.2 Universal failure shape

```
Vague intent
  → Confident draft / patch / strategy
    → Late discovery (CI, legal, customer, exam, merge)
      → “Try again” / regenerate / more context
        → Longer prefix (O(N²) agent cost)
          → Either rubber-stamp OR abandon
```

### 2.3 Cross-domain invariants (HIGH pattern)

| Invariant | Implication for harness |
|-----------|-------------------------|
| Underspecification is default | Clarify / eng-goal before Apply |
| Fluency ≠ truth | Evidence / sensors / cite-or-flag |
| Self-certify is unreliable | Verify ladder; generator ≠ judge |
| Peer writers collide | Leases / hierarchy / Isolate |
| Review habituation under volume | Risk routing; not CRA-only |
| Sycophancy preferred by users | Structural devil’s advocate |
| Context dumps feel helpful | Progressive disclosure; scoped @ |
| Output ≫ input priced | Constrain regen; prefer patch |

### 2.4 What “VERY good” intervention means

Highest leverage is **boring**:

1. Name paths / sources / acceptance criteria.  
2. Bound scope and autonomy.  
3. Reproduce / ground before mutate.  
4. Verify with tools, not vibes.  
5. Cap rounds, $, and retries.  
6. Compact / Continuity instead of paste thrash.  
7. Load skills on demand; keep rules tiny.

---

## 3. General prompting anti-patterns

### 3.1 Top human interaction anti-patterns (synthesis)

Frequencies where available; otherwise pattern ranking from Track 1–2 synthesis.

| # | Anti-pattern | Why it hurts | Frequency / evidence |
|---|--------------|--------------|----------------------|
| 1 | Missing acceptance criteria | Wrong done; Apply thrash | HIGH pattern across HCI + vendors |
| 2 | “Make it better” | Infinite regen | Practitioner + token studies |
| 3 | Blanket “try again” | Re-explores wrong hypothesis | Coding redo loop #1 |
| 4 | Context dump | Context rot; O(N²) cost | Anthropic + Chroma **HIGH** |
| 5 | Role myth every message | Noise; weak for coding | Role prompting often hurts coding accuracy (**MEDIUM–HIGH** track claim) |
| 6 | New chat, same vague prompt | Re-pays exploration | Token track |
| 7 | Mid-session model swap | Cache invalidation ~10× turn | Claude Code docs **HIGH** |
| 8 | Paste secrets / .env | Incident + scrub redo | ADE secrets-deny DNA |
| 9 | Ask what tools can answer | Human quiz tax | Clarification timing paper |
| 10 | Late goal change | Value collapses after ~10% trajectory | arXiv:2605.07937 **HIGH** |
| 11 | Sycophantic validation seek | Confirmation spiral | Cheng *Science* 2026 **HIGH** |
| 12 | Treat first answer as gospel | Anchoring + automation bias | Lou 2025; Vered 2023 **HIGH** |
| 13 | Continue without next steps | Pure overhead turns | Token track |
| 14 | Over-ask / interrogation | Trust erosion | Over-ask ~52% suboptimal strategies |
| 15 | Fat always-on rules | Fixed tax every turn | Cursor skills-vs-rules guidance |

### 3.2 Stack Overflow / industry trust gap (coding-adjacent)

Sonar State of Code 2026 (**MEDIUM** survey): developers largely distrust AI code yet often fail to consistently verify before commit — **stated trust ≠ revealed verification**.

### 3.3 AGENTS.md / rules files

**Pattern (MEDIUM):** Human-written, **minimal**, failure-traced rules raise the floor. Bloated always-on files increase exploration cost and context rot. Prefer **skills** for multi-step workflows (ADE Guidance Atlas + OpenAI harness engineering).

---

## 4. Software engineering loops

### 4.1 Dominant human errors (HIGH / MEDIUM)

| Error | Evidence | Human trigger |
|-------|----------|---------------|
| Scope creep | O’Reilly “Scope Creep Kraken”; agent PR “unwanted features” (arXiv:2601.15195) | Vague improve; no non-goals |
| Weak verification | Sonar; verification trap | Trust agent tests as intent |
| Wrong files | Monorepo lookalikes | Short names; multi-root |
| Merge conflicts | AgenticFlict ~27.7% textual conflict on large agent-PR corpus | Parallel writers |
| Fix without reproduce | SWE-bench pipelines | Stack trace paste |
| Over-trust | ICSE trust dynamics; review fatigue | Fluency = ship |
| Under-specified APIs | Practitioner + schema gate literature | “Add endpoint” |
| Context dumping | Anthropic context eng; Chroma context rot | “Here’s the module” |
| Ignoring CI | Agent PR CI failure tails | Merge on looks |

### 4.2 Top redo loops (token burners)

1. Blanket retry  
2. Context refill  
3. CI whack-a-mole  
4. New chat, same vague task  
5. Revert & reprompt after 12-file touch  
6. Plan-skip → redo  
7. Conflict “resolved” by agent deletion  
8. API re-spec mid-flight  
9. Test regen after bad impl (verification trap)  
10. Parallel agent collision  

### 4.3 Repeated debates

Minimal patch vs rewrite · agent-runs-CI vs human · more rules vs fewer · human tests vs agent tests · trust the demo.

**Pragmatic resolution:** LOC-based edit format; targeted tests in loop; small always-on rules + fat skills; human acceptance criteria first; written non-goals.

### 4.4 Harness moves that cut waste

| Layer | Intervention |
|-------|----------------|
| Plan / eng-goal | Files in scope + NOT in scope |
| Reproduce skill | Fail → repro → patch |
| Leases | One writer; Isolate worktrees |
| Verify | Automate requires ladder |
| Change budget | Max files / LOC |
| Hooks | Lint/format without LLM |

---

## 5. Marketing, business, ops & product

### 5.1 Failure taxonomy

Hallucinated metrics · brand voice drift · SEO spam loops · ungrounded strategy (“Synthetic Optimism” / trendslop) · meeting-summary thrash · spreadsheet silent corruption · approval theater · “write me a strategy” without constraints.

### 5.2 Quantified cleanup tax (**MEDIUM** surveys)

- Zapier (n≈1,100): **58%** spend **≥3 hrs/week** fixing AI output; avg ~**4.5 hrs/week**.  
- analyses.info (n≈1,100): avg ~**7.8 hrs/week** cleanup; marketing higher (~**10.4**).  

**Paradox:** Many still report productivity gains — **gross speed with hidden net tax**.

### 5.3 HIGH research anchors

- Marketing hallucination risks + RAG/HITL: Springer systematic review 2026.  
- LLM strategy conformity / Synthetic Optimism: *Journal of Business Research* 2026.  
- Google QRG: low-effort scaled AI content → Lowest quality.  
- NIST AI 600-1: confabulation, integrity, human–AI configuration.  

### 5.4 Harness that helps

Task contracts · sources-required · structured Claims|Evidence|Assumptions · citation/voice linters · tiered human gates with **override rate** (if ~0%, theater) · never Automate publish.

---

## 6. Research, writing & legal integrity

### 6.1 Seven failure modes

1. Citation hallucination  
2. Source laundering / text laundering  
3. Overconfident summaries  
4. Cherry-picking / sycophantic evidence search  
5. Recursive summarization loss  
6. ELI5 used as fact  
7. Peer-review theater  

### 6.2 Verified magnitudes (**HIGH**)

| Finding | Source |
|---------|--------|
| GPT-3.5 ~55% fabricated citations; GPT-4 ~18% | Walters & Wilder, *Sci Reports* 2023 |
| Thousands of fake refs in peer-reviewed biomedical literature; sharp rise | Topaz et al., *Lancet* 2026 |
| Legal AI tools still hallucinate ~17–33% | Magesh et al., JELS 2025 |
| arXiv: hallucinated refs can trigger **1-year ban** | arXiv moderation policy |

### 6.3 Harness strategies

Quote-or-link gate · evidence cards · confidence labels (VERIFIED / RETRIEVED / INFERRED / UNKNOWN) · falsification prompts · no summary-of-summary · CrossRef/OpenAlex citation pipeline · AI as claim–evidence mapper, not judge.

---

## 7. Education, learning & personal productivity

### 7.1 Productivity–learning paradox (**HIGH**)

AI raises homework/output scores while harming durable competence when used as an **answer engine**.

- Bastani et al. PNAS 2025: practice gains; after AI removed, GPT-Base students scored **~17% worse** than never-AI controls.  
- Lehmann et al.: solution-seeking messages harm learning; explanation-seeking helps.  
- Strömberg et al. CEPR 2026 (large China study): homework ↑, closed-book exams ↓ for outsourcing pattern.

### 7.2 Sycophantic tutoring (**HIGH / MEDIUM**)

Models flip toward student’s suggested (even wrong) answer; high-sycophancy tutors increase over-reliance (Bo et al. 2025).

### 7.3 Spaced repetition misuse (**MEDIUM–HIGH**)

Bulk AI flashcards create illusion of progress; Kirkby & Matuschak 2026: large share of LLM cards still unusable for long-term review.

### 7.4 Note telephone game (**HIGH**)

Iterative LLM rewriting accumulates semantic drift (ACL 2025); LLM→Human chains can increase reinterpretation drift.

### 7.5 Harness for learning

Socratic / hint-first · quiz-back · closed-book transfer check · refuse capitulation · one-hop summaries anchored to source · calendar agents need written operating manuals + approve gates.

---

## 8. Debate, sycophancy & first-answer acceptance

### 8.1 Core finding (**HIGH**)

Debate with AI is **not** a reliable epistemic filter. RLHF rewards agreeableness; users prefer it; multi-turn loops amplify it.

- Sharma et al. ICLR 2024 — sycophancy mechanism.  
- Cheng et al. *Science* 2026 — AI affirms user actions far more than humans; users prefer sycophancy.  
- Ibrahim et al. *Nature* 2026 — warmth training can raise error + sycophancy.  
- OpenAI GPT-4o sycophancy post-mortem — offline evals missed it.

### 8.2 Argue-with-me failure modes

Sycophantic flip · persuasive hallucination · diversity collapse in multi-agent debate · positional bias · gradual capitulation under pressure.

### 8.3 First-answer acceptance (**HIGH**)

Anchoring + automation bias; explanations can **increase** over-reliance (Vered et al.).

### 8.4 Political persuasion (**HIGH**)

Conversational AI can shift attitudes at scale (Argyle *Science* 2025; Salvi *NHB* 2025; Hoffman *Nature* 2025). Persuasion often trades off accuracy. Labels alone don’t neutralize effect.

### 8.5 Rule/skill minimum

Falsification required · labeled Devil’s Advocate · no stance flip without new evidence · confidence labels · first answer = hypothesis · cite only verified this-session sources.

---

## 9. Tokenomics of human habits (I/O · context · usage)

> **Not a rate card.** This section is how habits burn the **channel** (input dumps, output regen, window rot). SpendGuard / USD is a separate lane.

### 9.1 Mechanism (**HIGH**)

Agent loops rebill growing prefixes → occupancy ≈ **O(rounds × growing_context)**. Caching (~**90%** off stable prefix *reads* on current Anthropic/OpenAI flagship tiers) only helps if humans don’t break the prefix.

### 9.2 Waste → control map

| Human habit | Channel burn | Control |
|-------------|--------------|---------|
| Vague explore | Tool spam (reads dominate) | Scoped ask; Ask vs Agent |
| Repo dump | Sticky context | Symbol search first |
| Blind regen | Output 4–6× input weight | Failure-mode before retry |
| Long chat | Rebill history | Compact @~70%; Continuity |
| Model swap | Uncached re-read | Lock model; subagent escalate |
| Retry storm | Tail growth | Max 2 retries + circuit breaker |
| “Continue” | Extra full turns | Explicit next steps |
| Fat always-on rules | Fixed tax | Glob rules + on-demand skills |

### 9.3 Operator ROI ranking (**ESTIMATE**)

1. Scope prompts (file + symptom + done-when) — often **3–5×** less exploration  
2. One task / thread; compact mid-gauge  
3. Lock model for session  
4. Tiny always-on rules  
5. Stop blind regen  
6. Retry budget  

---

## 10. Collaboration, trust, review & undo

### 10.1 Shift of work

Generation → **governance** (authorize, review, undo, absorb blame).

### 10.2 Calibration (**HIGH**)

METR time-horizon: agents strong on short human-equivalent tasks; collapse on multi-hour tasks. Autonomy should track **human-time × success probability × blast radius**, not fluency.

### 10.3 Habituation (**HIGH**)

AIDev longitudinal: approval rates rise while inline comments fall — **reflexive habituation under workload**, not necessarily calibrated trust.

Anthropic: ~**93%** of permission prompts approved — oversight theater risk.

### 10.4 Undo & reject (**HIGH**)

Large shares of agent PR fixes rejected; session studies show majority of visible misalignments need explicit user correction. Checkpoints + run ledgers + Git attribution required.

### 10.5 Parallel conflict (**HIGH**)

CooperBench: Coop ~**30% lower** success vs Solo (relative). SyncMind: human edits mid-run → agent belief ≠ world; collaboration willingness very low.

**Law:** Suggest/Apply + leases + Isolate; never two writers on one checkout.

### 10.6 Blame (**HIGH**)

Humans absorb disproportionate responsibility in hybrid teams (“moral crumple zone”). Machine-readable authorization trails are mandatory.

---

## 11. Clarification gates & “done when” contracts

### 11.1 Timing (**HIGH**)

Ask Early, Ask Late, Ask Right (arXiv:2605.07937):

- **Goal** ambiguity: clarify in first ~**10%** or lose value.  
- **Input**: can explore first (~50% window).  
- Mid/late **constraint** asks can be worse than never asking.  
- Models systematically over-ask or never-ask.

### 11.2 Patterns

Goal-first intake · option chips + Other · assumption ledger · plan-mode for forks · checkable AC · out-of-scope · approval interrupts · revert-to-plan · persist Q&A into plan artifact.

### 11.3 Done-when template

```markdown
## Goal
## Acceptance criteria (checkable)
## Out of scope
## Verification commands
## Approval required before (push/deploy/delete)
## Assumptions (confirm/deny)
```

### 11.4 ADE alignment

Eng-goal object · Suggest clarifies · Apply under leases · Automate + verify · `ade.next-actions` option chips — already DNA-compatible.

---

## 12. ADE intervention map (pragmatic)

### 12.1 Placement law

| Put here when | Mechanism |
|---------------|-----------|
| Must hold even if model ignores prompt | **Harness code** |
| Always-on invariant | **Rule** `.ade/rules` |
| Multi-step, on demand | **Skill** |
| Role × spend × tools | **Profile** |
| Outcome outlives chat | **Eng-goal** |

### 12.2 P0 interventions (ship / dogfood first)

| Habit | Mechanism | Concrete |
|-------|-----------|----------|
| No acceptance criteria | Eng-goal + Suggest | Require done-when before Apply on risky |
| Dual writers | Leases + Isolate | One `leaseAgentId`; worktrees |
| Self-certify done | Verify + Automate gate | `verify-ladder`; no Automate without verify |
| Secrets paste | Rule + Continuity scrub | `secrets-deny` |
| Fake marketing claims | Rule | Cite or `[CLAIM NEEDED]` |
| Fake research citations | Profile `research` + rule | Confidence labels; no invented DOIs |
| Spend runaway | SpendGuard | Reserve/reconcile; visible caps |
| Destructive ops | ToolEffect + skill | Accidental-data-loss skill |
| Vague improve | Rule | Refuse; ask success criteria |
| Learning answer-dump | Skill mode | Socratic / quiz-back profiles |

### 12.3 P1 interventions

Reproduce-first skill · progressive exploration · Continuity thrift · brand/voice pack · evidence-graded-research skill · next-actions chips · change budget (max files/LOC) · epistemic-adversary skill · model lock / subagent escalate · pack-filtered profiles.

### 12.4 Example artifacts to author

**Rule `clarify-before-apply.mdc`**
```markdown
If acceptance criteria are missing for a mutating task, ask ≤3 clarifying
questions in Suggest before any Apply. Do not invent requirements.
```

**Rule `minimal-diff.mdc`**
```markdown
Patch the failing path. No drive-by refactors unless eng-goal lists them.
Prefer minimal diff; rewrite only when the goal says rewrite.
```

**Skill `reproduce-first`**
Stages: capture failure → minimal repro → then patch → verify.

**Skill `epistemic-adversary`**
Triggers on debate/validation: restate → falsify → devil’s advocate → evidence → confidence labels.

**Skill `evidence-graded-research`**
Stages gather→triage→synthesize; VERIFIED/RETRIEVED/INFERRED/UNKNOWN.

**Profiles:** `scout` · `planner` · `coder` · `research` · `marketing-draft` · `thrifty` · `automate-dogfood`.

### 12.5 What not to do

Long “be careful” prompts · mega always-on wiki · verify = same model saying LGTM · peer write agents · hope caching alone · block harness work on Zed wrap (DEC-A-014).

---

## 13. Priority backlog (P0→P3)

| Pri | Work | Outcome |
|-----|------|---------|
| **P0** | Eng-goal gate for risky Apply | Cuts wrong-done thrash |
| **P0** | Clarify-before-Apply rule + chips | Cuts underspec loops |
| **P0** | SpendGuard honesty (H1) | Stops silent $ burn |
| **P0** | Lease UX + conflict messages | Multi-agent robustness |
| **P0** | Secrets + claims + no-weaken-tests rules | Safety / integrity |
| **P1** | `reproduce-first` + `evidence-graded-research` skills | Domain redo cuts |
| **P1** | Continuity thrift defaults | Rediscovery cuts |
| **P1** | Slot Orchestrator (H2) | Planner≠worker product truth |
| **P1** | `epistemic-adversary` skill | Debate integrity |
| **P2** | Model profiles router (H3) | Per-model amplification |
| **P2** | Brand / marketing pack | Workslop reduction |
| **P2** | Learning/Socratic profile | Education modes |
| **P3** | Habituation dashboards | Review quality |
| **Z?** | Zed ACP | Optional after harness dogfood |

---

## 14. Bibliography (selected primary)

**HCI / clarification / trust**  
Amershi CHI’19 · Horvitz CHI’99 · Liao CHI’24 · Ask Early Ask Late arXiv:2605.07937 · METR long tasks arXiv:2503.14499 · Habituation arXiv:2606.22721 · SyncMind arXiv:2502.06994 · CooperBench arXiv:2601.13295 · AIHR blame arXiv:2604.08866  

**Coding agents**  
Ehsani failed PRs arXiv:2601.15195 · AgenticFlict · SWE-bench / Anthropic Sonnet SWE post · Sonar State of Code 2026 · Anthropic context engineering · Chroma Context Rot  

**Business / marketing**  
Springer marketing hallucination review 2026 · J. Business Research Synthetic Optimism 2026 · Google Search AI content / QRG · NIST AI 600-1 · Zapier workslop survey · McKinsey State of AI  

**Research integrity**  
Walters & Wilder 2023 · Topaz Lancet 2026 · Magesh JELS 2025 · arXiv moderation · Springer Nature AI policy · Resnik Accountability in Research  

**Education**  
Bastani PNAS 2025 · Lehmann arXiv:2409.09047 · Strömberg CEPR DP21577 · Arvin sycophancy tutoring · Bo Invisible Saboteurs · Mohamed ACL distortion · Kirkby & Matuschak Memory Machines  

**Sycophancy / persuasion**  
Sharma ICLR 2024 · Cheng Science 2026 · Ibrahim Nature 2026 · Argyle Science 2025 · Salvi NHB 2025 · Hoffman Nature 2025 · OpenAI Model Spec / sycophancy posts · Anthropic Constitution  

**Tokenomics (I/O) & $ metering**  
Anthropic context engineering · Chroma context rot · *ADE Tokenomics* PDF · Anthropic / OpenAI pricing & caching (invoice) · arXiv:2601.06007 Don’t Break the Cache · Augment agent-loop cost · Cursor self-summarization  

**ADE**  
`AGENTS.md` · DEC-A-010/013/014 · Orch G0–G4 · Effort B0–B4 · TOKEN_ECONOMICS_RESEARCH · Guidance Atlas · leases / ToolEffect / verify / Continuity / SpendGuard  

---

## 15. Research provenance

| Track ID | Role |
|----------|------|
| 0c6ad0ce… | Prompting habits |
| 1cfa89c5… | Coding human errors |
| 7b79c052… | Marketing / business / ops / product |
| 306cb668… | Research integrity |
| 587aee98… | Token waste from humans |
| 08833098… | Collaboration / trust / undo |
| 3e67125c… | Education / productivity |
| 9e189643… | Debate / sycophancy |
| 84d04288… | ADE intervention map |
| 5250a561… | Clarification gates |

Parent synthesis + ADE DNA alignment: 2026-07-23.

---

## Closing thesis

The scarce resource is not another model API. It is **human attention under fluent automation**. A VERY good ADE wins by making the right next action cheap and the wrong loops expensive: **contracts before Apply, leases before parallel, sensors before praise, skills on demand, spend caps that tell the truth, and Continuity instead of paste thrash.**

That is the harness answer to the human condition.

*End of study.*
