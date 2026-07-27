# Contributing to ADE

Thanks for interest in ADE. This repo is a **local agent harness** (Rust + Tauri Desktop + CLI). Keep PRs small, honest, and secret-free.

## Prerequisites

- Rust **stable**
- Node **22+** (Desktop / e2e)
- Windows: WebView2 for Tauri Desktop

## Golden path

From the repo root:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build
```

Desktop:

```bash
cd apps/desktop
npm install
npm run tauri dev
```

## Branch & commits

- Branch from `main` (or the active feature branch if coordinating)
- Prefer conventional subjects: `feat(desktop): …`, `fix(db): …`, `docs: …`
- **Do not** commit `.env`, keys, PEMs, or credential JSON
- Prefer **your** git author for merges — squash when possible

## Docs

- User-facing entry: `README.md` + `docs/guides/`
- Product DNA: `AGENTS.md` (do not blur Ideal vs aspirational vision)
- Architecture: `docs/architecture/`, ADRs in `docs/decisions/`
- Wiki source mirror: `docs/wiki/` (push to GitHub Wiki when publishing)

## Pull requests

Include:

1. **Summary** — why this change exists  
2. **Test plan** — commands you ran / UI paths checked  

CI must be green (`cargo` clippy/test + Desktop checks as configured).

## Code of collaboration

- Suggest = inspect; Apply = leased writes; Automate = Apply + verify  
- Spend caps require honest $/MTok (or explicit unmetered)  
- Risk categories (secrets / infra / migrate / publish) stay human-gated  

Questions? Open an issue or discuss in the PR.
