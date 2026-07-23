# ADE repository layout (multi-host)

**Schema:** `ade.repo-layout/v1`  
**Status:** Active · evolutionary (no big-bang crate move)  
**ADR:** [DEC-A-010](../decisions/DEC-A-010-multi-host-agent-os.md) · [DEC-A-011](../decisions/DEC-A-011-repo-layout.md)  
**Canvas:** `ADE-multihost-gameplan.canvas.tsx`

## Principle

ADE is one **Rust agent OS** (harness). Editors are **hosts**, not forks inside this repo.

```
                    ┌─────────────────────────┐
                    │   ADE harness (crates)  │
                    │  agents · workflow · $  │
                    └───────────┬─────────────┘
           ┌────────────────────┼────────────────────┐
           ▼                    ▼                    ▼
    apps/cli + acp        apps/desktop         hosts/* (docs)
    `ade` / `ade acp`     Tauri control plane  Zed · VSCodium
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
│   ├── plugins/        # WASM plugin host (ADE plugins — not Zed/VS Code)
│   ├── desktop/        # Tauri command crate (Desktop host backend)
│   └── acp/            # NEW — Agent Client Protocol adapter (ADE as ACP agent)
├── apps/
│   ├── cli/            # `ade` binary (includes `ade acp` entry)
│   └── desktop/        # Tauri + React harness UI
├── hosts/              # NEW — host integration packs (no vendored IDEs)
│   ├── zed/            # ACP settings examples, dogfood notes
│   ├── vscodium/       # Open-in + Open VSX companion notes
│   └── README.md
├── docs/
│   ├── architecture/   # REPO_LAYOUT.md (this file)
│   ├── decisions/      # ADRs (DEC-A-*)
│   ├── platform/       # Ideal, Orch, Effort, vision
│   └── guides/
├── evals/ · scripts/ · tests/ · docker/ · nix/
├── AGENTS.md
└── Cargo.toml          # workspace members
```

## What does **not** live here

| Out of repo | Why |
|-------------|-----|
| Zed / VSCodium / Code-OSS source | External hosts; integrate via ACP / Open in… |
| Open VSX registry | External catalog for VSCodium only |
| Electron IDE fork | Explicit non-goal (DEC-A-010) |

## Migration rule

1. **Add** new crates/dirs (`crates/acp`, `hosts/`) immediately.  
2. **Do not** rename/move existing crates in one PR.  
3. Optional later: `crates/desktop` → keep name; document as “Desktop host backend.”

## Workspace members (Cargo)

Current + `crates/acp`. CLI gains `ade acp` subcommand that runs the ACP stdio agent.
