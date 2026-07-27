---
name: progressive-ui
description: >-
  ADE desktop/browser bindings for progressive disclosure: surface tokens plus
  Panel, MetricCard, SubTabs, BarSeries, StatBar, Disclosure, Hint, Chip,
  EmptyState, Legend in apps/desktop/src/components/ui.tsx; Standard/Debug only
  (Simple parked); Home = composer/agent; Setup = Environment/Keys/Stack/Test
  project; Insight = Trust · Analytics · Plan Map · Atlas sub-tabs; Workspaces =
  open/adopt/switch; Settings = defaults. Use when editing ADE App.tsx, Insight
  surfaces, Guidance/Rules views, Agent sidebars, or mode surfaces. For general
  UI strategy, also apply the global ui-strategy skill.
---
# Progressive UI (ADE)

Apply global **ui-strategy** first. This skill is ADE wiring only.
Surface plan of record: `docs/platform/ADE_SURFACE_GAMEPLAN.md`.

## Tokens (author against these, not raw hex)

`apps/desktop/src/styles.css` `@theme`:

| Group | Tokens |
|-------|--------|
| Surfaces | `surface-0` app · `surface-1` rail · `surface-2` panel · `surface-3` raised |
| Lines | `line` · `line-strong` |
| Ink | `ink` · `ink-dim` · `ink-faint` |
| Tone | `accent` · `ready` · `warn` · `danger` · `info` · `authority` |
| Elevation | `shadow-panel` |

New or rewritten UI uses `bg-surface-2 border-line shadow-panel`. Do not add
another panel style; do not re-introduce `#0d121a` literals.

## Components

`apps/desktop/src/components/ui.tsx`:

- `Panel` — panel chrome; `actions` slot for the one right-aligned control
- `MetricCard` — one figure; `estimated` renders the `est` badge
- `SubTabs` — sub-navigation *inside* one nav destination (Insight)
- `BarSeries` / `StatBar` — inline SVG/CSS charts; **no charting dependency**
- `Disclosure` — expand/collapse; `storageKey`, `forceOpen`, `lockedOpen`
- `Hint` — hover/focus/tap tooltip
- `Chip` / `ChipRow` — presets and filter toggles
- `EmptyState` — honest empty + one next action
- `Legend` — swatch/line key for graphs and charts
- `Tone`, `TONE_TEXT`, `TONE_FILL` — shared semantic colour
- `Panel` / `MetricCard` / `StatBar` take `testId` — add one when a spec needs the value, do not assert on Tailwind classes

Formatting lives in `apps/desktop/src/format.ts` — `usd`, `signedUsd`,
`compactCount`. Never hand-roll `toFixed` on money: `usd` shows cents, and drops
to four decimals only below a cent, so the same figure cannot render `$8.97` on
one surface and `$8.9700` on another.

Surfaces:

- `AnalyticsView` — spend trend · model/provider attribution · reserve Δ · outcome
- `AtlasView` — Authority/Work graph; **focus mode default**, Map is opt-in
- `PlanMap` — Plan Map spine + phase DAG
- `GraphCanvas` — pan/zoom viewport; `fitToken` bump re-fits
- `RulesEditor` — Guidance browser (Global / Workspace)
- `WorkspacesView` · `BrowserView` · `TerminalView` · `SettingsView`
- Chat transcript — `.ade/chat/thread.json` via `chat_load` / `chat_save` / `chat_clear`
- Eng-goal — `.ade/goals/{id}.json` + `active.json` via `goal_*`; Home strip Save/Run/Done

## Mode defaults (ADE)

| Surface | Default |
|---------|---------|
| Standard (`power`) | **Product default.** Home · Setup · Workspaces · **Insight** (Trust · Analytics) · **Terminal** · **Browser** |
| Debug (`dev`) | Standard + Insight gains **Plan Map · Atlas** + Editor / MCP nav + Home Harness panels |
| Simple (`guided`) | **Parked** — migrate to Standard; do not expose in UI |

**One surface control:** sidebar **footer** compact **Debug** toggle. `devMode` UI ≡ `surfaceMode === "dev"`.

A deep link (Guidance → Atlas, Plan → Atlas) may open a Debug sub-tab on
Standard rather than bouncing to Home — reveal on demand, never a dead end.

## Nav IA (usage order)

1. **Work:** Home (composer + agent)
2. **Setup:** Environment · Keys · **Integrations** · Stack · Test project — amber/green lights
3. **Context:** Workspaces · **Insight** · **Terminal** · **Browser**
4. **More:** Guidance (+ Debug: Editor · MCP)
5. **Footer:** Debug toggle · Settings gear

Eight Standard destinations. Adding a ninth requires deleting one or nesting it
as a sub-tab — the research pack refuses nine-way equal nav.

### Concept split (do not collapse)

