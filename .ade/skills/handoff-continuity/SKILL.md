---
name: handoff-continuity
description: >-
  Create and consume ADE handoff capsules for session continuity. Use at phase
  boundaries, after verify, or when switching agents/threads.
---
# Handoff Continuity

- Capsules live under `.ade/handoff/` (`latest.json` + dated files).
- On start: load latest capsule into context (secret-scrubbed).
- On finish: save capsule with goal, phase, owned_paths, verify fold-in, and next step.
- Prefer score_max / section metrics already produced by PromptAssembler.
- Do not paste secrets into handoff bodies.
