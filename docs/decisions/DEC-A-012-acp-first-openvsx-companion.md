---
layout: default
title: DEC-A-012-acp-first-openvsx-companion
---

# DEC-A-012 — ACP first (Open VSX companion retired)

- **Status:** Superseded by DEC-A-013 (2026-07-23)
- **Date:** 2026-07-22
- **Depends on:** DEC-A-010

## Original context

Open VSX is attractive but only runs on VS Code–compatible hosts. Zed extensions are Rust→Wasm and incompatible with `.vsix`.

## Original decision

- Primary editor bridge: ACP into Zed.
- Open VSX path: “Open in VSCodium” companion only.
- ADE plugins remain ADE’s own WASM lane.

## Supersession

**DEC-A-013** retires the VSCodium / Open VSX companion from the product plan to keep focus on Zed + ADE harness quality. Open VSX may still be used personally; it is not an ADE host commitment.
