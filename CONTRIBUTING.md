# Contributing to ADE

Thanks for interest in ADE. This repo is a **local agent harness** (Rust + Tauri Desktop + CLI). Keep PRs small, honest, and secret-free.

## Prerequisites

- Rust **stable**
- Node **22+** (Desktop / e2e)
- Windows: WebView2 for Tauri Desktop

## Golden path (mirrors `.github/workflows/ci.yml`)

From the repo root. Prefer the same flags CI uses so local green ≈ merge green:

```bash
# Guidance mirrors (.ade ↔ .cursor)
pwsh -File ./scripts/sync-cursor-guidance.ps1 -Check

# Rust job
cargo fmt --check
cargo clippy --workspace --exclude ade-desktop-app --all-targets -- -D warnings
cargo test --workspace --exclude ade-desktop-app
cargo run -p ade-cli --quiet -- eval --gold
```

Desktop job (`apps/desktop`):

```bash
cd apps/desktop
npm ci
npm run build
npm run test:unit
# CI also runs a Playwright subset (after npm run test:e2e:install):
#   npx playwright test e2e/sidebar-ia.spec.ts e2e/insight-analytics.spec.ts
```

G4 (after rust + desktop jobs in CI):

```bash
pwsh -File ./scripts/verify-g4.ps1
```

Local Desktop iteration:

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

CI must be green (Rust job + Desktop unit/build + Playwright subset + G4 as configured).

## Code of collaboration

- Suggest = inspect; Apply = leased writes; Automate = Apply + verify  
- Spend caps require honest $/MTok (or explicit unmetered)  
- Risk categories (secrets / infra / migrate / publish) stay human-gated  

Questions? Open an issue or discuss in the PR.
