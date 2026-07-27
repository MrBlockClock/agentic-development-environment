# Security Policy

## Reporting a vulnerability

If you discover a security issue in ADE (secret handling, vault, path policy, auth), please **do not** open a public issue with exploit details.

Email the maintainer via GitHub profile contact, or open a **private** security advisory on the repository if enabled.

## Scope

In scope: credential vault, SensitivePathPolicy, lease/isolate boundaries, spend ledger integrity, MCP spawn approval.

Out of scope: third-party model providers, upstream Tauri/WebView2 CVEs (report upstream; link here if ADE-specific).

## Secrets hygiene

- Never commit `.env` (use `.env.example` only)
- Prefer Desktop → Keys (OS vault) for BYOK
- Rotate any key that may have been exposed in logs or chat
