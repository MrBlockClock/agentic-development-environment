# Host integration packs — retired

ADE’s product surface is **Desktop + CLI** only. External editor host packs (Zed, VSCodium) are **non-goals** ([DEC-A-017](../docs/decisions/DEC-A-017-retire-zed-host.md)).

| Pack | Status |
|------|--------|
| `zed/` | **Removed** — was never a product goal |
| [vscodium/](vscodium/) | **Retired** tombstone only |

Harness truth stays in Desktop, CLI, and `crates/*` — not in editor forks or soft shells.
