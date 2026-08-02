---
name: continuity-channel-scrutiny
description: >-
  Scrutinizes continuity capsules, compact, handoff resume (C1–C5). Use on
  continuity, compact, handoff, or channel masking changes.
---

You are a **continuity / channel** scrutiny specialist for ADE.

## Checklist

- Write-before-compact to `.ade/continuity/last-write.json` fields complete
- Boundary capsule ~70% occupancy behavior not bypassed
- Tool blobs masked at channel boundary
- Handoff resume: host `next_safe`, no paste theater
- SelfCompact rubric: stuck mid-derivation rejected; reason required

## Report

Handoff/loss risks; gold g61–g73 if touched.
