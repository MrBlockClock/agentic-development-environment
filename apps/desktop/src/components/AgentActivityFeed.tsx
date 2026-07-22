import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { AssistantMarkdown } from "./AssistantMarkdown";
import type { TurnFailureAction, TurnFailureAdvice } from "./turnFailure";

export type { TurnFailureAction, TurnFailureAdvice };

export type AgentFeedEvent =
  | { type: "user_message"; text: string }
  | { type: "started"; session_id: string; provider: string; model: string }
  | { type: "text_delta"; text: string }
  | {
      type: "tool_call";
      server: string;
      tool: string;
      arguments: unknown;
      effect?: string;
    }
  | { type: "tool_result"; server: string; tool: string; is_error: boolean; text: string }
  | {
      type: "usage";
      input_tokens: number;
      output_tokens: number;
      cost_micros: number;
    }
  | {
      type: "spend_warning";
      scope: string;
      period_key: string;
      projected_micros: number;
      soft_cap_micros: number;
    }
  | {
      type: "budget_exhausted";
      kind: string;
      limit: number;
      used: number;
      detail: string;
    }
  | { type: "verify_complete"; gate: string; passed: boolean; summary: string }
  | {
      type: "completed";
      result: {
        provider: string;
        model: string;
        tool_calls: number;
        cost_micros: number;
        usage: { input_tokens: number; output_tokens: number };
      };
    }
  | { type: "failed"; error: string }
  | { type: "cancelled"; reason: string };

type Step =
  | {
      kind: "tool";
      key: string;
      server: string;
      tool: string;
      effect?: string;
      status: "running" | "ok" | "error";
      detail?: string;
      pathHint?: string;
    }
  | { kind: "verify"; key: string; gate: string; passed: boolean; summary: string }
  | { kind: "note"; key: string; tone: "info" | "warn" | "error"; text: string }
  | { kind: "user"; key: string; text: string };

function pathFromArgs(args: unknown): string | undefined {
  if (!args || typeof args !== "object") return undefined;
  const record = args as Record<string, unknown>;
  for (const key of ["path", "file", "filename", "target", "cwd"]) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return undefined;
}

function summarizeDetail(detail: string, pathHint?: string): string {
  if (pathHint) return pathHint;
  const firstLine = detail.split(/\r?\n/).find((line) => line.trim())?.trim();
  if (!firstLine) return "output";
  return firstLine.length > 80 ? `${firstLine.slice(0, 78)}…` : firstLine;
}

