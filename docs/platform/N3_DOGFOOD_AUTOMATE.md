# N3 — Dogfood Automate acceptance

**Schema:** `ade.n3-dogfood/v1`  
**Status:** Done · live pass 2026-07-21 · verify G0–G3  
**Canvas:** archived — [`ADE-n3-dogfood.canvas.tsx`](C:\Users\caleb\.cursor\projects\c-Dev\canvases\_archive\ADE-n3-dogfood.canvas.tsx) · live board: [`ADE-master-gameplan.canvas.tsx`](C:\Users\caleb\.cursor\projects\c-Dev\canvases\ADE-master-gameplan.canvas.tsx)

## Goal

Prove ADE can run an **Automate** turn against the ADE repo itself with:

1. Explicit write scope (`--approve-owned-paths` + owned path under `.ade/dogfood/`)
2. **Verify-on-complete through G3** (required by Automate autonomy)
3. No self-certify — gates must pass
4. Rebuild-lock honesty when Desktop/`ade.exe` holds binaries

## Exit criteria

| Check | Pass |
|-------|------|
| Script | `pwsh -File scripts/dogfood-automate.ps1` exits 0 |
| Evidence | `.ade/dogfood/automate-acceptance.md` exists and notes the run |
| Verify | Turn completes with verify G3 success (or script reports gate failure honestly) |
| Scope | No edits outside `.ade/dogfood/` |

## Operator steps

1. Workspaces → **Open ADE on itself** (or attach `C:\Dev\ade`).
2. Keys: OpenCode Zen or FreeLLMAPI configured.
3. Quit Desktop / `ade serve` if you need a clean rebuild afterward.
4. Run: `pwsh -File scripts/dogfood-automate.ps1`  
   (script seeds a G1 eng-goal contract; uses tiny non-zero $/MTok so verify-on-complete cargo tests are not poisoned by `ADE_ALLOW_UNPRICED`)
5. Optional Desktop Debug: chip **Dogfood Automate** → Go.

## Rebuild lock

Windows refuses overwriting `ade-desktop-app.exe` / `ade.exe` while running (**os error 5**). Automate must not treat that as a model failure — stop processes, rebuild, relaunch.

## Work packages

| WP | Work | Status |
|----|------|--------|
| WP37 | This doc + Ideal plan N3 section | Done |
| WP38 | `scripts/dogfood-automate.ps1` | Done |
| WP39 | Debug Home “Dogfood Automate” chip | Done |
| WP40 | Live pass evidence on ADE repo | Done |

## Related

- `docs/platform/IDEAL_ADE_DEVELOPMENT_PLAN.md` (N3)
- Live board: `ADE-master-gameplan.canvas.tsx`
- Archived: `canvases/_archive/ADE-nav-ia.canvas.tsx` (Home / Environment / Workspaces)
