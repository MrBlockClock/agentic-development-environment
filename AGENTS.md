# ADE — Agent Contract

This is the canonical agent contract for the ADE project itself (dogfooding).

## Authority Order

1. Law/security/human direction
2. CI, tests, schemas
3. This AGENTS.md
4. `.ade/rules/` scoped rules (`.mdc`)
5. `.ade/skills/` on-demand skills (`*/SKILL.md`)
6. Task/issue acceptance criteria
7. Provider/adapter files
8. Chat memory

## Rules & Skills

- Rules: `.ade/rules/*.mdc` — always loaded into agent authority context; `globs:` + `write: deny` enforce path policy.
- Skills: `.ade/skills/<name>/SKILL.md` — catalog always listed; full body injected when `alwaysApply: true` or the user prompt matches the skill.
- Cursor IDE mirrors: `.cursor/rules/` and `.cursor/skills/` (for Cursor agents; ADE runtime uses `.ade/` only).

## Golden Path

- **Runtime:** Local (Windows) or WSL2
- **Root:** `C:\Dev\ade`
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
