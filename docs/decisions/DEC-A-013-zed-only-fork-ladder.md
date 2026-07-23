# DEC-A-013 — Zed-only hosts; fork ladder (gated)

- **Status:** Accepted
- **Date:** 2026-07-23
- **Supersedes in part:** DEC-A-010 (VSCodium host), DEC-A-012 (Open VSX companion)
- **Canvas:** ADE-zed-only-fork-research.canvas.tsx

## Context

VSCodium/Open VSX added a second chassis and marketing confusion without owning ADE’s moat. Zed + ACP is enough for a Cursor-shaped job if the harness is excellent. A hard Zed fork is legally possible (GPL) but strategically expensive (rebase, trademarks, focus risk). Community forks (Gram, Zedless) show patch-tracking is hard even without shipping AI product chrome.

## Decision

1. **Hosts:** ADE Desktop (control plane) + **Zed** (primary coding host via ACP). **No VSCodium / Open VSX companion** in the product plan.
2. **Default integration:** Stock Zed + `ade acp` + soft shell (Open in Zed / injected `agent_servers`). **Do not fork on day one.**
3. **Fork ladder (gated):**
   - **L0** Stock Zed + ACP  
   - **L1** Soft shell  
   - **L2** Upstream ACP contributions  
   - **L3** Patch fork (minimal ADE chrome; separate repo; track upstream)  
   - **L4** Hard rebranded editor fork (last resort)
4. **Promote L3/L4 only** after dogfood produces a written list of Agent Panel / chrome gaps that ACP + Desktop cannot close within a defined window.
5. **Never** vendor full Zed into the ADE monorepo as “ADE core.” Editor fork (if any) stays a separate repo under GPL compliance + non-Zed branding.

## Consequences

- Deprecate `hosts/vscodium` and Open-in-VSCodium work.
- Update Ideal / multi-host docs to Zed-only eyes.
- Near-term excellence = ACP fidelity, guidance parity, Orchestrator — not IDE maintenance.
- Legal: any distributed modified Zed build requires GPL source offer and trademark-safe naming (not “Zed”).
