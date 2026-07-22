/** Classify agent turn failures and propose fix/retry actions. */

import {
  DEFAULT_MODEL,
  DEFAULT_PROVIDER,
  PROVIDER_PRESETS,
  presetById,
  type ProviderPreset,
} from "../providers";

export type TurnFailureKind =
  | "provider_5xx"
  | "provider_timeout"
  | "provider_unreachable"
  | "provider_auth"
  | "provider_rate_limit"
  | "tool_round_limit"
  | "token_budget"
  | "spend_cap"
  | "lease_conflict"
  | "unknown";

export type EffortTier = "low" | "medium" | "high";

export type TurnFailureAction =
  | { id: "retry"; label: string }
  | { id: "retry_alt_model"; label: string; model: string }
  | {
      id: "switch_provider";
      label: string;
      providerId: string;
      baseUrl: string;
      model: string;
    }
  | {
      id: "raise_steps";
      label: string;
      effort: EffortTier;
      maxSteps: number;
    }
  | {
      id: "continue_handoff";
      label: string;
      effort: EffortTier;
      maxSteps: number;
    }
  | { id: "open_keys"; label: string }
  | { id: "fix_base_url"; label: string; baseUrl: string };

export type TurnFailureAdvice = {
  kind: TurnFailureKind;
  title: string;
  summary: string;
  /** Safe to auto-apply once without asking. */
  autoFix: TurnFailureAction | null;
  actions: TurnFailureAction[];
};

export type TurnFailureContext = {
  error: string;
  providerId: string;
  model: string;
  baseUrl: string;
  effort?: EffortTier;
  /** Prefer Continue handoff over same-prompt raise_steps when a capsule exists. */
  handoffAvailable?: boolean;
};

function alternateModel(preset: ProviderPreset | undefined, current: string): string | null {
  if (!preset || preset.models.length === 0) return null;
  const other = preset.models.find(
    (item) => item.toLowerCase() !== current.trim().toLowerCase(),
  );
  return other ?? null;
}

function fallbackProvider(currentId: string): ProviderPreset | null {
  const preferred =
    currentId === "opencode"
      ? presetById("freellm")
      : presetById("opencode") ?? presetById(DEFAULT_PROVIDER);
  if (!preferred || preferred.id === currentId) {
    return PROVIDER_PRESETS.find((item) => item.id !== currentId) ?? null;
  }
  return preferred;
}

function nextEffortRaise(effort: EffortTier | undefined): {
  effort: EffortTier;
  maxSteps: number;
} | null {
  if (effort === "high") return null;
  if (effort === "medium") return { effort: "high", maxSteps: 32 };
  return { effort: "medium", maxSteps: 24 };
}

function parseRoundLimit(error: string): number | null {
  const match = error.match(/(\d+)\s*-?\s*round/i);
  if (!match) return null;
  const n = Number(match[1]);
  return Number.isFinite(n) && n > 0 ? n : null;
}

