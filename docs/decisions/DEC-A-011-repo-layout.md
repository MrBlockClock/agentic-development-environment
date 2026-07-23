# DEC-A-011 — Repository layout (evolutionary)

- **Status:** Accepted
- **Date:** 2026-07-22
- **Depends on:** DEC-A-010
- **Doc:** [REPO_LAYOUT.md](../architecture/REPO_LAYOUT.md)

## Context

Architecture docs sketched `docs/decisions/` and a monorepo tree, but ADRs were empty and host integration had no home. A big-bang move of `crates/*` would churn Cargo paths and Desktop without shipping multi-host value.

## Decision

1. Adopt the **target tree** in `docs/architecture/REPO_LAYOUT.md`.
2. **Add** `crates/acp` and `hosts/{zed,vscodium}` now.
3. **Defer** renames/moves of existing crates; document roles in place.
4. Ship ACP as library + `ade acp` CLI subcommand (stdio), not a vendored editor.

## Consequences

- Clear place for host packs without forking IDEs.
- Workspace `Cargo.toml` gains `crates/acp`.
- Future layout PRs must be mechanical and gated by `cargo test` / Desktop smoke.
