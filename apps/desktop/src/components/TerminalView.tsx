import { useCallback, useEffect, useRef, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { invoke, isTauri } from "../ipc";
import { DesktopRequired } from "./DesktopRequired";

type PtyEvent =
  | { type: "data"; data: string }
  | { type: "exit"; code: number | null };

type PtySpawnResult = { sessionId: string; cwd: string };

export function TerminalView() {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sessionRef = useRef<string | null>(null);
  const [cwd, setCwd] = useState<string | null>(null);
  const [status, setStatus] = useState<"idle" | "running" | "exited">("idle");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [rebuildWarnings, setRebuildWarnings] = useState<string[]>([]);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke<{ rebuild_lock_warnings?: string[] }>("get_dashboard")
      .then((dash) => setRebuildWarnings(dash.rebuild_lock_warnings ?? []))
      .catch(() => setRebuildWarnings([]));
  }, []);

  const openSystemTerminal = async () => {
    setError(null);
    try {
      await invoke<string>("open_system_terminal");
    } catch (reason) {
      setError(String(reason));
    }
  };

  const killSession = useCallback(async () => {
    const id = sessionRef.current;
    if (!id) return;
    sessionRef.current = null;
    try {
      await invoke("pty_kill", { sessionId: id });
    } catch {
      /* already gone */
    }
  }, []);

  const startSession = useCallback(async () => {
    if (!isTauri() || !hostRef.current) return;
    setBusy(true);
    setError(null);
    await killSession();

    const term = termRef.current;
    const fit = fitRef.current;
    if (!term || !fit) {
      setBusy(false);
      return;
    }

    term.reset();
    fit.fit();
    const cols = Math.max(term.cols, 40);
    const rows = Math.max(term.rows, 12);

    const onEvent = new Channel<PtyEvent>();
    onEvent.onmessage = (event) => {
      if (event.type === "data") {
        term.write(event.data);
      } else if (event.type === "exit") {
        setStatus("exited");
        sessionRef.current = null;
        term.writeln("");
        term.writeln(
          `\r\n[process exited${event.code != null ? ` with code ${event.code}` : ""}]`,
        );
      }
    };

    try {
      const result = await invoke<PtySpawnResult>("pty_spawn", {
        cols,
        rows,
        onEvent,
      });
      sessionRef.current = result.sessionId;
      setCwd(result.cwd);
      setStatus("running");
      term.focus();
    } catch (reason) {
      setError(String(reason));
      setStatus("idle");
    } finally {
      setBusy(false);
    }
  }, [killSession]);

  useEffect(() => {
    if (!isTauri() || !hostRef.current || termRef.current) return;

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace',
      theme: {
        background: "#0b0f14",
        foreground: "#e2e8f0",
        cursor: "#93c5fd",
        selectionBackground: "#1e3a5f",
      },
      allowProposedApi: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(hostRef.current);
    fit.fit();
    termRef.current = term;
    fitRef.current = fit;

    term.onData((data) => {
      const id = sessionRef.current;
      if (!id) return;
      void invoke("pty_write", { sessionId: id, data }).catch(() => {
        /* ignore write races on exit */
      });
    });

    const onResize = () => {
      fit.fit();
      const id = sessionRef.current;
      if (!id) return;
      void invoke("pty_resize", {
        sessionId: id,
        cols: term.cols,
        rows: term.rows,
      }).catch(() => {
        /* ignore */
      });
    };
    const observer = new ResizeObserver(onResize);
    observer.observe(hostRef.current);
    window.addEventListener("resize", onResize);

    void startSession();

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", onResize);
      void killSession();
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
    // Mount once; startSession/killSession are stable enough for first open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!isTauri()) {
    return <DesktopRequired view="Terminal" />;
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <section className="shrink-0 rounded-xl border border-white/8 bg-[#0d121a] px-4 py-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="text-sm font-semibold text-slate-100">Terminal</h2>
            <p className="mt-0.5 text-[11px] text-slate-500">
              Interactive shell in the attached workspace
              {cwd ? (
                <>
                  {" "}
                  · <span className="font-mono text-slate-400">{cwd}</span>
                </>
              ) : null}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={`rounded-md border px-2 py-1 text-[10px] ${
                status === "running"
                  ? "border-emerald-400/20 bg-emerald-400/8 text-emerald-200/90"
                  : status === "exited"
                    ? "border-amber-400/20 bg-amber-400/8 text-amber-100/90"
                    : "border-white/10 bg-white/5 text-slate-400"
              }`}
            >
              {status === "running" ? "Running" : status === "exited" ? "Exited" : "Idle"}
            </span>
            <button
              type="button"
              className="rounded-md border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] text-slate-200 hover:bg-white/10 disabled:opacity-50"
              disabled={busy}
              onClick={() => void startSession()}
            >
              New shell
            </button>
            <button
              type="button"
              className="rounded-md border border-blue-400/20 bg-blue-400/8 px-2.5 py-1 text-[11px] text-blue-100 hover:bg-blue-400/15"
              onClick={() => void openSystemTerminal()}
            >
              System terminal
            </button>
            <button
              type="button"
              className="rounded-md border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] text-slate-200 hover:bg-white/10 disabled:opacity-50"
              disabled={busy}
              onClick={() => {
                termRef.current?.clear();
              }}
            >
              Clear
            </button>
            <button
              type="button"
              className="rounded-md border border-red-400/20 bg-red-400/8 px-2.5 py-1 text-[11px] text-red-100 hover:bg-red-400/15 disabled:opacity-50"
              disabled={status !== "running"}
              onClick={() => void killSession().then(() => setStatus("exited"))}
            >
              Kill
            </button>
          </div>
        </div>
        {rebuildWarnings.length > 0 && (
          <ul className="mt-2 space-y-1 text-[11px] leading-5 text-amber-100/85">
            {rebuildWarnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        )}
        <p className="mt-2 text-[10px] text-slate-600">
          Agents get <span className="font-mono text-slate-500">shell/run_command</span> in Act/Automate
          (one-shot, dangerous commands blocked). Interactive PTY stays human-only for now.
        </p>
        {error && (
          <p className="mt-2 text-[11px] text-red-200">{error}</p>
        )}
      </section>

      <div
        ref={hostRef}
        className="min-h-[320px] flex-1 overflow-hidden rounded-xl border border-white/8 bg-[#0b0f14] p-2"
      />
    </div>
  );
}
