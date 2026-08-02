---
layout: default
title: DEC-A-016-security-baseline
---

# DEC-A-016 — Desktop/CLI security baseline (spend, MCP, path, CSP)

- **Status:** Accepted
- **Date:** 2026-08-02
- **Depends on:** DEC-A-014 (harness-first)
- **Canvas:** ADE-scrutiny-improvement-gameplan.canvas.tsx (W0–W1)

## Context

Scrutiny (2026-08-02) found spend bypasses, client-trusted MCP `approved`, extract path escape, ledger TOCTOU, and `csp: null` as the highest harness risks. Ideal gold and Desktop IA were already strong; security honesty needed a single baseline ADR.

## Decision

1. **Spend honesty:** Caps require priced $/MTok (or explicit `--allow-unpriced` / Confirm unmetered). Reserve is transactional (`BEGIN IMMEDIATE`); commit re-checks hard cap.
2. **MCP spawn:** Catalog `recipe_id` allowlist is the trust root; bare client `approved` alone cannot spawn arbitrary commands. Custom MCP needs `ADE_ALLOW_CUSTOM_MCP` in release.
3. **Path containment:** Extract/transcribe resolve under workspace (or staged inbox); PDF/Office byte caps apply.
4. **CSP:** Production Tauri CSP is enabled; `devCsp` stays relaxed for Vite HMR.
5. **Leases:** Lease registry uses cross-process `fs2` locking (same pattern as task registry).
6. **Agent web_fetch:** Loopback/private/link-local/metadata hosts are denied (in-app Browser may still allow localhost by design).

## Consequences

- CLI/Desktop/worker share `require_priced_for_caps` via turn prepare.
- Dogfood scripts that set `ADE_ALLOW_UNPRICED=1` are explicit unmetered paths, not silent bypasses.
- Phantom ADR ids in older synthesis docs remain **non-binding** until filed; this baseline covers the W0–W1 security slice.

## References

- `crates/agents/src/{spend,mcp,web,turn}.rs`
- `crates/db/src/usage_ledger.rs`
- `apps/desktop/src-tauri/tauri.conf.json`
- `crates/workflow/src/parallel.rs`
