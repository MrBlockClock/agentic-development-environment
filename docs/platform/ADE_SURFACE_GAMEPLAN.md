# ADE Surface Gameplan — Atlas, Analytics, and a Cleaner Desktop

**Schema:** `ade.surface-gameplan/v1`
**Status:** Wave 6 (Surface) · implemented · 2026-07-24
**Locked by:** AGENTS.md DNA · DEC-A-010 / 013 / 014 / 015 · skills `ui-strategy` + `progressive-ui`
**Predecessor:** `docs/research/ADE-Master-Gameplan.md` (Waves 0–5 closed the harness; Mission Control chrome deferred)

## 0. One sentence

The harness is honest but the **surface is not** — Atlas draws a fake graph, Analytics does not exist, and eleven equal nav rows hide ADE's only real differentiators (invoice honesty and verify-as-truth), so Wave 6 rebuilds the *looking* surfaces without touching the brain.

---

## 1. Diagnosis (evidence, not vibes)

### 1.1 Atlas draws a graph that lies

`apps/desktop/src/components/AtlasView.tsx` builds nodes at hardcoded coordinates:

| Problem | Evidence |
|---------|----------|
| Layout is a hardcoded grid, not a graph | `y: 180 + index * 36` with fixed `x: 40 / 280 / 420 / 520` — columns collide once two kinds share a column |
| Silently truncates the graph | `rules.slice(0, 12)`, `skills.slice(0, 12)`, `auditFindings.slice(0, 8)`, `planPhases.slice(0, 8)`, `verifyGates.slice(0, 6)`, `handoffs.slice(0, 4)` — no count, no "show all" |
| Labels truncated to 14 chars | `node.label.slice(0, 14)` inside a fixed `width={110}` rect |
| No traversal | No neighbor list, no depth control, no keyboard — click-only, whole-graph-only |
| Detail panel is a blob | Single `<pre>` of `preview` text |
| No status | A `Blocker` finding and a passing gate render identically |

### 1.2 Analytics does not exist

`apps/desktop/src/components/AnalyticsDashboard.tsx` is a **one-line comment**, imported by nothing. Meanwhile the data is already on disk and already reachable:

| Source | Command | Fields available today |
|--------|---------|------------------------|
| Usage ledger | `spend_ledger_recent` | `created_at`, `status`, `provider`, `model`, `reserved_usd`, `actual_usd`, `input_tokens`, `output_tokens` |
| Spend caps | `spend_summary` | `used_usd`, `reserved_usd`, `remaining_usd`, `daily_cap_usd`, `session_cap_usd` |
| Action envelopes | `continuity_actions_recent` | `effect`, `paths`, `autonomy`, `risk_tier`, `risk_category` |
| Verify | `run_verify` / `last_verify` | `gate`, `passed`, `status` |
| Work | `get_dashboard` | `tasks`, `handoff.recent` (`turn_status`, `score_delta`) |

**Wave 6 needs zero backend work.** Every metric below is a client-side aggregate of existing IPC.

### 1.3 The nav is nine-way-equal, which the research pack explicitly refuses

Eleven sidebar destinations + four shell-tab kinds, and the four "looking" surfaces (Trust, Plan Map, Atlas, and the missing Analytics) are scattered across two groups with two of them Debug-gated. `COMPETITIVE_RESEARCH_PACK.md` lists "nine-way equal nav" under **Refuse**.

### 1.4 No token layer

`#0d121a`, `border-white/7`, `shadow-[0_12px_45px_rgba(0,0,0,0.15)]` are copy-pasted across ~12k lines. `Panel` and `MetricCard` are private to `App.tsx` (7317 lines), so a new view cannot reuse panel chrome without duplicating it.

### 1.5 Dead code advertising unbuilt features

Six one-line stubs: `AnalyticsDashboard`, `AdminPanel`, `AgentChat`, `Layout`, `VerifyRunner`, `WorkspaceView`.

---

## 2. How everyone else does it (and what we take)

### 2.1 Graph views — the Obsidian verdict

