---
name: progressive-ui
description: >-
  ADE desktop/browser bindings for progressive disclosure: Disclosure, Hint,
  Chip in apps/desktop/src/components/ui.tsx; Standard/Debug only (Simple
  parked); Home = composer/agent; Environment = setup audit; Workspaces = open/
  adopt/switch; Settings = defaults; Checks under Configure; Atlas/Plan Map peers.
  Use when editing ADE App.tsx, Guidance/Rules views, Agent sidebars, or mode
  surfaces. For general UI strategy, also apply the global ui-strategy skill.
---
# Progressive UI (ADE)

Apply global **ui-strategy** first. This skill is ADE wiring only.

## Components

`apps/desktop/src/components/ui.tsx`:

- `Disclosure` — expand/collapse; `storageKey`, `forceOpen`, `lockedOpen`
- `Hint` — hover/focus/tap tooltip
- `Chip` / `ChipRow` — presets (models, prompts, providers)
- `RulesEditor` — Guidance browser (Global / Workspace)
- `PlanMap` — Plan Map spine + phase DAG
- `AtlasView` — dual-layer Authority/Work graph
- `WorkspacesView` — open / create-adopt / recent / switch folders
- `BrowserView` — Desktop in-app WebView2 window launcher
- `SettingsView` — durable defaults only (caps, Suggest/Apply, Keys link)
- `TerminalView` — Desktop PTY
- Chat transcript — `.ade/chat/thread.json` via `chat_load` / `chat_save` / `chat_clear`
- Eng-goal — `.ade/goals/{id}.json` + `active.json` via `goal_*`; Home strip Save/Run/Done

## Mode defaults (ADE)

| Surface | Default |
|---------|---------|
| Standard (`power`) | **Product default.** Slim rail; no harness chrome. |
| Debug (`dev`) | Standard + traces, leases, Audit/Browser/Terminal/MCP nav, Observe/Automate |
| Simple (`guided`) | **Parked** — migrate to Standard; do not expose in UI |

**One surface control:** sidebar Standard / Debug. Do **not** add a second “Debug panels” toggle. `devMode` UI ≡ `surfaceMode === "dev"`.

## Nav IA (usage order)

1. **Work:** Home only (composer + agent session — one surface)  
2. **Context (Standard):** Environment, Workspaces, Atlas, Plan Map, Trust  
3. **Configure (Standard):** Recipes, Guidance, Keys, Checks  
4. **Settings:** sidebar **footer gear only** (not listed under Configure)  
5. **Debug-only nav:** Browser, Terminal, MCP  

### Concept split (do not collapse)

| Surface | Job |
|---------|-----|
| **Home** | Ask ADE / Suggest / Apply in the *attached* folder |
| **Environment** | Audit *this* folder’s setup gaps (keys, recipe, checks, blockers) |
| **Workspaces** | Open / Create-Adopt / Recent / Switch which folder is attached |
| **Trust** | Ignore drift, spend vs caps, audit log (scoreboard — not Home) |
| **Settings** | Defaults: write scope, spend caps, provider preset — **one entry, footer gear** |
| **Keys** | Credentials only (vault) — linked from Settings |

**Checks** (view id `Verify`) = gate *evidence* under Configure.

Header always shows **Working in `{folder}`** + **Change…** → Workspaces.

## Tier density (Standard)

| Tier | Nav | In-page default |
|------|-----|-----------------|
| 0 Work | Home | Composer/Go; Suggest/Apply; scope; eng-goal; Role split + Isolate; model · Effort |
| 1 Context | Environment, Workspaces, Atlas, Plan, Trust | Continuity / metrics collapsed where dense |
| 2 Configure | Recipes, Guidance, Keys, Checks | Browse catalog / smoke / pricing collapsed |
| Footer | Settings (gear) | One path only — never also under Configure |
| Debug | + Browser, Terminal, MCP | Harness advanced, Observe/Automate, rebuild locks |

## Min/max rules

- **Min:** one control path per preference (surface, caps, autonomy, shell scope).  
- **Max:** Debug adds density; Standard never requires Debug to finish a turn.  
- Never hide Suggest/Apply, Workspace|Home scope, spend caps, or Keys behind Debug.  
- Recipes: Stack Fit Tier 0; **Browse all recipes** closed by default.  
- Settings: Tier 0 = Suggest/Apply + caps + Keys link; Tier 1 = effort/provider presets.

## Standard density rules

- Header: **Change…** + refresh only.  
- Home: Composer pinned at bottom; one scroll for activity/response above. Advanced only in Debug.  
- Prefer `Panel dense` and collapsed Disclosures.

## Layout

One tree: drawer nav under `md:`, stacks `grid-cols-1 lg:grid-cols-[…]`.
Do not fork mobile vs desktop page trees.
