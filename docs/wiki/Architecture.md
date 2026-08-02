---
layout: default
title: Architecture
---

# Architecture

ADE is a **local harness**: one brain in Rust crates, **Desktop + CLI** as the product surface. External editor hosts are non-goals ([DEC-A-017](../decisions/DEC-A-017-retire-zed-host.html)).

```
 crates/*              apps/desktop         apps/cli
 harness brain         Tauri control plane  `ade` CLI
```

## Canonical docs

| Doc | Role |
|-----|------|
| [REPO_LAYOUT](../architecture/REPO_LAYOUT.html) | Tree & non-goals |
| [ARCHITECTURE_SYNTHESIS](../platform/ARCHITECTURE_SYNTHESIS.html) | Full system design |
| [ADRs](../decisions/) | DEC-A-010 … 017 |
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

## Apps

| App | Role |
|-----|------|
| `apps/desktop` | Control plane — composer, Trust, Analytics, Integrations |
| `apps/cli` | `ade` CLI |

## Non-goals

- Electron / Zed / VS Code editor forks or soft shells  
- Vendoring IDE source  
- Replacing MCP with a proprietary marketplace  

See DEC-A-014 (harness-first) · DEC-A-017 (retire Zed/ACP hosts).