The consistent community finding is that a **global** force graph is decorative past ~200 nodes ("a tangled web that's more fun to look at than navigate"), while the **local** graph — current node at center, neighbors by depth, pinned in a side panel — stays useful at any size. The plugins that fixed it (Excalibrain, Graph Explorer) all add the same four things: explicit hierarchy/direction, metadata filters, a side panel that keeps the graph in view, and keyboard traversal. Non-deterministic layout is called out as a defect: "the position of each note changes on every load."

| Their lesson | ADE action |
|--------------|-------------|
| Local graph beats global graph | **Focus mode is the default**: focus node + depth 1, `Map` is opt-in |
| Layout must be stable across reloads | Deterministic layout seeded by sorted node id — no force jitter, no random |
| Global graph has exactly one good use: orphan review | Ship an explicit **Orphans** filter, don't pretend the map is navigation |
| Side panel, not "open in new tab" | Keep the inspector; add a **clickable neighbor list** (the hierarchy layer) |
| Keyboard traversal | Arrows walk neighbors · `Enter` re-centers · `/` search · `f` fit · `Esc` reset |
| Metadata awareness | Filter by layer, scope, and kind; color by **status**, not just type |

We take the local-graph model and refuse force-directed physics. ADE's graph is *authority and work*, which is inherently layered — Sourcegraph-style precision (deterministic, explainable edges) beats Obsidian-style prettiness.

### 2.2 Analytics — the 2026 agent-spend consensus

Native vendor dashboards (Cursor Admin/Analytics, Copilot usage metrics, Anthropic Enterprise Analytics) are authoritative but walled per vendor. The local-first tools built to escape that (AgentsView, TokenShift, teamspend, Agentlytics) converged on a page shape and a set of honesty rules.

| Their pattern | ADE action |
|---------------|-------------|
| AgentsView page shape: summary cards → cost trend → attribution → bottom grid | Adopt directly — it is the proven layout |
| Attribution by model *and* provider, not one global total | Two attribution bars with $, tokens, and share % |
| Cursor: accepted vs suggested, model breakdown, leaderboard | Single-user local product — replace "leaderboard" with **outcome rates** (verify gates, turn status, tasks) |
| teamspend marks every derived number `is_estimated` rather than "showing a misleading exact-looking $" | **Honesty badge** per card: invoice-class actual vs reserve estimate; explicit "priced turn reported $0" counter |
| "Optimizing token spend instead of cost per completed task" is listed as the top mistake | Ship **cost per verified turn** and **cost per completed task** as first-class cards |
| On-device, no code access | Already true — local ledger, no upload, no cloud analytics in Wave 6 |

The differentiator nobody else can ship: ADE reserves budget *before* a turn and commits actuals after, so it can show **reserve accuracy (Δ)** per model. That is the visual proof of the `H1` spend-honesty contract and the answer to the "credits ≠ invoice dollars" reconciliation gap.

### 2.3 What we explicitly refuse

Cloud analytics upload · per-user leaderboards (single-operator product) · a charting dependency (inline SVG keeps the Tauri bundle honest) · force-directed physics · Atlas as a landing screen · nine-way equal nav.

---

## 3. Target IA

Four "looking" surfaces become **one destination with sub-tabs**. Nav drops 11 → 8 in Standard.

```
Sidebar                          Insight (one destination, sub-tabs)
──────────────────────────       ─────────────────────────────────────
Home            ← work           Trust      what happened · is it safe
  ├ workspaces rail              Analytics  what did it cost · did it work
  └ sessions rail                Plan Map   what is planned      (Debug)
Setup ▾         ← first run      Atlas      how it all relates   (Debug)
  Environment · Keys · Integrations
  Stack · Test project
Insight         ← looking
More ▾          ← rare
  Guidance · MCP
─────────────────
Debug · Settings
```

**The split that stops the duplication** (today spend appears in Trust, the agent strip, and Harness; findings appear in Environment, Trust, Atlas, and Plan Map):

| Surface | Owns the question | Does not own |
|---------|-------------------|--------------|
| **Trust** | "Is this safe, and what did it do?" — drift, risk, envelopes, audit log | Trend lines, model attribution |
| **Analytics** | "What did it cost, and did it work?" — trend, attribution, Δ, outcome rates | Raw audit rows, drift repair |
| **Plan Map** | "What is planned, in what order, gated by what?" | Authority relationships |
| **Atlas** | "How does authority relate to work?" | Being a dashboard |

