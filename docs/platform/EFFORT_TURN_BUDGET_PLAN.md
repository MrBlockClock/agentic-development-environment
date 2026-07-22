# Effort / Turn Budget Honesty Plan

**Schema:** `ade.effort-budget/v1`  
**Status:** **B0–B4 shipped** · Next track: SpendGuard /$ (held)  
**Canvas:** `ADE-effort-budget-gameplan.canvas.tsx`  
**Depends on:** N4 Continuity · turnFailure Fix&retry · Product DNA (harness owns budgets)

## Verdict

Effort is a **per-turn gas tank** (tool rounds + tokens), not “how smart the
model is.” Hard caps stay — they prevent runaway loops and spend. When the tank
hits empty, ADE must **tell the truth**, **persist a handoff**, and **offer
Continue / raise Effort** — never strand the user behind `Provider error`.

## Shipped

- Effort tiers Low/Med/High map to max tool rounds (16 / 24 / 32).
- `turnFailure` classifies round-limit + auto-raises Effort once; offers
  **Continue handoff**.
- `scripts/dogfood-continuity.ps1` host-runs `next_safe_command` then resumes
  with raised `max_steps`.
- **B0:** `AdeError::Budget` + `AgentEvent::BudgetExhausted`; Failed/CLI/Desktop
  say `Budget exhausted:…` (not Provider); handoff `turn_status=budget_exhausted`.
- **B1:** Effort copy = turn gas tank; Live activity shows `used/max` rounds.
- **B2:** Budget banner + Continue handoff CTA (raise Effort + `handoff_resume`).
- **B3:** Desktop `handoff_resume` host-runs `ade …` next_safe; thrift resume
  prompt forbids discovery loops.
- **B4:** Apply / Automate / Continuity floor Effort at Med+; Low for Suggest.

## Phases

| Phase | Name | Ships |
|-------|------|-------|
| **B0** | Honesty | Round/token budget stop ≠ Provider error — **done** |
| **B1** | UX dial | Effort copy + rounds used/remaining — **done** |
| **B2** | Continue | Continue CTA on budget stop — **done** |
| **B3** | Continuity thrift | Host runs next_safe; thrifty resume — **done** |
| **B4** | Defaults | Continuity/Automate Med+; Low for Suggest — **done** |

## Non-goals / held

- Unlimited tool loops
- Mission Control budget dashboards
- Silent model swap when the real issue is budget
- **SpendGuard** reserve policy / $0 rates / cache ledger (next track)

## Alignment

| Contract | Location |
|----------|----------|
| max_tool_rounds | `crates/agents/src/session.rs` |
| Effort UI | Desktop Agent / Settings |
| Fix&retry / Continue | `apps/desktop/src/components/turnFailure.ts` |
| Continuity host-run | `crates/desktop/src/commands.rs` `handoff_resume` |
| Continuity dogfood | `scripts/dogfood-continuity.ps1` |
