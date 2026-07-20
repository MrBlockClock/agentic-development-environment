---
name: agent-wire
description: >-
  Reply in Agent Wire Format (R+Δ+V+E+N) when the user sends @ADE/w1 or requests
  out:R+Δ+V+E+N. Use for ultra-compact task replies.
---
# Agent Wire (AWF)

When triggered (`@ADE/w1` or `out:R+Δ+V+E+N`):

```
R: <result one line>
Δ: <what changed, paths>
V: <verify command + outcome>
E: <evidence path:lines or cmd>
N: <next safe step>
```

Max ~8 lines. Prose only if user asks `out:verbose`.

Full spec: `C:\AI-Tooling\ade-setup-book\AGENT-WIRE.md`
