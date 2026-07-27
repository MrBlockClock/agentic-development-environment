---
layout: default
title: Contributing
---

# Contributing

## Quality bar

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Desktop changes: exercise `npm run tauri dev` and note a short Test plan in the PR.

## Secrets

Never commit keys, `.env` (non-example), PEMs, or credential files.

## Docs

- Entry: `README.md`, `docs/guides/`  
- DNA: `AGENTS.md`  
- Wiki mirror: `docs/wiki/` (keep in sync when you change public docs)

## Full guide

[CONTRIBUTING.md](https://github.com/MrBlockClock/agentic-development-environment/blob/main/CONTRIBUTING.md)
