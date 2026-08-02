---
name: scrutiny-council
description: >-
  Run ADE field scrutiny bots mapped to stack layers and JD alignment.
  Use before claiming done, after multi-layer changes, or when reviewing
  tech-stack / career fit. See docs/platform/SCRUTINY_AGENTS.md.
---

# Scrutiny council

Catalog: `docs/platform/SCRUTINY_AGENTS.md`  
Agents: `.cursor/agents/*-scrutiny.md` + `scrutiny-council.md`

## When

- Multi-crate or Rust+UI+API diffs
- Integrations / secrets / spend / verify changes
- Before “done” / PR / interview-facing demos
- Any request to align ADE with a JD or Enspire-shaped stack

## Layer → agent

| Touch | Agent |
|-------|--------|
| `crates/*` harness | `rust-harness-scrutiny` |
| Tauri / IPC | `tauri-desktop-scrutiny` |
| `apps/desktop` UI | `react-desktop-ui-scrutiny` + `progressive-ui-scrutiny` |
| `crates/api` | `axum-api-scrutiny` |
| DB / ledger | `sqlite-ledger-scrutiny` |
| Vault / MCP / Keys | `mcp-secrets-scrutiny` |
| Verify / gold | `verify-gold-scrutiny` |
| Pricing / Trust spend | `spend-honesty-scrutiny` |
| Leases / slots / Apply | `leases-slots-scrutiny` |
| Continuity / compact | `continuity-channel-scrutiny` |
| Wasm plugins | `wasm-plugins-scrutiny` |
| Playwright | `playwright-e2e-scrutiny` |
| Integrations catalog | `integrations-connectors-scrutiny` |
| Positioning / IDE-like | `dna-anti-ide-scrutiny` |
| Cloud/auth/pay/JD | `jd-platform-ops-scrutiny` |
| Recipes / Fit | `recipe-stack-fit-scrutiny` |
| ADR / layout | `architecture-adr-scrutiny` |
| Near finish | `problems-diagnostics-scrutiny` |

Always finish with **council verdict**: `ship` | `fix-first` | `park` (JD-bleed).

## End-goal reminder

DNA (`AGENTS.md`) wins. JD skills show up as connectors, verify rigor, React+TS quality, and ledger honesty — not as portal/Auth0/Stripe-product bleed.
