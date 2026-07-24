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
- `WorkspacesView` — New project / Open / More (adopt) / recent / switch / Open in Zed
- `BrowserView` — Desktop in-app WebView2 window launcher
- `SettingsView` — durable defaults only (caps, Suggest/Apply, Keys link)
- `TerminalView` — Desktop PTY
- Chat transcript — `.ade/chat/thread.json` via `chat_load` / `chat_save` / `chat_clear`
- Eng-goal — `.ade/goals/{id}.json` + `active.json` via `goal_*`; Home strip Save/Run/Done

## Mode defaults (ADE)

| Surface | Default |
|---------|---------|
| Standard (`power`) | **Product default.** Home · Setup · Workspaces · Trust · **Terminal** · **Browser** |
| Debug (`dev`) | Standard + Atlas / Plan Map / Editor / MCP + Home Harness panels |
| Simple (`guided`) | **Parked** — migrate to Standard; do not expose in UI |

**One surface control:** sidebar **footer** compact **Debug** toggle (not a top dual button). `devMode` UI ≡ `surfaceMode === "dev"`.

## Nav IA (usage order)

1. **Work:** Home (composer + agent) — dominant center surface  
2. **Context (compact rail):** Workplace switcher · agent Sessions  
3. **Setup (collapsed):** Environment · Keys · Stack · Test — expand when amber  
4. **Trust** · **More (collapsed):** Guidance (+ Debug: Atlas · Plan Map · MCP)  
5. **Footer:** Debug toggle · Settings gear  

Header tabs: Agent sessions + Terminal / Browser / Editor (tools docked as tabs, not left-nav peers).

### Concept split (do not collapse)

| Surface | Job |
|---------|-----|
| **Home** | Ask ADE / Suggest / Apply in the *attached* folder |
| **Environment** | First-run readiness checklist for *this* folder |
| **Keys** | Save an API key (under Setup) |
| **Stack** | Pick a project recipe (under Setup) |
| **Test project** | Run build/lint/test gates — not chat |
| **Workspaces** | New / Open / Default / **Open in Zed** (non-Default) |
| **Terminal / Browser** | Everyday tools on Standard — not Debug-gated |
| **Atlas / Plan Map** | Debug maps — not the Default landing experience |
| **Trust** | Ignore drift, spend vs caps, audit log |
| **Settings** | Defaults: write scope, spend caps, provider preset |

Setup group light: **amber** = recommended / incomplete · **green** = ready · **brighter amber** = failing tests / blockers.

Header: **Change folder…** + refresh (+ **New chat** on Home). **No Zed in header** — Open in Zed lives under Workspaces.

## Tier density (Standard)

| Tier | Nav | In-page default |
|------|-----|-----------------|
| 0 Work | Home | Composer/Go; Suggest/Apply; Auto model; feed |
| 1 Setup | Environment, Keys, Stack, Test project | Status lights; gaps / one CTA |
| 1 Context | Workspaces, Trust, Terminal, Browser | Continuity strip when handoff |
| 2 More | Guidance | Browse collapsed |
| Footer | Debug toggle · Settings | Compact; Debug off by default |
| Debug More | + Atlas, Plan Map, Editor, MCP | Maps / harness density |
| Home Debug | Harness Disclosure (Verify, dogfood, rates) | Collapsed by default |

## Min/max rules

- **Min:** one control path per preference (surface, caps, autonomy, shell scope).  
- **Max:** Debug adds density; Standard never requires Debug to finish a turn.  
- Never hide Suggest/Apply, Workspace|Home scope, spend caps, Keys, Terminal, or Browser behind Debug.  
- Atlas is **not** the Default start screen.  
- Recipes: Stack Fit Tier 0; **Browse all recipes** closed by default.  
- Settings: Tier 0 = Suggest/Apply + caps + Keys link; Tier 1 = effort/provider presets.  
- Layperson copy: short verbs, no BYOK/vault/L0–L11/invoice-class in Tier 0.

## Standard density rules

- Header: **Change folder…** + refresh (+ New chat on Home).  
- Home: Composer pinned at bottom; one scroll for activity/response above. Harness only in Debug.  
- Prefer `Panel dense` and collapsed Disclosures.  
- Workspaces: New project + Open folder + Default Tier 0; Open in Zed for non-Default; adopt / ADE source under More.

## Layout

One tree: drawer nav under `md:`, stacks `grid-cols-1 lg:grid-cols-[…]`.
Do not fork mobile vs desktop page trees.