/** Strip model thinking tags, special tokens, and zero-width junk from assistant text. */
export function sanitizeAssistantText(text: string): string {
  let out = text;
  out = out.replace(/<think\b[^>]*>[\s\S]*?<\/think>/gi, "");
  out = out.replace(/<reasoning\b[^>]*>[\s\S]*?<\/reasoning>/gi, "");
  // Hide incomplete blocks while streaming
  out = out.replace(/<think\b[^>]*>[\s\S]*$/gi, "");
  out = out.replace(/<reasoning\b[^>]*>[\s\S]*$/gi, "");
  out = out.replace(/<\/?think\b[^>]*>/gi, "");
  out = out.replace(/<\/?reasoning\b[^>]*>/gi, "");
  out = out.replace(/```(?:thinking|reasoning)[\s\S]*?```/gi, "");
  out = out.replace(/\[\s*(?:thinking|reasoning)\s*\][\s\S]*?\[\s*\/\s*(?:thinking|reasoning)\s*\]/gi, "");
  out = out.replace(/<\|[^|]{0,80}\|>/g, "");
  out = out.replace(/[\u200B-\u200D\uFEFF\uFFFD]/g, "");
  // Soften leftover orphan fences that are only punctuation noise
  out = out.replace(/^[ \t]*[`|]{3,}[ \t]*$/gm, "");
  out = out.replace(/[ \t]+\n/g, "\n");
  out = out.replace(/\n{4,}/g, "\n\n\n");
  return out.replace(/^\s+/, "");
}

function mentionsApply(text: string): boolean {
  return /\bapply\b/i.test(text);
}

function buildSteps(events: AgentFeedEvent[]): Step[] {
  const steps: Step[] = [];
  const open = new Map<string, number>();

  events.forEach((event, index) => {
    if (event.type === "user_message") {
      steps.push({ kind: "user", key: `user-${index}`, text: event.text });
      return;
    }
    if (event.type === "started") {
      steps.push({
        kind: "note",
        key: `started-${index}`,
        tone: "info",
        text: `Started · ${event.provider} / ${event.model}`,
      });
      return;
    }
    if (event.type === "tool_call") {
      open.set(`${event.server}/${event.tool}`, steps.length);
      steps.push({
        kind: "tool",
        key: `${event.server}/${event.tool}-${index}`,
        server: event.server,
        tool: event.tool,
        effect: event.effect,
        status: "running",
        pathHint: pathFromArgs(event.arguments),
      });
      return;
    }
    if (event.type === "tool_result") {
      const key = `${event.server}/${event.tool}`;
      const at = open.get(key);
      if (at != null) {
        const step = steps[at];
        if (step?.kind === "tool") {
          step.status = event.is_error ? "error" : "ok";
          step.detail = event.text;
        }
        open.delete(key);
      } else {
        steps.push({
          kind: "tool",
          key: `${key}-result-${index}`,
          server: event.server,
          tool: event.tool,
          status: event.is_error ? "error" : "ok",
          detail: event.text,
        });
      }
      return;
    }
    if (event.type === "verify_complete") {
      steps.push({
        kind: "verify",
        key: `verify-${index}`,
        gate: event.gate,
        passed: event.passed,
        summary: event.summary,
      });
      return;
    }
    if (event.type === "spend_warning") {
      steps.push({
        kind: "note",
        key: `spend-${index}`,
        tone: "warn",
        text: `Spend warning (${event.scope}): nearing soft cap`,
      });
      return;
    }
    if (event.type === "budget_exhausted") {
      const label =
        event.kind === "tool_rounds"
          ? `Budget exhausted · ${event.used}/${event.limit} tool rounds`
          : event.kind === "output_tokens"
            ? `Budget exhausted · ${event.used}/${event.limit} output tokens`
            : `Budget exhausted · ${event.detail}`;
      steps.push({
        kind: "note",
        key: `budget-${index}`,
        tone: "warn",
        text: label,
      });
      return;
    }
    if (event.type === "failed") {
      const budget = /budget exhausted/i.test(event.error);
      steps.push({
        kind: "note",
        key: `failed-${index}`,
        tone: budget ? "warn" : "error",
        text: event.error,
      });
      return;
    }
    if (event.type === "cancelled") {
      steps.push({
        kind: "note",
        key: `cancelled-${index}`,
        tone: "warn",
        text: event.reason || "Cancelled",
      });
    }
  });

  return steps;
}

function StatusPill({
  busy,
  failed,
  budget,
  completed,
}: {
  busy: boolean;
  failed: boolean;
  budget: boolean;
  completed: boolean;
}) {
  const label = busy
    ? "Running"
    : budget
      ? "Budget"
      : failed
        ? "Failed"
        : completed
          ? "Done"
          : "Idle";
  const klass = busy
    ? "bg-blue-500/20 text-blue-100"
    : budget
      ? "bg-amber-500/20 text-amber-100"
      : failed
        ? "bg-red-500/20 text-red-200"
        : completed
          ? "bg-emerald-500/20 text-emerald-100"
          : "bg-white/8 text-slate-400";
  return (
    <span className={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${klass}`}>
      {busy && <span className="mr-1 inline-block size-1.5 animate-pulse rounded-full bg-blue-300" />}
      {label}
    </span>
  );
}

function effectLabel(effect?: string, simple?: boolean): string {
  if (!effect) return simple ? "Working" : "tool";
  if (!simple) return effect;
  if (effect === "ReadOnly") return "Looking up";
  if (effect === "WorkspaceWrite") return "Writing";
  if (effect === "ProcessExecution") return "Shell";
  return "Working";
}

function activitySummary(steps: Step[]): string {
  const tools = steps.filter((s) => s.kind === "tool");
  const errors = tools.filter((s) => s.status === "error").length;
  const running = tools.some((s) => s.status === "running");
  const parts: string[] = [];
  if (tools.length > 0) {
    const shell = tools.filter((s) => s.effect === "ProcessExecution").length;
    const looking = tools.filter((s) => s.effect === "ReadOnly").length;
    const writing = tools.filter((s) => s.effect === "WorkspaceWrite").length;
    if (shell) parts.push(`${shell} shell`);
    if (looking) parts.push(`${looking} lookup`);
    if (writing) parts.push(`${writing} write`);
    const other = tools.length - shell - looking - writing;
    if (other > 0) parts.push(`${other} step${other === 1 ? "" : "s"}`);
    if (parts.length === 0) parts.push(`${tools.length} step${tools.length === 1 ? "" : "s"}`);
  }
  const status = running ? "Running" : errors > 0 ? "Failed" : "Done";
  return parts.length > 0 ? `${parts.join(" · ")} · ${status}` : status;
}

