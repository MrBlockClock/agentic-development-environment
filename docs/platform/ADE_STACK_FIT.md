# ADE Stack Fit

**Schema:** `ade.stack-fit/v1`  
**Status:** Product + implementation contract · 2026-07-20

## Positioning

A stack **recipe** is an agent **trust contract** (AGENTS.md + verify ladder + G5 evidence), not a code dump. **Stack Fit** ranks those contracts from a short interview so users get the right verify story without browsing thirteen opaque cards.

Browse facets (era / domain / search) remain available underneath recommendations. Catalog is never hidden.

## Fit variables (v1)

| Variable | Values |
|----------|--------|
| `intent` | `product` · `lib` · `ops` |
| `primary_runtime` | `rust` · `node` · `python` · `mixed` · `any` |
| `ui_surface` | `none` · `web` · `desktop` · `mobile` · `game` |
| `evidence` | `http` · `playwright` · `binary` · `device` · `hil` · `plan` · `any` |
| `compliance` | `none` · `regulated` |
| `repo_state` | `empty` · `existing` |
| `host` | `windows` · `wsl` · `macos` · `linux` · `any` |

Unset / empty answers are treated as wildcards (no score change).

## Catalog facets

| Facet | Purpose |
|-------|---------|
| `era` | `classic` · `modern` · `frontier` — browse familiar vs forward stacks |
| `domain` | e.g. `saas`, `systems`, `data-ai`, `game`, `mobile`, `desktop`, `embedded`, `oss`, `ade` |
| `tags[]` | Free-form search tokens (`axum`, `turso`, `playwright`, …) |

## Scoring

Deterministic weighted match (no LLM in v1):

1. Runtime / UI surface / evidence / intent: +points on match, mild penalty on hard mismatch.
2. `compliance: regulated` strongly boosts regulated recipes and penalizes casual SaaS.
3. G5 evidence mismatch is a hard penalty (wrong evidence story → wrong trust contract).
4. Host hints are soft (Windows/WSL-friendly recipes slight boost when host matches).
5. Results sorted by score descending; ties keep catalog order.

UI shows top matches with short `why[]` strings; full catalog stays browsable.

## Non-goals

- Community recipe marketplace / unsigned remote catalogs  
- Full application source codegen  
- Replacing AUDIT/PLAN (Fit is bootstrap; AUDIT owns brownfield scoring)  
- Scraping the public web for “every stack”

## Success criteria

- Changing Fit answers reorders recommendations without hiding browse.
- Regulated + web → `business-regulated` near the top.
- Rust + API + HTTP evidence → `rust-api-turso` near the top.
- Initialize path unchanged: AGENTS.md + `.ade/recipe.json` via transactional scaffold.
