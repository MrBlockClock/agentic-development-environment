---
layout: default
title: Architecture
---

# Architecture

ADE is **one Rust agent OS** (harness). Editors and UIs are **hosts**, not forks inside this repo.

```
                 â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
                 â”‚   ADE harness (crates)  â”‚
                 â”‚  agents Â· workflow Â· $  â”‚
                 â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¬â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
        â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”¼â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
        â–¼                    â–¼                    â–¼
 apps/cli + acp        apps/desktop         hosts/* (docs)
 `ade` / `ade acp`     Tauri control plane  Zed Â· â€¦
```

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
| `apps/desktop` | Control plane â€” composer, Trust, Analytics, Integrations |
| `apps/cli` | `ade` CLI including `ade acp` for Zed |

## Canonical docs

- [REPO_LAYOUT.md](https://github.com/MrBlockClock/agentic-development-environment/blob/main/docs/architecture/REPO_LAYOUT.md)
- [ARCHITECTURE_SYNTHESIS.md](https://github.com/MrBlockClock/agentic-development-environment/blob/main/docs/platform/ARCHITECTURE_SYNTHESIS.md)
- ADRs: [`docs/decisions/`](https://github.com/MrBlockClock/agentic-development-environment/tree/main/docs/decisions)

## Non-goals

- Electron IDE forks  
- Vendoring Zed / VS Code source  
- Replacing MCP with a proprietary marketplace  

See DEC-A-010 / DEC-A-014 in the decisions tree.
