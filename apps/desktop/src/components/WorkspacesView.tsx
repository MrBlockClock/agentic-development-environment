import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "../ipc";
import { DesktopRequired } from "./DesktopRequired";

export type WorkspaceEntry = {
  path: string;
  name: string;
  has_agents: boolean;
  has_recipe: boolean;
  is_current: boolean;
  is_ade_source: boolean;
};

type WorkspaceList = {
  current: string;
  entries: WorkspaceEntry[];
  ade_source_root: string | null;
};

export function WorkspacesView({
  onOpened,
  onOpenEnvironment,
}: {
  onOpened: () => void;
  onOpenEnvironment: () => void;
}) {
  const [list, setList] = useState<WorkspaceList | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingAdopt, setPendingAdopt] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    try {
      const next = await invoke<WorkspaceList>("list_workspaces");
      setList(next);
      setError(null);
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (!isTauri()) {
    return <DesktopRequired view="Workspaces" />;
  }

  const openPath = async (path: string) => {
    setBusy(true);
    setError(null);
    setPendingAdopt(null);
    try {
      await invoke("open_workspace", { path });
      await refresh();
      onOpened();
    } catch (reason) {
      const message = String(reason);
      if (message.includes("missing AGENTS.md")) {
        setPendingAdopt(path);
      }
      setError(message);
    } finally {
      setBusy(false);
    }
  };

  const adoptPath = async (path: string) => {
    setBusy(true);
    setError(null);
    try {
      await invoke("create_workspace", { path, projectName: null, force: false });
      setPendingAdopt(null);
      await refresh();
      onOpened();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const pickOpen = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Open ADE workspace folder",
    });
    if (!selected || Array.isArray(selected)) return;
    await openPath(selected);
  };

  const pickCreate = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Create / adopt ADE workspace",
    });
    if (!selected || Array.isArray(selected)) return;
    await adoptPath(selected);
  };

  const openAdeSource = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke("open_ade_on_itself");
      await refresh();
      onOpened();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto max-w-3xl space-y-4">
      <section className="rounded-xl border border-white/8 bg-[#0d121a] p-4">
        <h2 className="text-sm font-semibold text-slate-100">Workspaces</h2>
        <p className="mt-1 text-xs leading-5 text-slate-500">
          Pick which folder ADE is attached to.{" "}
          <span className="text-slate-400">Home</span> works in that folder;{" "}
          <span className="text-slate-400">Environment</span> audits its setup.
        </p>
        <div className="mt-4 flex flex-wrap gap-2">
          <button
            type="button"
            disabled={busy}
            onClick={() => void pickOpen()}
            className="rounded-lg bg-blue-500 px-3.5 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
          >
            Open folder…
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => void pickCreate()}
            className="rounded-lg border border-white/10 bg-white/4 px-3.5 py-2 text-xs font-semibold text-slate-200 hover:bg-white/8 disabled:opacity-50"
          >
            Create / Adopt…
          </button>
          {list?.ade_source_root && (
            <button
              type="button"
              disabled={busy}
              onClick={() => void openAdeSource()}
              className="rounded-lg border border-white/10 px-3.5 py-2 text-xs font-semibold text-slate-400 hover:bg-white/6 hover:text-slate-200 disabled:opacity-50"
            >
              Open ADE on itself
            </button>
          )}
          <button
            type="button"
            disabled={busy}
            onClick={onOpenEnvironment}
            className="rounded-lg border border-white/10 px-3.5 py-2 text-xs font-semibold text-slate-400 hover:bg-white/6 hover:text-slate-200"
          >
            Environment audit →
          </button>
        </div>
        {error && <p className="mt-3 text-xs text-red-300/90">{error}</p>}
        {pendingAdopt && (
          <div className="mt-3 rounded-lg border border-amber-400/25 bg-amber-400/8 px-3 py-2 text-xs text-amber-100/90">
            <p>
              That folder has no <span className="font-mono">AGENTS.md</span>. Adopt it as an ADE
              workspace?
            </p>
            <button
              type="button"
              disabled={busy}
              onClick={() => void adoptPath(pendingAdopt)}
              className="mt-2 rounded-md bg-amber-500/90 px-3 py-1.5 text-[11px] font-semibold text-slate-950 hover:bg-amber-400 disabled:opacity-50"
            >
              Adopt folder
            </button>
          </div>
        )}
      </section>

      <section className="rounded-xl border border-white/8 bg-[#0d121a] p-4">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-slate-500">
            Current & recent
          </h3>
          <button
            type="button"
            onClick={() => void refresh()}
            className="text-[10px] text-slate-500 hover:text-slate-300"
          >
            Refresh
          </button>
        </div>
        <ul className="mt-3 space-y-2">
          {(list?.entries ?? []).map((entry) => (
            <li
              key={entry.path}
              className={`rounded-lg border px-3 py-2.5 ${
                entry.is_current
                  ? "border-blue-400/30 bg-blue-500/10"
                  : "border-white/7 bg-black/20"
              }`}
            >
              <div className="flex flex-wrap items-start justify-between gap-2">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium text-slate-100">{entry.name}</span>
                    {entry.is_current && (
                      <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] font-semibold text-blue-200">
                        current
                      </span>
                    )}
                    {entry.is_ade_source && (
                      <span className="rounded bg-violet-500/15 px-1.5 py-0.5 text-[10px] text-violet-200">
                        ADE source
                      </span>
                    )}
                    {entry.has_recipe ? (
                      <span className="text-[10px] text-emerald-300/80">recipe</span>
                    ) : (
                      <span className="text-[10px] text-slate-600">no recipe</span>
                    )}
                  </div>
                  <p className="mt-1 truncate font-mono text-[10px] text-slate-500" title={entry.path}>
                    {entry.path}
                  </p>
                </div>
                {!entry.is_current && (
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => void openPath(entry.path)}
                    className="shrink-0 rounded-md border border-white/10 px-2.5 py-1 text-[11px] font-semibold text-slate-300 hover:bg-white/6 disabled:opacity-50"
                  >
                    Switch
                  </button>
                )}
                {entry.is_current && (
                  <button
                    type="button"
                    onClick={onOpenEnvironment}
                    className="shrink-0 rounded-md border border-white/10 px-2.5 py-1 text-[11px] font-semibold text-slate-300 hover:bg-white/6"
                  >
                    Audit
                  </button>
                )}
              </div>
            </li>
          ))}
          {list && list.entries.length === 0 && (
            <li className="text-xs text-slate-500">No workspaces yet — Open or Create a folder.</li>
          )}
        </ul>
      </section>
    </div>
  );
}