Deep links keep working: `Audit`, `Analytics`, `Plan`, and `Atlas` remain valid `activeView` values and resolve into the Insight shell with the matching sub-tab, so the existing Guidance → Atlas, Plan → Atlas, and Verify → Atlas cross-links survive.

---

## 4. Atlas rebuild spec

| # | Ship | Done when |
|---|------|-----------|
| A1 | **Focus mode default** | Focus node centered; `Depth 1 / 2 / Map` control; opens on Workspace hub |
| A2 | **Deterministic layout** | Focus = radial rings sorted by id; Map = layered columns (Global authority · Workspace authority · Work) with no overlap at any node count |
| A3 | **No silent truncation** | All nodes built; header shows `n nodes · m edges`; visible count reflects filters |
| A4 | **Status tone** | Blocker/High findings red, Medium amber, failed gates red, passed gates emerald, failed handoffs red |
| A5 | **Neighbor traversal** | Inspector lists in/out neighbors with edge kind; each is a button that selects |
| A6 | **Keyboard** | `↑↓←→` walk neighbors · `Enter` re-center · `/` search · `f` fit · `Esc` reset |
| A7 | **Filters + legend** | Layer (Authority/Work/Both) · Scope (Global/Workspace) · kind chips · Orphans-only · legend of kind colors and edge kinds |
| A8 | **Readable nodes** | Width from label length (clamped), full label to ~26 chars, kind glyph, scope tint |

## 5. Analytics build spec

Window selector: `Today · 7d · 30d · All` over the recent ledger, labeled honestly with the row count it is aggregating.

| # | Panel | Content |
|---|-------|---------|
| N1 | Summary cards | Spend (committed) · Reserved open · Remaining vs cap · Tokens in/out · Turns · **Cost per verified turn** |
| N2 | Spend trend | Per-day stacked bars: committed actual vs open reserve, with cap reference line |
| N3 | Attribution | By model and by provider — $ , tokens, share %, turn count |
| N4 | **Reserve accuracy (Δ)** | Per model: reserved vs actual, mean Δ, over/under bias, and a **priced-turn-reported-$0** counter (the `H1` honesty detector) |
| N5 | Outcome | Verify gate pass rate · turn status mix (completed/failed/cancelled) · tasks done · **cost per completed task** |
| N6 | Ledger (collapsed) | Recent rows with Δ column and JSON export |

Every derived figure carries an `est` marker when it comes from a reserve rather than a committed actual — never an exact-looking `$` for an estimate.

## 6. Foundation work

| # | Ship | Why |
|---|------|-----|
| F1 | `@theme` tokens in `styles.css` | `surface-0..3`, `line`, `line-strong`, `accent`, `ready`, `warn`, `danger`, `info` — stop copy-pasting `#0d121a` |
| F2 | Promote primitives into `components/ui.tsx` | `Panel`, `MetricCard`, `SubTabs`, `BarSeries`, `StatBar`, `EmptyState`, `Legend` — Analytics cannot exist without shared panel chrome |
| F3 | Delete six dead stubs | They advertise features that do not exist |
| F4 | `GraphCanvas` fit signal | Atlas keyboard `f` and focus changes need to trigger fit |

## 7. Non-goals for Wave 6

Mission Control chrome · cloud analytics upload · SSO/team dashboards · charting library · `App.tsx` full decomposition (extract only what Insight needs) · new Tauri commands · touching the harness, gates, or spend math.

## 8. Success criteria

1. Standard surface exposes **8** nav destinations, not 11, and Analytics is reachable without Debug.
2. Atlas at 60+ nodes is navigable: focus + depth + neighbor list, no overlapping nodes, same layout on every reload.
3. Analytics answers "what did today cost, which model, and did it pass" in one screen with no estimate shown as an exact dollar.
4. Spend appears in exactly one analytical surface; Trust keeps the audit log.
5. `npm run build` (tsc), `npm run test:unit`, and the sidebar IA e2e spec stay green.

---

*End · `ade.surface-gameplan/v1` · 2026-07-24 · Wave 6 (Surface) — harness untouched*
