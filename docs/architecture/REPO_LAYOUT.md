---
layout: default
title: Repository Layout
---

# ADE repository layout

**Schema:** `ade.repo-layout/v1`  
**Status:** Active · evolutionary (no big-bang crate move)  
**ADR:** [DEC-A-011](../decisions/DEC-A-011-repo-layout.md) · host policy [DEC-A-017](../decisions/DEC-A-017-retire-zed-host.md)  
**Canvas:** `ADE-master-gameplan.canvas.tsx`

## Principle

ADE is one **Rust agent harness**. Product surface is **Desktop + CLI** only. External editor hosts (Zed, VSCodium, forks) are **non-goals**.

```
                    ┌─────────────────────────┐
                    │   ADE harness (crates)  │
                    │  agents · workflow · $  │
                    └───────────┬─────────────┘
                 ┌──────────────┴──────────────┐
                 ▼                             ▼
           apps/desktop                   apps/cli
           Tauri control plane            `ade` CLI
```

## Target tree

```
ade/
├── crates/
│   ├── core/           # Money, audit, recipe, handoff types, errors
│   ├── agents/         # Turn loop, tools, spend, chat, MCP client
│   ├── workflow/       # Leases, tasks, verify, worktrees
│   ├── db/             # Ledger, vault adapters
│   ├── api/            # HTTP API (thin)
│   ├── service/        # Background workers
│   ├── plugins/        # WASM plugin host
│   └── desktop/        # Tauri command crate (Desktop backend)
├── apps/
│   ├── cli/            # `ade` binary
│   └── desktop/        # Tauri + React harness UI
├── hosts/              # Retired editor-host tombstones only (DEC-A-017)
├── docs/
│   ├── architecture/   # REPO_LAYOUT.md (this file)
│   ├── decisions/      # ADRs (DEC-A-*)
│   ├── platform/       # Ideal, Orch, Effort, vision
│   └── guides/
├── evals/ · scripts/ · tests/
├── docker/             # optional CLI-only Dockerfile (not full Desktop runtime)
├── nix/                # planned / empty scaffold (no flake yet)
├── AGENTS.md
└── Cargo.toml          # workspace members
```

## What does **not** live here

| Out of repo / non-goal | Why |
|------------------------|-----|
| Zed / VSCodium / Code-OSS source or soft shells | Product non-goal (DEC-A-017) |
| Electron IDE fork | Explicit non-goal |
| `crates/acp` / `ade acp` | Removed with DEC-A-017 |

## Migration rule

1. **Do not** rename/move existing crates in one PR.  
2. Keep `crates/desktop` as the Desktop host backend.  
3. Do not re-add editor host packs without a superseding ADR.
