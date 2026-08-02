# Host pack: Zed

**Role:** Primary coding eyes for ADE (native Rust editor).  
**Bridge:** [Agent Client Protocol](https://zed.dev/acp) → run `ade acp` as a custom agent.  
**Non-goal:** Day-one fork of Zed; Open VSX / VSCodium (retired — DEC-A-013).  
**Fork:** Only via gated ladder L3/L4 in DEC-A-013 after ACP dogfood proves chrome gaps.

## Prerequisites

- Zed installed (Win/Mac/Linux)
- Local `ade` on `PATH`, or use `cargo run -p ade-cli -- …`
- Optional: `gh auth status` if you use GitHub MCP from Desktop/Integrations

## Example config

Copy [`settings.example.json`](settings.example.json) into your Zed **user** settings (`%APPDATA%\Zed\settings.json` on Windows, `~/.config/zed/settings.json` on Linux/macOS) under `agent_servers`, or merge the `ADE` block:

```json
"ADE": {
  "type": "custom",
  "command": "ade",
  "args": ["acp"],
  "env": {}
}
```

Prefer `"command": "ade"` on PATH. For a debug build without installing:

```json
"command": "cargo",
"args": ["run", "-q", "-p", "ade-cli", "--", "acp"]
```

(Working directory should be the ADE repo root, or point `command` at your release binary.)

Project-local `.zed/settings.json` is optional and gitignored — keep machine-specific paths out of the repo.

## Smoke

```powershell
cargo run -p ade-cli --quiet -- acp --probe
```

`ade acp` speaks ACP JSON-RPC over stdio: `initialize` · `session/new` · `session/set_mode` · `session/prompt` · `session/cancel`. Modes map to Suggest / Apply / Automate. Full live-LLM turns remain on ADE Desktop; this shell is coding eyes + harness guidance.

From Desktop: header **Zed** opens the attached workspace in Zed (`open_in_zed`).

**Fork ladder:** DEC-A-015 — stay at **L1 soft shell**. Do not fork unless written chrome gaps ACP+Desktop cannot close.

## Dogfood

1. Build CLI: `cargo build -p ade-cli`
2. Ensure `ade` is on PATH **or** use the `cargo run … acp` agent_servers block above
3. Open the ADE workspace in Zed → Agent Panel → pick **ADE**
4. Compare Threads feel vs Cursor; keep spend/keys/MCP on Desktop

See [DEC-A-010](../../docs/decisions/DEC-A-010-multi-host-agent-os.md) and [hosts/README](../README.md).
