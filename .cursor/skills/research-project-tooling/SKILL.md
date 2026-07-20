---
name: research-project-tooling
description: >-
  Research the current best tools, extensions, linters, test frameworks, and MCP
  servers for a given stack, then propose a tailored setup. Use when starting a
  new project, adopting an unfamiliar stack, or when the user asks what to use
  for X.
---
# Research Project Tooling

1. Identify the stack (ask, or detect from repo manifests).
2. Web-search, with the **current year** in queries:
   - `"<stack> recommended tooling <year>"` (linter, formatter, test runner)
   - `"<stack> VS Code extensions <year>"`
   - `"<stack> MCP server"`
   - `"<stack> project structure best practices <year>"`
3. Cross-check at least 2 sources per recommendation; prefer official docs.
4. Present a table: tool → purpose → why this over alternatives → install cmd.
5. On approval: install, configure, write/update project rules under `.ade/rules/` and `.cursor/rules/`.
6. Save findings to `<repo>/docs/tooling-decisions.md` with the date.

Canonical book: `C:\AI-Tooling\ade-setup-book\BOOK.md` Part 2 step 5.