export function evaluateTurnFailure(ctx: TurnFailureContext): TurnFailureAdvice {
  const error = ctx.error.trim();
  const lower = error.toLowerCase();
  const preset = presetById(ctx.providerId);
  const altModel = alternateModel(preset, ctx.model);
  const other = fallbackProvider(ctx.providerId);

  const retry: TurnFailureAction = { id: "retry", label: "Retry same" };
  const openKeys: TurnFailureAction = { id: "open_keys", label: "Open Keys" };
  const switchModel: TurnFailureAction | null = altModel
    ? {
        id: "retry_alt_model",
        label: `Retry with ${altModel}`,
        model: altModel,
      }
    : null;
  const switchProvider: TurnFailureAction | null = other
    ? {
        id: "switch_provider",
        label: `Switch to ${other.label}`,
        providerId: other.id,
        baseUrl: other.baseUrl,
        model: other.models[0] ?? DEFAULT_MODEL,
      }
    : null;

  if (
    lower.includes("tool-call limit") ||
    lower.includes("tool call limit") ||
    (lower.includes("budget exhausted") && lower.includes("round")) ||
    (lower.includes("exceeded the") && lower.includes("round"))
  ) {
    const hit = parseRoundLimit(error) ?? 8;
    const raised = nextEffortRaise(ctx.effort);
    const raiseAction: TurnFailureAction | null = raised
      ? {
          id: "raise_steps",
          label: `Raise effort → ${raised.effort} (${raised.maxSteps} steps)`,
          effort: raised.effort,
          maxSteps: Math.max(raised.maxSteps, hit * 2),
        }
      : null;
    const continueAction: TurnFailureAction = raised
      ? {
          id: "continue_handoff",
          label: `Continue handoff → ${raised.effort}`,
          effort: raised.effort,
          maxSteps: Math.max(raised.maxSteps, hit * 2),
        }
      : {
          id: "continue_handoff",
          label: "Continue from handoff",
          effort: "high",
          maxSteps: 32,
        };
    const preferContinue = Boolean(ctx.handoffAvailable);
    return {
      kind: "tool_round_limit",
      title: "Tool-round budget exhausted",
      summary: preferContinue
        ? `ADE stopped after ${hit} tool rounds. Continuing from the handoff with raised Effort (host runs next_safe_command first).`
        : raiseAction
          ? `ADE stopped after ${hit} tool rounds (Effort was too low for this Continuity/dogfood turn). Raising the step budget and retrying.`
          : `Already on High effort (${hit}+ rounds). Narrow the goal, or continue from the last handoff with a shorter next_safe_command.`,
      autoFix: preferContinue ? continueAction : raiseAction,
      actions: preferContinue
        ? ([continueAction, raiseAction, retry].filter(Boolean) as TurnFailureAction[])
        : ([raiseAction, continueAction, retry].filter(Boolean) as TurnFailureAction[]),
    };
  }

  if (
    lower.includes("token output budget") ||
    lower.includes("token budget") ||
    (lower.includes("budget exhausted") && lower.includes("token")) ||
    (lower.includes("exceeded the") && lower.includes("token"))
  ) {
    const raised = nextEffortRaise(ctx.effort);
    const raiseAction: TurnFailureAction | null = raised
      ? {
          id: "raise_steps",
          label: `Raise effort → ${raised.effort}`,
          effort: raised.effort,
          maxSteps: raised.maxSteps,
        }
      : null;
    const continueAction: TurnFailureAction = raised
      ? {
          id: "continue_handoff",
          label: `Continue handoff → ${raised.effort}`,
          effort: raised.effort,
          maxSteps: raised.maxSteps,
        }
      : {
          id: "continue_handoff",
          label: "Continue from handoff",
          effort: "high",
          maxSteps: 32,
        };
    const preferContinue = Boolean(ctx.handoffAvailable);
    return {
      kind: "token_budget",
      title: "Token budget hit",
      summary: preferContinue
        ? "Output token budget hit. Continuing from the handoff with raised Effort."
        : raiseAction
          ? "The turn hit the output token budget. Raising Effort increases the allowance."
          : "Already on High effort. Split the task or continue from handoff.",
      autoFix: preferContinue ? continueAction : raiseAction,
      actions: preferContinue
        ? ([continueAction, raiseAction, retry].filter(Boolean) as TurnFailureAction[])
        : ([raiseAction, continueAction, retry].filter(Boolean) as TurnFailureAction[]),
    };
  }

  if (
    /http\s*5\d\d/.test(lower) ||
    lower.includes("internal server error") ||
    lower.includes("bad gateway") ||
    lower.includes("service unavailable")
  ) {
    const actions = [switchModel, retry, switchProvider, openKeys].filter(
      Boolean,
    ) as TurnFailureAction[];
    return {
      kind: "provider_5xx",
      title: "Provider server error",
      summary: switchModel
        ? `The gateway returned a 5xx. Switching to ${altModel} usually clears transient OpenCode/FreeLLM outages.`
        : "The gateway returned a 5xx. Retry, or switch provider if it keeps failing.",
      autoFix: switchModel,
      actions,
    };
  }

  if (
    lower.includes("401") ||
    lower.includes("403") ||
    lower.includes("unauthorized") ||
    lower.includes("invalid api key") ||
    lower.includes("authentication")
  ) {
    return {
      kind: "provider_auth",
      title: "Provider auth failed",
      summary: "Key missing, wrong, or not accepted by this base URL. Fix Keys, then retry.",
      autoFix: null,
      actions: [openKeys, retry, switchProvider].filter(Boolean) as TurnFailureAction[],
    };
  }

  if (
    lower.includes("429") ||
    lower.includes("rate limit") ||
    lower.includes("too many requests")
  ) {
    return {
      kind: "provider_rate_limit",
      title: "Rate limited",
      summary: "Provider asked us to slow down. Wait briefly, retry, or switch model/provider.",
      autoFix: null,
      actions: [retry, switchModel, switchProvider].filter(Boolean) as TurnFailureAction[],
    };
  }

  if (
    lower.includes("timed out") ||
    lower.includes("timeout") ||
    lower.includes("deadline")
  ) {
    return {
      kind: "provider_timeout",
      title: "Provider timed out",
      summary: "Transient stall or overloaded model. Retry once; switch model if it repeats.",
      autoFix: retry,
      actions: [retry, switchModel, switchProvider].filter(Boolean) as TurnFailureAction[],
    };
  }

  if (
    lower.includes("cannot reach") ||
    lower.includes("connection refused") ||
    lower.includes("error sending request") ||
    lower.includes("dns error") ||
    lower.includes("failed to connect")
  ) {
    const local =
      ctx.baseUrl.includes("127.0.0.1") ||
      ctx.baseUrl.includes("localhost") ||
      lower.includes("local gateway");
    return {
      kind: "provider_unreachable",
      title: local ? "Local gateway unreachable" : "Provider unreachable",
      summary: local
        ? "FreeLLMAPI / local gateway is not answering. Start it, or switch to OpenCode Zen."
        : "Network or DNS failed reaching the provider. Retry or switch provider.",
      autoFix: local ? switchProvider : retry,
      actions: [switchProvider, retry, openKeys].filter(Boolean) as TurnFailureAction[],
    };
  }

  if (lower.includes("spend") && (lower.includes("cap") || lower.includes("budget"))) {
    return {
      kind: "spend_cap",
      title: "Spend cap hit",
      summary: "Session or daily spend reserve blocked the turn. Raise caps in Settings, or wait for the period to roll.",
      autoFix: null,
      actions: [retry],
    };
  }

  if (lower.includes("lease") && lower.includes("conflict")) {
    return {
      kind: "lease_conflict",
      title: "Lease conflict",
      summary: "Another agent holds a write lease on an owned path. Switch to Suggest, or wait / release the lease.",
      autoFix: null,
      actions: [retry],
    };
  }

  return {
    kind: "unknown",
    title: "Turn failed",
    summary: "Could not classify this error. Retry, or switch model/provider if it persists.",
    autoFix: null,
    actions: [retry, switchModel, switchProvider, openKeys].filter(
      Boolean,
    ) as TurnFailureAction[],
  };
}

export function failureFingerprint(ctx: TurnFailureContext): string {
  return `${ctx.providerId}|${ctx.model}|${ctx.effort ?? "low"}|${ctx.error.trim()}`;
}
