---
name: find-skills
description: >-
  Discover and install agent skills when the user asks for a skill, how to do X,
  or wants to extend capabilities. Checks local .ade/skills and the ADE skill store.
---
# Find Skills

## Local first

1. List `.ade/skills/*/SKILL.md` in this workspace.
2. Check Cursor skills: `%USERPROFILE%\.cursor\skills\*\SKILL.md`
3. Optional store: `C:\AI-Tooling\ade-skill-store\` (agents/ + cursor/)

## External ecosystem

- Browse https://skills.sh/ leaderboard for popular skills
- `npx skills find [query]` / `npx skills add <package>` when appropriate

## Before recommending

Prefer high install counts and known publishers. Present name, purpose, source, and install path. Prefer copying into `.ade/skills/<name>/` for ADE runtime visibility.
