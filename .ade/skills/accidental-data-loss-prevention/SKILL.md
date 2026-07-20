---
name: accidental-data-loss-prevention
description: >-
  STOP AND VERIFY before irreversible data loss. Use for DROP/TRUNCATE/broad
  DELETE, production bucket deletes, project/resource destruction, secret or
  KMS key destruction. Always obtain explicit user consent first.
alwaysApply: true
---
# Accidental Data Loss Prevention

> **STOP AND VERIFY**: Before any irreversible data-loss command, obtain explicit user consent.

## Mandatory procedure

1. Halt — do not execute the command.
2. Explain impact, why it seems necessary, and request explicit approval.
3. Wait for clear affirmative consent in the conversation.
4. Only then proceed.

Applies to: SQL DROP/TRUNCATE/broad DELETE; `gcloud`/`gsutil` production deletes; Spanner/BigQuery/Dataproc destruction; secret/KMS destruction; wiping `.ade` coordination state without approval.
