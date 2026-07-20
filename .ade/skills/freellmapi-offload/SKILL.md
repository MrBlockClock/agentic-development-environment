---
name: freellmapi-offload
description: >-
  Offload large codegen, reviews, and analysis to FreeLLMAPI MCP tools. Use when
  generating big patches, multi-file reviews, or planning docs to save premium tokens.
---
# FreeLLMAPI Offload

1. Read `C:\AI-Tooling\freellmapi\FREELLMAPI-BASE.md` only if endpoint/model facts are needed.
2. Use MCP server `user-freellmapi`:
   - `freellmapi_code` — codegen / refactor drafts
   - `freellmapi_review` — reviews
   - `freellmapi_chat` — planning / analysis
3. Keep Cursor/ADE model for tools, git, terminal, small edits, and applying FreeLLMAPI drafts.
4. If MCP unreachable, note once and continue with built-in model.
