# ADE — Agent Contract

This is the canonical agent contract for the ADE project itself (dogfooding).

## Authority Order

1. Law/security/human direction
2. CI, tests, schemas
3. This AGENTS.md
4. Global guidance (`<ade-home>/guidance/rules`) + `.ade/rules/` (workspace wins body; deny-writes union)
5. Global + `.ade/skills/` on-demand skills (`*/SKILL.md`)
6. Task/issue acceptance criteria
7. Provider/adapter files
8. Chat memory

## Rules & Skills

- Rules: Global guidance + `.ade/rules/*.mdc` — merged into agent authority; workspace stem wins prompt body; `globs:` + `write: deny` union across scopes.
- Skills: Global + `.ade/skills/<name>/SKILL.md` — catalogs merge; full body when `alwaysApply: true`, match, or `activate_skill`.
- Profiles: `.ade/profiles/*.toml` and Global profiles filter `pack:`-tagged items (`active-profile.txt`).
- Cursor IDE mirrors: `.cursor/rules/` and `.cursor/skills/` (for Cursor agents; ADE runtime uses Global + `.ade/`).
- Atlas / Plan Map: Full/Debug views for authority+work graph and Trust Route (AUDIT→PLAN→EXECUTE→VERIFY).

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
