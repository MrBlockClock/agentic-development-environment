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

Do **not** commit secrets. Prefer Desktop â†’ **Keys** (OS vault) for provider credentials.

## Rust golden path

```bash
cargo build
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

```bash
cargo run -p ade-cli -- --help
```

## Desktop

```bash
cd apps/desktop
npm install
npm run tauri dev
```

Vite serves **`http://127.0.0.1:1420`**. Keep the terminal open.

1. Attach a workspace folder  
2. **Setup â†’ Keys** â€” add a provider key  
3. **Home** â€” Suggest first; Apply when you have a contract  

## In-repo guide

Full detail: [`docs/guides/getting-started.md`](https://github.com/MrBlockClock/agentic-development-environment/blob/main/docs/guides/getting-started.md)

## Next

- [[Architecture]]
- [[Desktop]]
- [[Safety-and-Spend]]
