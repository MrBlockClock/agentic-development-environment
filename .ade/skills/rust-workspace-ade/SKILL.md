---
name: rust-workspace-ade
description: >-
  Build and change the ADE Rust+Tauri monorepo safely (Money, locks, clippy,
  desktop exclude). Use when editing crates/, apps/, or fixing compile/test failures.
---
# Rust Workspace ADE

## Layout

- Crates: `crates/{core,db,workflow,agents,api,desktop,service,...}`
- Apps: `apps/cli`, `apps/desktop` (Tauri)
- Target dir often `ade-target/` (see `.cargo/config.toml`)

## Safe build habits

```powershell
Get-Process ade,ade-desktop-app -ErrorAction SilentlyContinue | Stop-Process -Force
cargo fmt
cargo clippy --workspace --exclude ade-desktop-app -- -D warnings
cargo test --workspace --exclude ade-desktop-app
```

## Invariants

- Money = `ade_core::Money` micros; no `f64` finance math
- Authority loads `AGENTS.md` + `.ade/rules/*.mdc`
- Skills load from `.ade/skills/*/SKILL.md`
- Prefer owned-String / clear lifetimes over clever borrows when rust-analyzer noise appears

## Desktop vs API

Browser preview: dashboard/recipes/verify via HTTP. MCP connect + agent turns need Tauri desktop.