function ToolStepRow({
  step,
  simpleMode,
}: {
  step: Extract<Step, { kind: "tool" }>;
  simpleMode?: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const hasDetail = Boolean(step.detail?.trim());
  const summary = hasDetail
    ? summarizeDetail(step.detail!, step.pathHint)
    : step.pathHint;

  return (
    <div>
      <div className="flex flex-wrap items-center gap-2 text-[11px]">
        <span
          className={
            step.status === "running"
              ? "text-amber-200"
              : step.status === "error"
                ? "text-red-300"
                : "text-emerald-300"
          }
        >
          {step.status === "running"
            ? `${effectLabel(step.effect, simpleMode)}…`
            : step.status === "error"
              ? "Failed"
              : "Done"}
        </span>
        {simpleMode ? (
          <span className="text-slate-400">{effectLabel(step.effect, true)}</span>
        ) : (
          <span className="font-mono text-slate-400">
            {step.server}/{step.tool}
          </span>
        )}
        {summary && (
          <span className="truncate font-mono text-[10px] text-slate-500">{summary}</span>
        )}
        {hasDetail && (
          <button
            type="button"
            className="text-[10px] font-semibold text-blue-300/90 hover:text-blue-200"
            onClick={() => setExpanded((value) => !value)}
          >
            {expanded ? "Hide" : "Show"}
          </button>
        )}
      </div>
      {expanded && hasDetail && (
        <pre className="mt-1 whitespace-pre-wrap wrap-break-word font-mono text-[10px] leading-4 text-slate-400">
          {step.detail}
        </pre>
      )}
    </div>
  );
}

function ActivityBlock({
  steps,
  open,
  simpleMode,
  label,
}: {
  steps: Step[];
  open: boolean;
  simpleMode?: boolean;
  label: string;
}) {
  // null = follow `open` (expanded while live, collapsed when reply lands)
  const [manual, setManual] = useState<boolean | null>(null);
  useEffect(() => {
    if (open) setManual(null);
  }, [open]);
  const show = manual ?? open;
  const summary = activitySummary(steps);

  return (
    <div>
      <button
        type="button"
        className="mb-1 flex w-full items-center gap-2 text-left"
        onClick={() => setManual((prev) => !(prev ?? open))}
        aria-expanded={show}
      >
        <span className="text-[10px] font-semibold uppercase tracking-wider text-slate-600">
          {label}
        </span>
        {!show && (
          <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] font-medium text-slate-500">
            {summary}
          </span>
        )}
        <span className="ml-auto text-[10px] font-semibold text-slate-600">
          {show ? "Hide" : "Show"}
        </span>
      </button>
      {show && (
        <ol className="space-y-1">
          {steps.map((step, index) => (
            <li key={step.key} className="flex gap-2 py-1">
              <span className="mt-0.5 w-4 shrink-0 font-mono text-[10px] text-slate-600">
                {String(index + 1).padStart(2, "0")}
              </span>
              <div className="min-w-0 flex-1">
                {step.kind === "tool" && (
                  <ToolStepRow step={step} simpleMode={simpleMode} />
                )}
                {step.kind === "verify" && (
                  <div
                    className={`text-[11px] ${
                      step.passed ? "text-emerald-200" : "text-red-200"
                    }`}
                  >
                    Verify {step.gate}: {step.summary}
                  </div>
                )}
                {step.kind === "note" && (
                  <div
                    className={`text-[11px] leading-5 ${
                      step.tone === "error"
                        ? "text-red-200"
                        : step.tone === "warn"
                          ? "text-amber-100"
                          : "text-slate-300"
                    }`}
                  >
                    {step.text}
                  </div>
                )}
              </div>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}

export function AgentActivityFeed({
  events,
  busy,
  simpleMode = false,
  autonomySuggest = false,
  autonomyLabel,
  scopeLabel,
  maxSteps,
  onSwitchToApply,
  onSwitchToHomeScope,
  onPrefillPrompt,
  onSelectOption,
  failureAdvice = null,
  failureBusy = false,
  onFailureAction,
}: {
  events: AgentFeedEvent[];
  busy: boolean;
  simpleMode?: boolean;
  autonomySuggest?: boolean;
  /** Suggest / Apply / … shown in the status row (G1 DNA honesty). */
  autonomyLabel?: string;
  /** Workspace / Home shell scope (G1). */
  scopeLabel?: string;
  /** Effort tool-round cap for live used/remaining. */
  maxSteps?: number;
  onSwitchToApply?: () => void;
  /** Flip shell scope to Home/Desktop for organize-style goals. */
  onSwitchToHomeScope?: () => void;
  onPrefillPrompt?: (text: string) => void;
  /** Clicking a suggested option runs a turn with that text. */
  onSelectOption?: (text: string) => void;
  failureAdvice?: TurnFailureAdvice | null;
  failureBusy?: boolean;
  onFailureAction?: (action: TurnFailureAction) => void;
}): ReactNode {
  const started = [...events]
    .reverse()
    .find(
      (event): event is Extract<AgentFeedEvent, { type: "started" }> =>
        event.type === "started",
    );
  const completed = [...events].reverse().find(
    (event): event is Extract<AgentFeedEvent, { type: "completed" }> =>
      event.type === "completed",
  );
  const failed = [...events].reverse().find(
    (event): event is Extract<AgentFeedEvent, { type: "failed" }> =>
      event.type === "failed",
  );
  const budgetExhausted = [...events].reverse().find(
    (event): event is Extract<AgentFeedEvent, { type: "budget_exhausted" }> =>
      event.type === "budget_exhausted",
  );
  const budgetStop =
    Boolean(budgetExhausted) ||
    Boolean(failed && /budget exhausted/i.test(failed.error));
  const usage = [...events].reverse().find(
    (event): event is Extract<AgentFeedEvent, { type: "usage" }> =>
      event.type === "usage",
  );
  const steps = buildSteps(events);
  const toolCount = steps.filter((step) => step.kind === "tool").length;
  const roundCap =
    typeof maxSteps === "number" && Number.isFinite(maxSteps) && maxSteps > 0
      ? Math.floor(maxSteps)
      : null;
  const roundsRemaining =
    roundCap != null ? Math.max(0, roundCap - toolCount) : null;

  const turns: { user?: string; events: AgentFeedEvent[] }[] = [];
  for (const event of events) {
    if (event.type === "user_message") {
      turns.push({ user: event.text, events: [] });
      continue;
    }
    if (turns.length === 0) turns.push({ events: [] });
    turns[turns.length - 1]!.events.push(event);
  }

  return (
    <div className="space-y-4">
      {(busy || started || failed || completed || autonomyLabel || scopeLabel) && (
        <div className="flex flex-wrap items-center gap-2 text-[10px] text-slate-500">
          <StatusPill
            busy={busy}
            failed={Boolean(failed) && !budgetStop}
            budget={budgetStop}
            completed={Boolean(completed)}
          />
          {autonomyLabel && (
            <span className="rounded bg-white/5 px-1.5 py-0.5 font-semibold text-slate-400">
              {autonomyLabel}
            </span>
          )}
          {scopeLabel && (
            <span
              className="rounded bg-white/5 px-1.5 py-0.5 font-semibold text-slate-400"
              title="Shell working-directory scope"
            >
              {scopeLabel}
            </span>
          )}
          <span className="font-mono text-slate-400">
            {started
              ? `${started.provider} · ${started.model}`
              : busy
                ? "Connecting…"
                : "Idle"}
          </span>
          {roundCap != null ? (
            <span
              title="Effort tool-round budget for this turn"
              className={
                busy && roundsRemaining != null && roundsRemaining <= 2
                  ? "font-semibold text-amber-200/90"
                  : undefined
              }
            >
              {toolCount}/{roundCap} rounds
              {busy && roundsRemaining != null ? ` · ${roundsRemaining} left` : ""}
            </span>
          ) : (
            toolCount > 0 && (
              <span>
                {toolCount} tool{toolCount === 1 ? "" : "s"}
              </span>
            )
          )}
          {usage && (
            <span>
              {usage.input_tokens} in / {usage.output_tokens} out
            </span>
          )}
        </div>
      )}

      {turns.length === 0 && !busy && (
        <p className="text-[12px] leading-5 text-slate-600">
          Ask ADE something — your question and the reply will show up here.
        </p>
      )}

      {turns.map((turn, turnIndex) => {
        const turnSteps = buildSteps(turn.events).filter((step) => step.kind !== "user");
        const turnText = sanitizeAssistantText(
          turn.events
            .filter(
              (event): event is Extract<AgentFeedEvent, { type: "text_delta" }> =>
                event.type === "text_delta",
            )
            .map((event) => event.text)
            .join(""),
        );
        const turnFailed = [...turn.events]
          .reverse()
          .find(
            (event): event is Extract<AgentFeedEvent, { type: "failed" }> =>
              event.type === "failed",
          );
        const turnCancelled = [...turn.events]
          .reverse()
          .find(
            (event): event is Extract<AgentFeedEvent, { type: "cancelled" }> =>
              event.type === "cancelled",
          );
        const isLatest = turnIndex === turns.length - 1;
        const activityOpen = (isLatest && busy) || Boolean(turnFailed);
        const showApplyCta =
          autonomySuggest &&
          Boolean(onSwitchToApply) &&
          Boolean(turnText) &&
          !(isLatest && busy) &&
          !turnFailed &&
          mentionsApply(turnText);

        return (
          <div key={`turn-${turnIndex}`} className="space-y-2.5">
            {turn.user && (
              <div className="rounded-lg bg-blue-500/10 px-3 py-2 text-[13px] leading-5 text-slate-100">
                <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-blue-200/80">
                  You
                </div>
                {turn.user}
              </div>
            )}

            {turnSteps.length > 0 && (
              <ActivityBlock
                steps={turnSteps}
                open={activityOpen}
                simpleMode={simpleMode}
                label={isLatest && busy ? "Live activity" : "Activity"}
              />
            )}

            {(turnText || (isLatest && busy)) && (
              <div className="text-[13px] leading-6 text-slate-200">
                {turnText ? (
                  <AssistantMarkdown
                    text={turnText}
                    onPickOption={
                      busy || turnFailed || !(onSelectOption || onPrefillPrompt)
                        ? undefined
                        : (text) => (onSelectOption ?? onPrefillPrompt)?.(text)
                    }
                  />
                ) : (
                  <span className="text-slate-500">Thinking…</span>
                )}
                {isLatest && busy && (
                  <span className="ml-1 inline-block size-1.5 animate-pulse bg-blue-300 align-middle" />
                )}
              </div>
            )}

            {turnFailed && (
              <div className="rounded-lg border border-red-400/30 bg-red-500/10 px-3 py-2 text-[12px] leading-5 text-red-100">
                <div className="mb-0.5 text-[10px] font-semibold uppercase tracking-wider text-red-200/90">
                  {isLatest && failureAdvice ? failureAdvice.title : "Turn failed"}
                </div>
                {isLatest && failureAdvice && (
                  <p className="mb-1.5 text-[11px] leading-5 text-red-100/85">
                    {failureAdvice.summary}
                  </p>
                )}
                <p className="font-mono text-[11px] leading-5 text-red-100/75 wrap-break-word">
                  {turnFailed.error}
                </p>
                {isLatest && failureAdvice && onFailureAction && !busy && (
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {failureAdvice.autoFix && (
                      <button
                        type="button"
                        disabled={failureBusy}
                        onClick={() => onFailureAction(failureAdvice.autoFix!)}
                        className="rounded-md border border-amber-400/35 bg-amber-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-amber-100 hover:bg-amber-500/25 disabled:opacity-40"
                      >
                        Fix &amp; retry
                      </button>
                    )}
                    {failureAdvice.actions
                      .filter((action) => {
                        if (!failureAdvice.autoFix) return true;
                        return !(
                          action.id === failureAdvice.autoFix.id &&
                          JSON.stringify(action) ===
                            JSON.stringify(failureAdvice.autoFix)
                        );
                      })
                      .map((action) => (
                        <button
                          key={`${action.id}-${"model" in action ? action.model : ""}${"providerId" in action ? action.providerId : ""}`}
                          type="button"
                          disabled={failureBusy}
                          onClick={() => onFailureAction(action)}
                          className="rounded-md border border-white/12 bg-white/5 px-2.5 py-1.5 text-[11px] font-semibold text-slate-200 hover:bg-white/10 disabled:opacity-40"
                        >
                          {action.label}
                        </button>
                      ))}
                  </div>
                )}
              </div>
            )}
            {turnCancelled && !turnFailed && (
              <div className="rounded-lg border border-amber-400/25 bg-amber-500/10 px-3 py-2 text-[12px] leading-5 text-amber-100">
                <div className="mb-0.5 text-[10px] font-semibold uppercase tracking-wider text-amber-200/90">
                  Cancelled
                </div>
                {turnCancelled.reason || "Cancelled"}
              </div>
            )}

            {showApplyCta && (
              <div className="flex flex-wrap items-center gap-2 pt-0.5">
                <button
                  type="button"
                  onClick={onSwitchToApply}
                  className="rounded-md border border-blue-400/35 bg-blue-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/25"
                >
                  Switch to Apply
                </button>
                {onPrefillPrompt && (
                  <button
                    type="button"
                    onClick={() => {
                      onSwitchToApply?.();
                      onSwitchToHomeScope?.();
                      onPrefillPrompt("Organize my Desktop as you recommended");
                    }}
                    className="rounded-md border border-white/10 bg-white/5 px-2.5 py-1.5 text-[11px] font-semibold text-slate-300 hover:bg-white/8 hover:text-slate-100"
                  >
                    Ask to organize
                  </button>
                )}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
