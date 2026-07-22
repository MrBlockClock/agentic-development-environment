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
  onNewAgent: () => void;
};

/** Standard honesty strip: who we are + whether Apply can take write leases. */
export function AgentSessionStrip({
  agentId,
  mutating,
  leases,
  ownedPaths,
  busy,
  shellScope = "workspace",
  onNewAgent,
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

  return (
    <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 rounded-xl border border-white/8 bg-white/2 px-3 py-2">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2 text-[11px]">
          <span className="font-semibold text-slate-400">Agent</span>
          <span className="font-mono text-slate-200" title={agentId}>
            {short}
          </span>
          <button
            type="button"
            disabled={busy}
            onClick={onNewAgent}
            className="rounded px-1.5 py-0.5 text-[10px] font-semibold text-slate-500 hover:bg-white/5 hover:text-slate-300 disabled:opacity-40"
          >
            New
          </button>
        </div>
        <div className={`mt-0.5 text-[10px] ${statusClass}`}>{statusLabel}</div>
      </div>
    </div>
  );
}

export function agentStorageKey(workspaceRoot: string): string {
  return `ade_lease_agent_id:${workspaceRoot}`;
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
