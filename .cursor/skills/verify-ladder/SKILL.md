---
name: verify-ladder
description: >-
  Run and interpret ADE verify gates G0–G5 (fmt, clippy, tests, integration,
  G5 evidence). Use when verifying work, debugging CI, or before claiming done.
---
# Verify Ladder

| Gate | Meaning | Typical command |
|------|---------|-----------------|
| G0 | Manifest | `cargo locate-project` |
| G1 | Contract | `AGENTS.md` present |
| G2 | Lint | `cargo fmt --check` + `cargo clippy -- -D warnings` |
| G3 | Unit/integration | `cargo test --workspace` |
| G4 | Full script | `scripts/verify-full.ps1` |
| G5 | Evidence | recipe profile / `scripts/g5-evidence.*` / cargo test |

**UI layout smoke (optional, not a gate):** `pwsh -File scripts/ui-smoke.ps1` — Playwright against Vite preview (`apps/desktop` e2e). Catches sidebar IA / type-size regressions. Does **not** replace G0–G5 and does not drive Tauri IPC.

## How to run

- CLI: `ade verify --gate G5 --through` (or desktop/browser Verify → posts `/api/verify`)
- API must be up for browser: `ade serve --bind 127.0.0.1:3210`
- On Windows: stop locking processes (`ade.exe`, `ade-desktop-app.exe`) before rebuilds

## Reporting

For each gate report: status (`pass`/`fail`/`unavailable`), command, and on fail the first actionable stderr/stdout lines. Unavailable is not a hard stop when running through.
