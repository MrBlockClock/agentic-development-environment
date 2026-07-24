# ADE Orchestration → Eng-Goal Product Plan

**Schema:** `ade.orch-eng-goal/v1`  
**Status:** G0–G4 shipped (G4 = isolated Apply worktree; no Mission Control)  
**Canvas:** `ADE-master-gameplan.canvas.tsx` (live) · archive: `canvases/_archive/ADE-orchestration-eng-goal.canvas.tsx`  
**Depends on:** `IDEAL_ADE_DEVELOPMENT_PLAN.md` · competitive research pack · `AGENTS.md` Product DNA

## Verdict

Ship a **goal runner on a correct agent loop**, not a Cursor clone. Industry
consensus (2026): **model × harness × prompts** decide quality; the IDE is a
side-effect host. ADE’s differentiator stays **local harness honesty** (leases,
ToolEffect, verify, spend). **Turn failures terminate in the chat feed** (G0).
**Suggest/Apply + Workspace|Home scope** are visible and harness-enforced (G1).
**Active eng-goals persist under `.ade/goals/`** and inject into turns (G2).
**Suggest queues PLAN→tasks; Apply claims one under leases** (G3).
**Apply can isolate into a git worktree**; leases/PLAN/goals stay on primary (G4).

## Layer ownership (do not blur)

| Layer | Owns | Must not own |
|-------|------|----------------|
| **Model** | Reasoning + tool_use proposals | Filesystem, secrets, “I have no terminal” when tools exist |
| **Harness / loop** | Tool schemas, authorize, budgets, compaction, retries | Pixel UI |
| **Host (ADE Desktop)** | Sessions, Suggest/Apply, streaming, human gates | Being an editor |
| **IDE / editor** | Buffers, LSP, SCM (Cursor/VS Code) | ADE product truth |
| **Eng-goal product** | Outcome criteria + scope + verify | Endless chat theater |

## Peer orchestration patterns (research)

| Pattern | Who | Takeaway for ADE |
|---------|-----|------------------|
| Recursive planner + isolated workers | Cursor long-running agents | Roles beat flat locks; workers need isolation |
| Dual planner/actor | Windsurf Cascade | Matches Suggest → Apply |
| Isolated Agent Host process | VS Code Copilot | Side effects + session authority outside renderer |
| CLI hooks/skills/sandbox | Claude Code / Codex | Harness depth > chat chrome |
| MCP host↔client↔server | Industry ABI | Tools are syscalls; host runs the loop |

**Cursor hard lesson:** self-coordinating agents + shared locks → risk-averse
churn. Planner/worker hierarchy + isolation scaled; extra “integrator” roles
often hurt. Prompts matter as much as harness.

## ADE today vs eng-goal

**Have:** autonomy dial, ToolEffect, leases, verify gates, MCP + host shell/fs,
chat persistence, option chips, **G0–G3**, **G4 Isolate checkbox** (worktree
Apply; gold-set 51/51 prerequisite met), **structured next-actions** fence
(`ade.next-actions/v1`; regex fallback remains).

**Partial:** —

**Not yet:** Mission Control / multi-agent dashboard chrome (explicit non-goal
for now — isolation without Agents Window clone).

## Roadmap

### G0 — Reliability — **Done**
### G1 — Intent surface — **Done**
### G2 — Eng-goal object — **Done**
### G3 — Role split — **Done**

### G4 — Parallel isolation — **Done** (product slice)
Prerequisite: gold-set gate green (`ade eval --gold` 51/51).
- Home **Isolate** toggle on Role split strip.
- Apply provisions `.ade/worktrees/{task_id}` via `WorktreeManager`.
- `run_agent_turn(execution_root)` tools run in worktree; `coordination_root`
  keeps leases / PLAN / eng-goals / handoff on primary checkout.
- Successful Apply force-removes the worktree; failures leave it for review.
- No Mission Control UI — copy Cursor’s isolation lesson only.
- Dogfood: `scripts/dogfood-isolate-apply.ps1` · `docs/platform/G4_DOGFOOD_ISOLATE_APPLY.md`
  (`ade worker run --once --worktree` for optional live path).

### Structured next-actions — **Done**
- Host parses fenced `ade.next-actions` / `ade.options` JSON
  (`schema: ade.next-actions/v1`) into the same option chips as prose lists.
- Items may use `{ label, prompt }`; click still dispatches via `onSelectOption`.
- Regex OPTION_LEAD lists remain the fallback.
- Suggest autonomy clause nudges the fence when offering choices.

## Non-goals
- Forking VS Code / replacing Cursor as daily IDE.
- Growing T0 to explain UI (client renders intents).
- Multi-agent Mission Control chrome (deferred indefinitely unless gold + demand).

## Alignment checklist (DNA)

| Contract | Location |
|----------|----------|
| Layer ownership | `AGENTS.md` Product DNA |
| Ideal spine | `IDEAL_ADE_DEVELOPMENT_PLAN.md` |
| This roadmap | `ORCHESTRATION_ENG_GOAL_PLAN.md` |
| Autonomy enum | `crates/agents/src/autonomy.rs` |
| ToolEffect | `crates/agents/src/authority.rs` |
| Turn loop | `crates/agents/src/turn.rs` + `session.rs` |
| Host terminalization | `apps/desktop/src/App.tsx` `runAgentTurn` / AgentView |
| Shell scope | `preferred_shell_cwd` on turn builder + Desktop scope chip |
| Eng-goal | `crates/agents/src/goal.rs` · `.ade/goals/` · Desktop strip |
| Tasks | `crates/workflow/src/tasks.rs` · Home Role split strip |
| Worktrees | `WorktreeManager` · `worktree_provision_for_task` · Isolate toggle |

## Immediate next engineering slice
**Effort / turn-budget honesty (B0–B4)** — see
[`EFFORT_TURN_BUDGET_PLAN.md`](./EFFORT_TURN_BUDGET_PLAN.md) · canvas
`ADE-effort-budget-gameplan`.

Orch spine (G0–G4 + next-actions) and Ideal **E2 Monaco** remain closed. Mission
Control still held. Continuity dogfood script:
`scripts/dogfood-continuity.ps1`.
