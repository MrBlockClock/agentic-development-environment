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

## Canonical docs

- [REPO_LAYOUT.md](https://github.com/MrBlockClock/agentic-development-environment/blob/main/docs/architecture/REPO_LAYOUT.md)
- [ARCHITECTURE_SYNTHESIS.md](https://github.com/MrBlockClock/agentic-development-environment/blob/main/docs/platform/ARCHITECTURE_SYNTHESIS.md)
- ADRs: [`docs/decisions/`](https://github.com/MrBlockClock/agentic-development-environment/tree/main/docs/decisions)

## Non-goals

- Electron IDE forks  
- Vendoring Zed / VS Code source  
- Replacing MCP with a proprietary marketplace  

See DEC-A-010 / DEC-A-014 in the decisions tree.
