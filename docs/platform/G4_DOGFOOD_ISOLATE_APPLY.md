# G4 — Dogfood Isolate Apply

**Schema:** `ade.g4-isolate-dogfood/v1`  
**Status:** Harness script shipped · live worker optional (`-Live`)  
**Depends on:** Orch G4 · `ORCHESTRATION_ENG_GOAL_PLAN.md`

## Goal

Prove Isolate Apply’s worktree path on a real queued task:

1. Enqueue + claim + start a task with owned paths under `.ade/dogfood/isolate/`
2. Provision `.ade/worktrees/{task_id}` (same layout as Desktop Isolate)
3. Writes land in the worktree only (primary checkout stays clean)
4. Complete task and force-remove the worktree

## Exit criteria

| Check | Pass |
|-------|------|
| Script | `pwsh -File scripts/dogfood-isolate-apply.ps1` exits 0 |
| Evidence | `.ade/dogfood/isolate-acceptance.md` exists |
| Isolation | Marker existed under worktree owned path; not on primary before cleanup |
| Cleanup | `.ade/worktrees/{task_id}` gone after success |

## Operator steps (CLI)

```powershell
pwsh -File scripts/dogfood-isolate-apply.ps1
# optional live agent worker:
pwsh -File scripts/dogfood-isolate-apply.ps1 -Live
```

## Operator steps (Desktop)

1. Attach ADE repo; ensure PLAN has phases with `owned_paths` (or enqueue via CLI).
2. Home → Role split → enable **Isolate**.
3. **Queue PLAN** (if needed) → **Apply next**.
4. Confirm note shows `Isolated · …\.ade\worktrees\{task}`.
5. On success: worktree cleaned; on failure: worktree left for review.

## Related

- `scripts/dogfood-isolate-apply.ps1`
- `ade worker run --worktree --cleanup-worktree --once --approve`
- Desktop `worktree_provision_for_task` + `executionRoot`
