---
layout: default
title: Contributing
---

# Contributing

## Quality bar (mirrors CI)

```bash
pwsh -File ./scripts/sync-cursor-guidance.ps1 -Check
cargo fmt --check
cargo clippy --workspace --exclude ade-desktop-app --all-targets -- -D warnings
cargo test --workspace --exclude ade-desktop-app
cargo run -p ade-cli --quiet -- eval --gold
```

Desktop (`apps/desktop`): `npm ci` · `npm run build` · `npm run test:unit` · Playwright IA/Analytics subset.  
G4: `pwsh -File ./scripts/verify-g4.ps1`.

Desktop changes: also exercise `npm run tauri dev` and note a short Test plan in the PR.

## Secrets

Never commit keys, `.env` (non-example), PEMs, or credential files.

## Docs

- Entry: `README.md`, `docs/guides/`  
- DNA: `AGENTS.md`  
- Wiki mirror: `docs/wiki/` (keep in sync when you change public docs)

## Full guide

[CONTRIBUTING.md](https://github.com/MrBlockClock/agentic-development-environment/blob/main/CONTRIBUTING.md)
