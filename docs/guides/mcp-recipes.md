---
layout: default
title: MCP recipes (GitHub · Linear)
---

# MCP recipes

ADE Desktop **Setup → Integrations** ships reviewed **stdio MCP recipes**. Vault tokens are storage only until you spawn the matching server; green Connected means MCP is live this session.

## One-click Add MCP

1. Save a token under the connector (OS vault) when the recipe needs env injection.  
2. On **Integrations**, use **Add MCP** on the row, or the **Add from recipe** strip (GitHub / Linear).  
3. Confirm the spawn command in the dialog — that single confirm is the approval.  
4. Confirm tools appear under **Host tools** / MCP console before treating the turn as ready.

Incomplete Azure-style recipes (vault + external env) spawn as **warn**, not fully ready.

## GitHub

| | |
|--|--|
| Recipe name | `github` |
| Package | `@modelcontextprotocol/server-github` |
| Windows | `npx.cmd -y @modelcontextprotocol/server-github` |
| Unix | `npx -y @modelcontextprotocol/server-github` |
| Vault id | `github` |
| Injected env | `GITHUB_PERSONAL_ACCESS_TOKEN`, `GITHUB_TOKEN` |
| Token URL | [github.com/settings/tokens](https://github.com/settings/tokens) |
| Upstream | [MCP server-github](https://github.com/modelcontextprotocol/servers/tree/main/src/github) |

**Typical agent uses:** list issues/PRs, search code, open draft PR comments — only after Connect MCP.

## Linear

| | |
|--|--|
| Recipe name | `linear` |
| Package | `mcp-linear` |
| Windows | `npx.cmd -y mcp-linear` |
| Unix | `npx -y mcp-linear` |
| Vault id | `linear` |
| Injected env | `LINEAR_API_KEY` |
| Token URL | [linear.app/settings/api](https://linear.app/settings/api) |

**Typical agent uses:** search issues, update status, link Continuity next-safe work to a ticket.

## Continuity dogfood (PDF + MCP)

Scripted path (CLI thrift resume + owned-path evidence):

```powershell
pwsh -File scripts/dogfood-continuity-pdf-mcp.ps1
```

Desktop path:

1. Attach a PDF on Home → **Extract** → `.ade/inbox/*.extract.md`.  
2. **Integrations** → one-click **Add MCP** for GitHub or Linear (token saved first).  
3. Continuity strip → Continue / thrift resume.  
4. Ask the agent to use extract markdown **and** MCP search (issues/PRs), write evidence only under `.ade/dogfood/`.

See also: `scripts/dogfood-continuity.ps1` (thrift-only) and [Desktop wiki](../wiki/Desktop.html).

## Other catalog recipes

GitLab, Slack, Stripe, and Azure appear in the same Integrations catalog with the same Connect MCP flow. Azure injects `AZURE_CLIENT_SECRET` from the vault; you must also set `AZURE_TENANT_ID` and `AZURE_CLIENT_ID` in the Desktop process env.
