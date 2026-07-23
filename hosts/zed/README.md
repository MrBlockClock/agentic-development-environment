# Host pack: Zed

**Role:** Primary coding eyes for ADE (native Rust editor).  
**Bridge:** [Agent Client Protocol](https://zed.dev/acp) → run `ade acp` as a custom agent.  
**Non-goal:** Day-one fork of Zed; Open VSX / VSCodium (retired — DEC-A-013).  
**Fork:** Only via gated ladder L3/L4 in DEC-A-013 after ACP dogfood proves chrome gaps.

## Prerequisites

- Zed installed (Win/Mac/Linux) — e.g. `%LOCALAPPDATA%\Programs\Zed\Zed.exe`
- `gh` logged in (`gh auth status`) for GitHub MCP
- Local `ade` build for ACP dogfood: `cargo build -p ade-cli`

## Cursor-like defaults (already applied on this machine)

User settings: `%APPDATA%\Zed\settings.json`

| Setting | Value |
|---------|--------|
| Keymap | `base_keymap: Cursor` |
| Format on save | on (Rust via rust-analyzer) |
| Git inline blame | on |
| Telemetry | off |
| Agents | Cursor + GitHub Copilot (registry) + ADE (custom scaffold) |
| GitHub MCP | `run-github-mcp.cmd` → `gh auth token` + `@modelcontextprotocol/server-github` |

Project settings: `ade/.zed/settings.json`

## GitHub robustness checklist

1. `gh auth status` — repo + workflow scopes (done if exploring this pack).
2. Open **Git** panel in Zed — stage/commit/push against `origin`.
3. Agent Panel → pick **Cursor** or **GitHub Copilot** → ask about open PRs / issues (MCP tools).
4. Command Palette → `git: …` actions; confirm blame on a file.
5. Optional: ACP Registry → install Claude / Codex for more agent surfaces.

## ADE agent (soft shell · Z1)

```json
"ADE": {
  "type": "custom",
  "command": "C:\\Dev\\ade-target\\debug\\ade.exe",
  "args": ["acp"],
  "env": {}
}
```

`ade acp` speaks ACP JSON-RPC over stdio: `initialize` · `session/new` · `session/set_mode` · `session/prompt` · `session/cancel`. Modes map to Suggest / Apply / Automate. Full live-LLM turns remain on ADE Desktop; this shell is coding eyes + harness guidance.

From Desktop: header **Zed** opens the attached workspace in Zed (`open_in_zed`).

**Fork ladder:** DEC-A-015 — stay at **L1 soft shell**. Do not fork unless written chrome gaps ACP+Desktop cannot close.

Probe only (CI / smoke):

```powershell
ade acp --probe
```

## Dogfood

```powershell
& "$env:LOCALAPPDATA\Programs\Zed\Zed.exe" c:\Dev\ade
```

Open Agent Panel (`Ctrl+?` / agent settings), Threads sidebar for parallel agents — compare feel vs Cursor.
