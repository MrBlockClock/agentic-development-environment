---
layout: default
title: Architecture
---

# Architecture

ADE is **one Rust agent OS** (harness). Editors and UIs are **hosts**, not forks inside this repo.

```
                 ┌─────────────────────────┐
                 │   ADE harness (crates)  │
                 │  agents · workflow · $  │
                 └───────────┬─────────────┘
        ┌────────────────────┼────────────────────┐
        ▼                    ▼                    ▼
 apps/cli + acp        apps/desktop         hosts/* (docs)
 `ade` / `ade acp`     Tauri control plane  Zed · …
```

## Canonical docs

| Doc | Role |
|-----|------|
| [REPO_LAYOUT](../architecture/REPO_LAYOUT.html) | Multi-host tree & non-goals |
| [ARCHITECTURE_SYNTHESIS](../platform/ARCHITECTURE_SYNTHESIS.html) | Full system design |
| [ADRs](../decisions/) | DEC-A-010 … 016 |
| [Master Gameplan](../research/ADE-Master-Gameplan.html) | Research → build waves |

## Crates (brain)

| Crate | Role |
|-------|------|
| `core` | Money, audit, recipe, handoff types |
| `agents` | Turn loop, tools, spend, chat, MCP |
| `workflow` | Leases, tasks, verify, worktrees |
| `db` | Ledger + secrets vault adapters |
| `api` | Thin HTTP API |
| `desktop` | Tauri command backend |
| `acp` | Agent Client Protocol adapter |

## Apps (hosts)

| App | Role |
|-----|------|
| `apps/desktop` | Control plane — composer, Trust, Analytics, Integrations |
| `apps/cli` | `ade` CLI including `ade acp` for Zed |

## Non-goals

- Electron IDE forks  
- Vendoring Zed / VS Code source  
- Replacing MCP with a proprietary marketplace  

See DEC-A-010 / DEC-A-014.
