---
layout: default
title: Getting Started
---

# Getting Started

## Prerequisites

- Rust **stable**
- Node.js **22+**
- Windows: **WebView2** for Desktop

## Clone

```bash
git clone https://github.com/MrBlockClock/agentic-development-environment.git
cd agentic-development-environment
cp .env.example .env
```

Do **not** commit secrets. Prefer Desktop → **Keys** (OS vault) for provider credentials. Optional vault import: `ADE_IMPORT_ENV_KEYS=1` (see `.env.example`).

## Rust golden path (CI mirror)

```bash
cargo fmt --check
cargo clippy --workspace --exclude ade-desktop-app --all-targets -- -D warnings
cargo test --workspace --exclude ade-desktop-app
cargo run -p ade-cli --quiet -- eval --gold
```

```bash
cargo run -p ade-cli -- --help
```

Full local checklist: [CONTRIBUTING](Contributing).

## Desktop

```bash
cd apps/desktop
npm install
npm run tauri dev
```

Vite serves **`http://127.0.0.1:1420`**. Keep the terminal open.

1. Attach a workspace folder  
2. **Setup → Keys** — add a provider key  
3. **Home** — Suggest first; Apply when you have a contract  
4. **Setup → Integrations** — MCP recipes (GitHub / Linear, …)  
5. Debug on → audio **Transcribe** → `.ade/inbox/*.transcript.md` when needed  

## Dogfood (optional)

- Continuity thrift: `scripts/dogfood-continuity.ps1`  
- Continuity + PDF extract: `scripts/dogfood-continuity-pdf-mcp.ps1` (CLI may `mcp=skipped` — Continuity+extract only)  

## In-repo guide

Full detail: [`docs/guides/getting-started.md`](https://github.com/MrBlockClock/agentic-development-environment/blob/main/docs/guides/getting-started.md)

## Next

- [[Architecture]]
- [[Desktop]]
- [[Safety-and-Spend]]
