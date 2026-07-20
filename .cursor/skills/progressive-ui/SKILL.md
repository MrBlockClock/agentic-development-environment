---
name: progressive-ui
description: >-
  ADE desktop/browser bindings for progressive disclosure: Disclosure, Hint,
  Chip in apps/desktop/src/components/ui.tsx; Simple/Full/Debug defaults;
  Guidance (Global+Workspace), Atlas, Plan Map.
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
- `PlanMap` — Trust Route spine + phase DAG
- `AtlasView` — dual-layer Authority/Work graph

## Mode defaults (ADE)

| Surface | Default |
|---------|---------|
| Simple (`guided`) | Essentials only; no harness / leases / MCP chrome |
| Full (`power`) | Dense work chrome; Guidance, Atlas, Plan Map available |
| Debug (`dev`) | Same as Full + traces, leases, task queue (`devMode`) |

## Guidance merge

Global (`<ade-home>/guidance`) + workspace `.ade/`; workspace wins body; deny union; profiles filter packs.

## Full density rules

- Header: compact **Check** + gate combo
- Agent: Model collapsed; verify-after in main column
- Prefer `Panel dense` and collapsed Disclosures

## Layout

One tree: drawer nav under `md:`, stacks `grid-cols-1 lg:grid-cols-[…]`.
Do not fork mobile vs desktop page trees.