| Surface | Job |
|---------|-----|
| **Home** | Ask ADE / Suggest / Apply in the *attached* folder |
| **Environment** | First-run readiness checklist for *this* folder |
| **Keys** | Save an API key (under Setup) |
| **Integrations** | Standing connectors (GitHub, GitLab, Stripe, Azure, MCP recipes) + host tools |
| **Stack** | Pick a project recipe (under Setup) |
| **Test project** | Run build/lint/test gates — not chat |
| **Workspaces** | New / Open / Default / **Open in Zed** (non-Default) |
| **Insight › Trust** | Is this safe, and what did it do — drift, risk, envelopes, audit log |
| **Insight › Analytics** | What it cost and whether it worked — trend, attribution, reserve Δ, outcome rates |
| **Insight › Plan Map** | What is planned, in what order, gated by what (Debug) |
| **Insight › Atlas** | How authority relates to work (Debug) |
| **Terminal / Browser** | Everyday tools on Standard — not Debug-gated |
| **Settings** | Defaults: write scope, spend caps, provider preset |

Setup group light: **amber** = recommended / incomplete · **green** = ready · **brighter amber** = failing tests / blockers.

Header: **Change folder…** + refresh (+ **New chat** on Home). **No Zed in header** — Open in Zed lives under Workspaces.

## Single-owner rule (kills duplication)

Each fact has exactly one analytical home:

| Fact | Owner | Elsewhere |
|------|-------|-----------|
| Cost trend · model/provider attribution · reserve Δ | **Analytics** | Trust shows headroom + a link; agent strip shows the live turn |
| Audit log · envelopes · ignore drift · risk waives | **Trust** | Analytics links back |
| Findings detail | Trust (Environment shows gaps as CTAs) | Atlas/Plan Map show them as nodes |
| Spend caps | Settings | Trust and Analytics link to it |

## Tier density (Standard)

| Tier | Nav | In-page default |
|------|-----|-----------------|
| 0 Work | Home | Composer/Go; Suggest/Apply; Auto model; feed |
| 1 Setup | Environment, Keys, Integrations, Stack, Test project | Status lights; gaps / one CTA |
| 1 Context | Workspaces, Insight, Terminal, Browser | Insight opens on the remembered sub-tab (Trust first run) |
| 2 More | Guidance | Browse collapsed |
| Footer | Debug toggle · Settings | Compact; Debug off by default |
| Debug More | + Editor, MCP; Insight + Plan Map, Atlas | Maps / harness density |
| Home Debug | Harness Disclosure (Verify, dogfood, rates) | Collapsed by default |

## Min/max rules

- **Min:** one control path per preference (surface, caps, autonomy, shell scope).
- **Max:** Debug adds density; Standard never requires Debug to finish a turn.
- Never hide Suggest/Apply, Workspace|Home scope, spend caps, Keys, Terminal, Browser, Trust, or Analytics behind Debug.
- Atlas is **not** the Default start screen, and **not** the default Insight tab.
- Recipes: Stack Fit Tier 0; **Browse all recipes** closed by default.
- Settings: Tier 0 = Suggest/Apply + caps + Keys link; Tier 1 = effort/provider presets.
- Layperson copy: short verbs, no BYOK/vault/L0–L11/invoice-class in Tier 0.

## Graph rules (Atlas / Plan Map)

- **Focus + depth beats whole-graph.** A global graph is structural review, not navigation.
- **Deterministic layout.** Sort by id; never random or force-jittered — a layout that moves cannot be learned.
- **No silent truncation.** Show `n of m nodes`; never `slice(0, 12)` without saying so.
- **Colour carries status**, not only type: blocking red, attention amber, passing emerald.
- **Keyboard:** `←→↑↓` neighbours · `Enter` re-centre · `/` search · `f` fit · `Esc` reset.
- **Side panel keeps the graph in view** and lists clickable neighbours.

## Number honesty (Analytics)

- Committed actual vs open reserve are different numbers — never sum them silently.
- Anything derived from a reserve carries `estimated` (`est` badge).
- Surface the `H1` detector: a priced turn reporting `$0` actual is a red flag, not a blank cell.
- Optimise **cost per completed task**, not raw token spend.
- **A cap must not set the chart scale.** `BarSeries` plots to the data peak and
  labels the reference as off-scale when it exceeds ~1.6× that peak; a $10 cap
  would otherwise flatten a real $1 day into a sliver. Label the scale (`peak …`)
  so a short bar cannot be mistaken for a small number.

## Testing Desktop-only surfaces

Analytics reads the usage ledger over Tauri IPC, so `vite preview` cannot reach
it. Instead of leaving it uncovered, `apps/desktop/e2e/fixtures/tauriStub.ts`
defines `window.__TAURI_INTERNALS__` and answers from fixtures, which puts the
app on its Tauri path in Chromium. `dashboard.json` is a captured `/api/state`
response — recapture it rather than hand-editing, so the shape keeps matching the
Rust serializers. Assert computed figures (totals, Δ, attribution order), not
just that a panel rendered.

Pure aggregation belongs in a plain module (`components/analyticsMath.ts`) with
`node --test` coverage; the money math must be verifiable without a browser.

## Layout

One tree: drawer nav under `md:`, stacks `grid-cols-1 lg:grid-cols-[…]`.
Do not fork mobile vs desktop page trees.
