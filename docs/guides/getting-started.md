# Getting started with ADE

ADE is a **local Agent Development Environment**: Rust harness + Tauri Desktop control plane + CLI. This guide gets you building and running on a developer machine.

## Prerequisites

| Tool | Notes |
|------|--------|
| Rust | Stable toolchain (`rustup default stable`) |
| Node.js | **22+** (Desktop Vite / Tauri) |
| WebView2 | Windows — required for Desktop |
| Git | Clone + hooks as needed |

Optional: a provider API key (BYOK) stored via Desktop → **Keys** (OS vault). Prefer vault over committing `.env` secrets.

## Clone & configure

```bash
git clone https://github.com/MrBlockClock/agentic-development-environment.git
cd agentic-development-environment
cp .env.example .env
```

Edit `.env` only for local overrides. Never commit real keys. Desktop can import env keys into the OS vault with an explicit action (`ADE_IMPORT_ENV_KEYS=1` — see `.env.example`).

## Build & verify (Rust)

```bash
cargo build
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

CLI help:

```bash
cargo run -p ade-cli -- --help
```

## Desktop (Tauri)

```bash
cd apps/desktop
npm install
npm run tauri dev
```

- Vite binds **`http://127.0.0.1:1420`** (IPv4) so WebView2 does not hit `ERR_CONNECTION_REFUSED`.
- Keep the `tauri dev` terminal open — closing it stops Vite and the UI cannot load.
- First-run: attach a workspace folder, then **Setup → Keys** for a provider.

### Useful Desktop destinations

| Nav | Purpose |
|-----|---------|
| Home | Ask ADE · Suggest / Apply / Automate |
| Setup → Keys | BYOK OS vault |
| Setup → Integrations | GitHub, Stripe, Azure, MCP recipes |
| Insight → Trust | Audit, risk, envelopes |
| Insight → Analytics | Spend trend & attribution (Desktop) |

## Dogfood scripts (optional)

See `docs/platform/N3_DOGFOOD_AUTOMATE.md` and `docs/platform/G4_DOGFOOD_ISOLATE_APPLY.md` for Automate / Isolate Apply acceptance paths under `.ade/dogfood/`.

## Gold evals

```bash
cargo run -p ade-cli -- eval --gold
```

Gold races cover spend honesty, slots, continuity, risk gates, and more (`AGENTS.md` H5 / C5).

## Next reading

- [Architecture / repo layout](../architecture/REPO_LAYOUT.md)
- [Surface gameplan](../platform/ADE_SURFACE_GAMEPLAN.md)
- [Master gameplan](../research/ADE-Master-Gameplan.md)
- [AGENTS.md](../../AGENTS.md) — product DNA
- [Wiki](https://github.com/MrBlockClock/agentic-development-environment/wiki)
