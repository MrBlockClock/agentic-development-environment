import { useMemo, useState } from "react";
import { invoke, isTauri } from "../ipc";
import type { ShellTab } from "../shellTabs";
import type { WorkspaceEntry } from "./WorkspacesView";

export type WorkspaceListSnapshot = {
  current: string;
  entries: WorkspaceEntry[];
  ade_source_root: string | null;
};

type SidebarRailListsProps = {
  workspaces: WorkspaceListSnapshot | null;
  workspaceBusy?: boolean;
  sessions: ShellTab[];
  activeSessionId: string | null;
  sessionsActive: boolean;
  onOpenWorkspace: (path: string) => void;
  onManageWorkspaces: () => void;
  onNewWorkspace: () => void;
  onFocusSession: (id: string) => void;
  onNewSession: () => void;
  onCloseSession: (id: string) => void;
};

/**
 * Compact workplace + sessions — same 13px type as Home nav.
 */
export function SidebarRailLists({
  workspaces,
  workspaceBusy = false,
  sessions,
  activeSessionId,
  sessionsActive,
  onOpenWorkspace,
  onManageWorkspaces,
  onNewWorkspace,
  onFocusSession,
  onNewSession,
  onCloseSession,
}: SidebarRailListsProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const entries = workspaces?.entries ?? [];
  const current = useMemo(
    () => entries.find((entry) => entry.is_current) ?? entries[0] ?? null,
    [entries],
  );
  const recent = useMemo(
    () => entries.filter((entry) => !entry.is_current).slice(0, 4),
    [entries],
  );
  const agentSessions = sessions.filter((tab) => tab.kind === "agent");

  return (
    <div className="space-y-3 border-b border-white/6 pb-3" data-testid="ade-rail-context">
      <div>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            disabled={workspaceBusy}
            title={current?.path ?? "Workplace"}
            aria-expanded={pickerOpen}
            data-testid="ade-workplace-switcher"
            onClick={() => setPickerOpen((open) => !open)}
            className="flex min-w-0 flex-1 items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[13px] font-medium text-slate-200 transition hover:bg-white/5 disabled:opacity-50"
          >
            <span className="min-w-0 flex-1 truncate">
              {current?.name ?? "No folder"}
            </span>
            <span className="shrink-0 text-[10px] text-slate-600" aria-hidden>
              {pickerOpen ? "▴" : "▾"}
            </span>
          </button>
          {isTauri() && (
            <button
              type="button"
              title="New workplace"
              aria-label="New workplace"
              disabled={workspaceBusy}
              onClick={onNewWorkspace}
              className="grid size-7 shrink-0 place-items-center rounded-md text-[13px] text-slate-500 hover:bg-white/6 hover:text-slate-200 disabled:opacity-40"
            >
              +
            </button>
          )}
        </div>
        {pickerOpen && (
          <div className="mt-0.5 space-y-px rounded-md border border-white/6 bg-[#080b11] p-0.5">
            {recent.length === 0 && (
              <p className="px-2.5 py-1.5 text-[13px] text-slate-600">
                No other recent folders.
              </p>
            )}
            {recent.map((entry) => (
              <button
                key={entry.path}
                type="button"
                disabled={workspaceBusy}
                title={entry.path}
                onClick={() => {
                  setPickerOpen(false);
                  onOpenWorkspace(entry.path);
                }}
                className="flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left text-[13px] text-slate-400 transition hover:bg-white/5 hover:text-slate-200 disabled:opacity-40"
              >
                <span className="min-w-0 flex-1 truncate">{entry.name}</span>
              </button>
            ))}
            <button
              type="button"
              onClick={() => {
                setPickerOpen(false);
                onManageWorkspaces();
              }}
              className="flex w-full items-center gap-2 rounded px-2.5 py-1.5 text-left text-[13px] text-slate-500 transition hover:bg-white/5 hover:text-slate-300"
            >
              Manage folders…
            </button>
          </div>
        )}
      </div>

      <div>
        <div className="mb-0.5 flex items-center gap-1 px-2.5">
          <span
            className="min-w-0 flex-1 text-[13px] font-medium text-slate-500"
            data-testid="ade-sessions-label"
          >
            Sessions
          </span>
          <button
            type="button"
            title="New agent session"
            aria-label="New agent session"
            data-testid="ade-session-new"
            onClick={onNewSession}
            className="grid size-6 place-items-center rounded text-[13px] text-slate-500 hover:bg-white/6 hover:text-slate-200"
          >
            +
          </button>
        </div>
        <div className="space-y-px" data-testid="ade-session-list">
          {agentSessions.map((tab) => {
            const active = sessionsActive && tab.id === activeSessionId;
            return (
              <div
                key={tab.id}
                className={`group flex w-full items-center gap-0.5 rounded-md ${
                  active ? "bg-blue-500/12" : "hover:bg-white/4"
                }`}
              >
                <button
                  type="button"
                  title={tab.title}
                  onClick={() => onFocusSession(tab.id)}
                  className={`flex min-w-0 flex-1 items-center gap-2 px-2.5 py-1.5 text-left text-[13px] transition ${
                    active
                      ? "font-medium text-blue-200"
                      : "font-normal text-slate-400 group-hover:text-slate-200"
                  }`}
                >
                  <span className="min-w-0 flex-1 truncate">{tab.title}</span>
                </button>
                {tab.closable && (
                  <button
                    type="button"
                    title="Close session"
                    aria-label={`Close ${tab.title}`}
                    onClick={() => onCloseSession(tab.id)}
                    className="mr-1 grid size-6 shrink-0 place-items-center rounded text-[11px] text-slate-600 opacity-0 transition group-hover:opacity-100 hover:bg-white/8 hover:text-slate-300"
                  >
                    ✕
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

export async function fetchWorkspaceList(): Promise<WorkspaceListSnapshot | null> {
  if (!isTauri()) return null;
  return invoke<WorkspaceListSnapshot>("list_workspaces");
}
