import type { ReactNode } from "react";

export type PathLease = {
  id: string;
  agent_id: string;
  path: string;
  mode: "observe" | "cooperative" | "strong" | "exclusive";
  created_at: string;
  expires_at: string;
  protected: boolean;
};

function normalizePath(path: string): string {
  return path.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

function pathsOverlap(a: string, b: string): boolean {
  const left = normalizePath(a);
  const right = normalizePath(b);
  return left === right || left.startsWith(`${right}/`) || right.startsWith(`${left}/`);
}

export function writableConflict(
  leases: PathLease[],
  agentId: string,
  ownedPaths: string[],
): PathLease | null {
  const mine = agentId.toLowerCase();
  for (const path of ownedPaths) {
    for (const lease of leases) {
      if (lease.mode === "observe") continue;
      if (!pathsOverlap(path, lease.path)) continue;
      if (lease.agent_id.toLowerCase() === mine) continue;
      if (lease.mode === "strong" || lease.mode === "exclusive" || lease.mode === "cooperative") {
        return lease;
      }
    }
  }
  return null;
}

export function myWritableLeases(leases: PathLease[], agentId: string): PathLease[] {
  const mine = agentId.toLowerCase();
  return leases.filter(
    (lease) => lease.agent_id.toLowerCase() === mine && lease.mode !== "observe",
  );
}

type AgentSessionStripProps = {
  agentId: string;
  mutating: boolean;
  leases: PathLease[];
  ownedPaths: string[];
  busy: boolean;
  /** G1 shell scope — Home Apply is shell-first, not PLAN-path-first. */
  shellScope?: "workspace" | "home";
  /** Rotate lease agent UUID (Apply ownership). Not a second chat agent. */
  onNewLease: () => void;
  /** Clear transcript for a fresh primary chat in this workspace. */
  onNewChat?: () => void;
  /** H4: refresh lease registry (wait). */
  onWaitRefresh?: () => void;
  /** H4: enable Isolate worktree for next Apply. */
  onEnableIsolate?: () => void;
  /** H4: switch autonomy to Suggest/Planner. */
  onSwitchSuggest?: () => void;
  isolateEnabled?: boolean;
  /** Standard Home: one-line status unless lease conflict (always expanded). */
  compact?: boolean;
};

/** Standard honesty strip: who we are + whether Apply can take write leases. */
export function AgentSessionStrip({
  agentId,
  mutating,
  leases,
  ownedPaths,
  busy,
  shellScope = "workspace",
  onNewLease,
  onNewChat,
  onWaitRefresh,
  onEnableIsolate,
  onSwitchSuggest,
  isolateEnabled = false,
  compact = false,
}: AgentSessionStripProps): ReactNode {
  const short = agentId.slice(0, 8);
  const conflict = mutating ? writableConflict(leases, agentId, ownedPaths) : null;
  const mine = myWritableLeases(leases, agentId);
  const covered =
    mutating &&
    ownedPaths.length > 0 &&
    ownedPaths.every((path) =>
      mine.some((lease) => pathsOverlap(path, lease.path)),
    );

  let statusLabel = "Suggest — no write lease needed";
  let statusClass = "text-slate-500";
  if (mutating) {
    if (conflict) {
      statusLabel = `Blocked — ${conflict.agent_id.slice(0, 8)} holds ${conflict.path}`;
      statusClass = "text-red-300";
    } else if (covered) {
      statusLabel = `Write lease ready · ${mine.length} path${mine.length === 1 ? "" : "s"}`;
      statusClass = "text-emerald-300";
    } else if (ownedPaths.length === 0 && shellScope === "home") {
      statusLabel = "Apply · Home shell + workspace writes (human dial)";
      statusClass = "text-emerald-300/90";
    } else if (ownedPaths.length === 0) {
      statusLabel = "Apply · workspace writes allowed (no PLAN path pin)";
      statusClass = "text-emerald-300/90";
    } else {
      statusLabel = "Will claim write lease on Go";
      statusClass = "text-amber-200/90";
    }
  }

  if (compact && !conflict) {
    return (
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 px-0.5 py-0.5">
        <div className={`min-w-0 truncate text-[10px] ${statusClass}`} title={agentId}>
          {statusLabel}
          <span className="ml-1.5 font-mono text-slate-600">{short}</span>
        </div>
        {onNewChat && (
          <button
            type="button"
            disabled={busy}
            onClick={onNewChat}
            title="Clear this workspace chat"
            className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold text-slate-500 hover:bg-white/5 hover:text-slate-300 disabled:opacity-40"
          >
            + Chat
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="flex shrink-0 flex-col gap-2 rounded-xl border border-white/8 bg-white/2 px-3 py-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2 text-[11px]">
            <span
              className="font-semibold text-slate-400"
              title="Primary worker lease id for this workspace"
            >
              Lease
            </span>
            <span className="font-mono text-slate-200" title={agentId}>
              {short}
            </span>
            <button
              type="button"
              disabled={busy}
              onClick={onNewLease}
              title="New lease id — for Apply ownership. Does not spawn a parallel agent."
              className="rounded px-1.5 py-0.5 text-[10px] font-semibold text-slate-500 hover:bg-white/5 hover:text-slate-300 disabled:opacity-40"
            >
              New lease
            </button>
          </div>
          <div className={`mt-0.5 text-[10px] ${statusClass}`}>{statusLabel}</div>
        </div>
        {onNewChat && (
          <button
            type="button"
            disabled={busy}
            onClick={onNewChat}
            title="Clear this workspace chat. Parallel agents will use separate session slots later."
            className="shrink-0 rounded-md border border-white/10 bg-white/4 px-2.5 py-1.5 text-[11px] font-semibold text-slate-200 hover:bg-white/8 disabled:opacity-40"
          >
            + Chat
          </button>
        )}
      </div>
      {conflict && (
        <div className="rounded-lg border border-red-400/25 bg-red-500/8 px-2.5 py-2">
          <div className="text-[10px] font-semibold uppercase tracking-wider text-red-200/80">
            Lease conflict
          </div>
          <p className="mt-1 text-[10px] leading-4 text-red-100/75">
            Another agent holds a write lease. Wait for expiry, Isolate into a worktree, rotate
            your lease id, or switch to Suggest.
          </p>
          <div className="mt-2 flex flex-wrap gap-1.5">
            {onWaitRefresh && (
              <button
                type="button"
                disabled={busy}
                onClick={onWaitRefresh}
                className="rounded-md border border-white/12 bg-white/5 px-2 py-1 text-[10px] font-semibold text-slate-200 hover:bg-white/10 disabled:opacity-40"
              >
                Wait · refresh
              </button>
            )}
            {onEnableIsolate && (
              <button
                type="button"
                disabled={busy || isolateEnabled}
                onClick={onEnableIsolate}
                className="rounded-md border border-amber-400/30 bg-amber-500/12 px-2 py-1 text-[10px] font-semibold text-amber-100 hover:bg-amber-500/20 disabled:opacity-40"
              >
                {isolateEnabled ? "Isolate on" : "Isolate"}
              </button>
            )}
            <button
              type="button"
              disabled={busy}
              onClick={onNewLease}
              className="rounded-md border border-white/12 bg-white/5 px-2 py-1 text-[10px] font-semibold text-slate-200 hover:bg-white/10 disabled:opacity-40"
            >
              Rotate lease
            </button>
            {onSwitchSuggest && (
              <button
                type="button"
                disabled={busy}
                onClick={onSwitchSuggest}
                className="rounded-md border border-blue-400/25 bg-blue-500/12 px-2 py-1 text-[10px] font-semibold text-blue-100 hover:bg-blue-500/20 disabled:opacity-40"
              >
                Suggest
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export function agentStorageKey(workspaceRoot: string): string {
  return `ade_lease_agent_id:${workspaceRoot}`;
}

export function sessionStorageKey(workspaceRoot: string): string {
  return `ade_chat_session_id:${workspaceRoot}`;
}

export function readOrCreateAgentId(workspaceRoot: string): string {
  if (typeof window === "undefined" || !workspaceRoot) {
    return cryptoRandom();
  }
  const key = agentStorageKey(workspaceRoot);
  const existing = window.localStorage.getItem(key)?.trim();
  if (existing && /^[0-9a-f-]{36}$/i.test(existing)) return existing;
  const next = cryptoRandom();
  window.localStorage.setItem(key, next);
  return next;
}

export function rotateAgentId(workspaceRoot: string): string {
  const next = cryptoRandom();
  if (typeof window !== "undefined" && workspaceRoot) {
    window.localStorage.setItem(agentStorageKey(workspaceRoot), next);
  }
  return next;
}

/** Fresh primary chat session id (orchestrator can later add worker session ids). */
export function rotateSessionId(workspaceRoot: string): string {
  const next = cryptoRandom();
  if (typeof window !== "undefined" && workspaceRoot) {
    window.localStorage.setItem(sessionStorageKey(workspaceRoot), next);
  }
  return next;
}

export function readOrCreateSessionId(workspaceRoot: string): string {
  if (typeof window === "undefined" || !workspaceRoot) {
    return cryptoRandom();
  }
  const key = sessionStorageKey(workspaceRoot);
  const existing = window.localStorage.getItem(key)?.trim();
  if (existing && /^[0-9a-f-]{36}$/i.test(existing)) return existing;
  const next = cryptoRandom();
  window.localStorage.setItem(key, next);
  return next;
}

function cryptoRandom(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return "00000000-0000-4000-8000-000000000000";
}

export function honestLeaseError(raw: string): string {
  const text = raw.trim();
  if (/lease conflict|incompatible|already holds|not covered by an active writable lease/i.test(text)) {
    return text.replace(/^Error:\s*/i, "");
  }
  if (/protected path/i.test(text)) {
    return text;
  }
  return text;
}
