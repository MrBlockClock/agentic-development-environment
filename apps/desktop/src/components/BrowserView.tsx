import { useEffect, useState } from "react";
import { invoke, isTauri } from "../ipc";
import { DesktopRequired } from "./DesktopRequired";

const PRESETS = [
  { label: "Docs", url: "https://docs.rs/" },
  { label: "crates.io", url: "https://crates.io/" },
  { label: "MDN", url: "https://developer.mozilla.org/" },
  { label: "DuckDuckGo", url: "https://duckduckgo.com/" },
];

function normalizeUrl(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "";
  if (trimmed.includes("://")) return trimmed;
  return `https://${trimmed}`;
}

export function BrowserView() {
  const [url, setUrl] = useState("https://duckduckgo.com/");
  const [openUrl, setOpenUrl] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke<string | null>("browser_window_url")
      .then((current) => {
        if (current) {
          setOpenUrl(current);
          setUrl(current);
        }
      })
      .catch(() => {
        /* window may not exist yet */
      });
  }, []);

  if (!isTauri()) {
    return <DesktopRequired view="Browser" />;
  }

  const open = async (target?: string) => {
    const next = normalizeUrl(target ?? url);
    if (!next) {
      setError("Enter a URL first.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const opened = await invoke<string>("open_browser_window", { url: next });
      setOpenUrl(opened);
      setUrl(opened);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-4">
      <section className="rounded-xl border border-white/8 bg-[#0d121a] p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 className="text-sm font-semibold text-slate-100">In-app Browser</h2>
            <p className="mt-1 max-w-xl text-xs leading-5 text-slate-500">
              Opens a separate WebView2 / Chromium window (not the main ADE shell). Agents can
              always use built-in <span className="font-mono text-slate-400">web_search</span> and{" "}
              <span className="font-mono text-slate-400">web_fetch</span> without this window.
            </p>
          </div>
          {openUrl && (
            <span className="rounded-md border border-emerald-400/20 bg-emerald-400/8 px-2 py-1 text-[10px] text-emerald-200/90">
              Window open
            </span>
          )}
        </div>

        <form
          className="mt-4 flex flex-wrap gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            void open();
          }}
        >
          <input
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            placeholder="https://…"
            className="min-w-[16rem] flex-1 rounded-lg border border-white/10 bg-[#101620] px-3 py-2 text-sm text-slate-200"
          />
          <button
            type="submit"
            disabled={busy}
            className="rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
          >
            {busy ? "Opening…" : openUrl ? "Go" : "Open"}
          </button>
        </form>

        <div className="mt-3 flex flex-wrap gap-1.5">
          {PRESETS.map((preset) => (
            <button
              key={preset.url}
              type="button"
              onClick={() => void open(preset.url)}
              className="rounded-md border border-white/8 bg-white/3 px-2 py-1 text-[11px] text-slate-400 hover:bg-white/6 hover:text-slate-200"
            >
              {preset.label}
            </button>
          ))}
        </div>

        {error && (
          <p className="mt-3 text-xs text-red-300/90">{error}</p>
        )}
        {openUrl && (
          <p className="mt-3 truncate font-mono text-[10px] text-slate-600" title={openUrl}>
            {openUrl}
          </p>
        )}
      </section>
    </div>
  );
}
