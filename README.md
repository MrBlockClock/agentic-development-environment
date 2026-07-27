# ADE — Agent Development Environment

**A local harness OS for agentic software engineering** — not another IDE fork.

ADE runs the agent loop with **honest spend**, **verify-as-truth**, **leases / Isolate worktrees**, and **Continuity** across turns. **Desktop** (Tauri) is the control plane; **CLI** and optional **Zed via ACP** are hosts on the same Rust brain.

[![CI](https://github.com/MrBlockClock/agentic-development-environment/actions/workflows/ci.yml/badge.svg)](https://github.com/MrBlockClock/agentic-development-environment/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

| | |
|--|--|
| **Docs site** | [mrblockclock.github.io/agentic-development-environment](https://mrblockclock.github.io/agentic-development-environment/) |
| **Stack** | Rust workspace · Tauri Desktop · React · MCP · OS key vault |
| **Autonomy** | Suggest → Apply → Automate (with verify gates) |

---

## Why ADE

Most “agent IDEs” optimize chat chrome. ADE optimizes the **harness**:

- **Spend honesty** — reserve → reconcile $; Trust shows used / reserved / remaining; never silent $0 on priced turns  
- **Verify-as-truth** — gates (G0–G5), not model self-certify  
- **Safe Apply** — eng-goal contracts, path leases, Isolate worktrees, risk HITL  
- **Continuity** — handoff capsules + thrift resume (no paste theater)  
- **Multi-host** — one brain, many eyes (Desktop control plane; Zed optional)

Deep dive: [docs/research/ADE-Master-Gameplan.md](docs/research/ADE-Master-Gameplan.md) · [Docs site](https://mrblockclock.github.io/agentic-development-environment/)

---

## Quick start

**Prereqs:** Rust stable · Node 22+ · (Windows) WebView2 for Desktop

```bash
git clone https://github.com/MrBlockClock/agentic-development-environment.git
cd agentic-development-environment
cp .env.example .env          # fill only what you need; never commit secrets
cargo build
cargo clippy -- -D warnings
cargo test
```

**Desktop (dev):**

```bash
cd apps/desktop
npm install
npm run tauri dev             # Vite on http://127.0.0.1:1420 + ADE window
```

**CLI:**

```bash
cargo run -p ade-cli -- --help
```

Full guide: [docs/guides/getting-started.md](docs/guides/getting-started.md) · [Docs · Getting Started](https://mrblockclock.github.io/agentic-development-environment/guides/getting-started.html)

---

## Repository layout

```
ade/
├── crates/          # Harness OS — agents, workflow, spend, MCP, db, api, acp
├── apps/
│   ├── cli/         # `ade` CLI (+ `ade acp` for Zed)
│   └── desktop/     # Tauri + React control plane
├── hosts/           # Integration packs (Zed, …) — no vendored IDE forks
├── docs/            # Architecture, platform plans, guides, ADRs
├── evals/           # Gold-set races (spend, slots, continuity, …)
└── AGENTS.md        # Product DNA + agent contract
```

See [docs/architecture/REPO_LAYOUT.md](docs/architecture/REPO_LAYOUT.md).

---

## Product surface (Desktop)

| Area | What you get |
|------|----------------|
| **Home** | Composer, Suggest / Apply / Automate, Continuity, feed |
| **Setup** | Environment · Keys (BYOK vault) · Integrations · Stack · Test |
| **Insight** | Trust · Analytics · Plan Map / Atlas (Debug) |
| **Context** | Workspaces · Terminal · Browser |
| **Integrations** | GitHub, GitLab, Stripe, Azure, MCP recipes + host tools |

Surface plan: [docs/platform/ADE_SURFACE_GAMEPLAN.md](docs/platform/ADE_SURFACE_GAMEPLAN.md)

---

## Safety & spend (non-negotiable)

| Rule | Meaning |
|------|---------|
| BYOK | Keys in OS vault via Desktop → Keys — never commit secrets |
| Caps need rates | Session/daily caps require honest $/MTok (or explicit unmetered) |
| Risk HITL | Secrets / infra / migrate / publish need confirm even under Automate |
| Verify | Automate expects verify-on-complete; Trust owns the audit trail |

Details: [Docs · Safety and Spend](https://mrblockclock.github.io/agentic-development-environment/wiki/Safety-and-Spend.html) · `AGENTS.md`

---

## Documentation

| Doc | Purpose |
|-----|---------|
| [Docs site](https://mrblockclock.github.io/agentic-development-environment/) | Polished entry + guides |
| [Getting started](docs/guides/getting-started.md) | Build, Desktop, CLI |
| [Architecture](docs/architecture/REPO_LAYOUT.md) | Multi-host layout |
| [Master gameplan](docs/research/ADE-Master-Gameplan.md) | What shipped / why |
| [Ideal masterplan](docs/platform/IDEAL_ADE_MASTERPLAN.md) | Product spine |
| [ADRs](docs/decisions/) | Decisions (DEC-A-*) |
| [AGENTS.md](AGENTS.md) | Contract for agents & humans |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Short version: `cargo fmt` · `cargo clippy -- -D warnings` · `cargo test` · keep secrets out of git · prefer small PRs with a Test plan.

---

## License

[MIT](LICENSE) © Caleb Enloe / MrBlockClock

---

## Status

Active personal / research harness. Desktop + CLI are dogfooded daily. Mission Control and a full Zed fork remain **deferred** — see gameplans for the honest roadmap.
