# Architecture decision records

ADRs for ADE. Format: `DEC-A-NNN-slug.md`.

| ID | Title | Status |
|----|-------|--------|
| [DEC-A-010](DEC-A-010-multi-host-agent-os.md) | Multi-host Agent OS (Zed + Desktop) | **Superseded in part by 017** |
| [DEC-A-011](DEC-A-011-repo-layout.md) | Repository layout (evolutionary) | Accepted (hosts/acp clauses outdated) |
| [DEC-A-012](DEC-A-012-acp-first-openvsx-companion.md) | ACP first; Open VSX companion | **Superseded** |
| [DEC-A-013](DEC-A-013-zed-only-fork-ladder.md) | Zed-only hosts; gated fork ladder | **Superseded by 017** |
| [DEC-A-014](DEC-A-014-harness-first-zed-optional.md) | Harness-first; Zed host optional | Accepted (harness); Zed path **superseded by 017** |
| [DEC-A-015](DEC-A-015-z2-fork-ladder-review.md) | Z2 fork ladder review | **Superseded by 017** |
| [DEC-A-016](DEC-A-016-security-baseline.md) | Security baseline (spend/MCP/path/CSP) | Accepted |
| [DEC-A-017](DEC-A-017-retire-zed-host.md) | Retire Zed / ACP host path | Accepted |

**Note:** Older synthesis docs that cite `DEC-A-001…003` / `DEC-P-*` without files are **non-binding** until filed. Zed/ACP host packs are product non-goals.

Canonical layout: [`../architecture/REPO_LAYOUT.md`](../architecture/REPO_LAYOUT.md).  
Master canvas: `ADE-master-gameplan.canvas.tsx` · H-detail: `ADE-harness-multiagent-gameplan.canvas.tsx`.
