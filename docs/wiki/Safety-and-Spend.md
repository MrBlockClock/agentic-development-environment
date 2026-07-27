---
layout: default
title: Safety and Spend
---

# Safety and Spend

These rules are product DNA â€” not optional polish.

## BYOK

Provider credentials live in the **OS vault** (Desktop â†’ **Keys**). Never commit `.env` secrets, PEMs, or credential JSON.

## Spend honesty (H1)

- Session / daily caps require honest **$/MTok** (or explicit unmetered / `ADE_ALLOW_UNPRICED`)  
- Reserves use estimated message size â€” not the full context window billed as if filled  
- Trust shows **used / reserved / remaining**; ledger shows reserved âˆ’ actual (Î”)  
- Missing provider usage on priced turns falls back to the reserve (never silent $0)

## Autonomy

| Mode | Meaning |
|------|---------|
| **Suggest** | Planner / inspect â€” no write leases |
| **Apply** | Worker under leases + eng-goal contract |
| **Automate** | Apply + verify-on-complete |

## Risk HITL (G2)

Secrets / infra / migrate / publish require explicit confirm even under Automate. Waives are logged.

## Verify-as-truth

Gates beat model self-certify. Trust owns the audit trail; Analytics owns cost trend & attribution.

## Continuity

Handoff capsules + thrift resume â€” Desktop **Continue** or `ade handoff resume`. Prefer host `next_safe` over paste theater.

## Contract

Full text: [`AGENTS.md`](https://github.com/MrBlockClock/agentic-development-environment/blob/main/AGENTS.md)
