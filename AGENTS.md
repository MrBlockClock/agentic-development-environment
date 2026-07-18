# ADE — Agent Contract

This is the canonical agent contract for the ADE project itself (dogfooding).

## Authority Order

1. Law/security/human direction
2. CI, tests, schemas
3. This AGENTS.md
4. `.ade/rules/` scoped rules
5. Task/issue acceptance criteria
6. Provider/adapter files
7. Chat memory

## Golden Path

- **Runtime:** Local (Windows) or WSL2
- **Root:** `C:\Users\caleb\OneDrive - Enspire LLC\Business based ADE`
- **Rust:** stable (rust-toolchain.toml)
- **Node:** v22.14.0
- **Commands:**
  - Build: `cargo build`
  - Lint: `cargo clippy -- -D warnings`
  - Format: `cargo fmt --check`
  - Test: `cargo test`

## Phases

The ADE follows AUDIT → PLAN → EXECUTE routing.

- **AUDIT:** Read-only discovery and scoring. No code changes.
- **PLAN:** Phases, gates, ownership. Read-mostly. Drafts in `docs/` or `.ade/` only.
- **EXECUTE:** Apply approved phases only. Never expand scope.

## Security

- NEVER read/quote `.env`, `*.pem`, `*.key`, `*credentials*.json`, or secrets
- NEVER commit two write-capable agents on one checkout
- NEVER merge or deploy without human approval
- NEVER disable security tools to "make AI work"

## Verify Ladder

| Gate | Command |
|------|---------|
| G0 | cargo locate-project |
| G2 | cargo fmt --check + cargo clippy -- -D warnings |
| G3 | cargo test |
