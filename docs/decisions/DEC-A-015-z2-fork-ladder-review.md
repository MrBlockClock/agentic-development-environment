# DEC-A-015 — Z2 fork-ladder review (post soft shell)

- **Status:** Accepted  
- **Date:** 2026-07-23  
- **Depends on:** DEC-A-013, DEC-A-014, Z1 (`ade acp` soft shell)  
- **Canvas:** ADE-master-gameplan.canvas.tsx  

## Context

Z1 shipped: `ade acp` speaks ACP JSON-RPC (`initialize` · `session/new` · `session/set_mode` · `session/prompt` · `session/cancel`) with Suggest / Apply / Automate modes. Desktop remains the live-turn control plane. DEC-A-013 forbids promoting a Zed fork without a written chrome-gap list.

## Review verdict (2026-07-23)

**Stay at L1 (soft shell).** Do **not** promote L3/L4.

| Ladder | Status |
|--------|--------|
| L0 Stock Zed + ACP | Available |
| **L1 Soft shell** | **Current — `ade acp`** |
| L2 Upstream ACP contributions | Open when gaps are protocol bugs, not product chrome |
| L3 Patch fork | **Blocked** — no written gaps ACP+Desktop cannot close |
| L4 Hard rebranded fork | **Blocked** — last resort only |

## Promotion gate (must all be true)

1. Dogfood log (≥2 weeks) listing Agent Panel / chrome gaps with repro steps.  
2. Each gap tried via ACP soft shell **and** ADE Desktop without adequate close.  
3. Written estimate: patch-fork maintenance cost vs benefit.  
4. Explicit human decision recorded in a follow-up ADR (not chat).  
5. Separate repo + GPL compliance + non-Zed branding (DEC-A-013 §5).

## Non-gaps (do not justify fork)

- Wanting live LLM turns inside Zed → use Desktop / wire provider later in ACP; not a fork.  
- Lease / contract / spend UX → harness + Desktop (already shipped).  
- Mission Control chrome → deferred until slot dogfood (DEC-A-014).

## Consequences

- Eng focus stays harness + gold (C5+) and ACP fidelity at L1.  
- `hosts/zed/README.md` documents L1 only.  
- Revisit this ADR only when the promotion gate checklist is filled.
