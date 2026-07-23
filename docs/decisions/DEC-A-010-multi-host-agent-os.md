# DEC-A-010 — Multi-host Agent OS

- **Status:** Accepted (amended 2026-07-23 by DEC-A-013)
- **Date:** 2026-07-22
- **Canvas:** ADE-multihost-gameplan.canvas.tsx · ADE-zed-only-fork-research.canvas.tsx

## Context

ADE’s end-goal is Cursor-shaped (AI-native build environment) without a Microsoft/Electron chassis. Zed provides a native Rust editor + [ACP](https://zed.dev/acp). ADE already owns harness truth (leases, Suggest/Apply, verify, Continuity, spend).

## Decision

ADE is a **multi-host agent OS** with a **narrow host set**:

| Host | Role |
|------|------|
| ADE Desktop (Tauri) | Harness control plane |
| Zed | Primary coding host via ACP |
| Other ACP clients (optional later) | Same `ade acp` binary |

One shared workspace/session policy across hosts. **Do not** treat Open VSX / VSCodium as a product host (see DEC-A-013). **Do not** fork Zed into this monorepo on day one; fork only via the gated ladder in DEC-A-013.

## Consequences

- Product identity = harness, not editor chrome.
- Near-term work: `crates/acp` + `ade acp`, Open-in-Zed, Orchestrator/SpendGuard.
- Cohesion = shared `.ade/` identity + Continuity, not a single fused binary (v1).
