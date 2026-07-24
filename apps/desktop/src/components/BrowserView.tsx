import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "../ipc";
import { DesktopRequired } from "./DesktopRequired";

/** Prefer http for local servers; https for public hosts. */
export function normalizeUrl(raw: string): string {
  const trimmed = raw.trim();
  if (!trimmed) return "https://www.google.com/";
  if (trimmed.includes("://") || trimmed.toLowerCase() === "about:blank") {
    return trimmed;
  }
  const local =
    /^localhost(?::\d+)?(?:\/.*)?$/i.test(trimmed) ||
    /^127\.0\.0\.1(?::\d+)?(?:\/.*)?$/i.test(trimmed) ||
    /^\[::1\](?::\d+)?(?:\/.*)?$/i.test(trimmed);
  if (local) {
    return `http://${trimmed}`;
  }
  if (/^[\w.-]+\.[a-z]{2,}([/:].*)?$/i.test(trimmed)) {
    return `https://${trimmed}`;
  }
  return `https://www.google.com/search?q=${encodeURIComponent(trimmed)}`;
}

function hostLabel(url: string): string {
  try {
    const parsed = new URL(normalizeUrl(url));
    if (parsed.protocol === "about:") return "New tab";
    return parsed.hostname.replace(/^www\./, "") || "Browser";
  } catch {
    return "Browser";
  }
}

type BrowserViewProps = {
  instanceId: string;
  initialUrl?: string;
  active?: boolean;
  onTitleChange?: (title: string) => void;
};

/**
 * In-ADE Chromium/WebView2 pane.
 */
export function BrowserView({
  instanceId,
  initialUrl = "https://www.google.com/",
  active = true,
  onTitleChange,
}: BrowserViewProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const label = `ade-browser-${instanceId}`;
  const [draft, setDraft] = useState(initialUrl);
  const [current, setCurrent] = useState(initialUrl);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const readyRef = useRef(false);
  const activeRef = useRef(active);
  activeRef.current = active;

  const waitForHost = useCallback(async () => {
    for (let i = 0; i < 30; i += 1) {
      const el = hostRef.current;
      if (el) {
        const rect = el.getBoundingClientRect();
        if (rect.width >= 8 && rect.height >= 8) return el;
      }
      await new Promise((r) => requestAnimationFrame(() => r(undefined)));
    }
    return hostRef.current;
  }, []);

  const syncBounds = useCallback(async () => {
    if (!isTauri() || !hostRef.current || !readyRef.current) return;
    const rect = hostRef.current.getBoundingClientRect();
    if (rect.width < 8 || rect.height < 8) return;
    try {
      await invoke("browser_set_bounds", {
        label,
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
      });
    } catch {
      /* pane may not exist yet */
    }
  }, [label]);

  const embed = useCallback(
    async (url: string) => {
      if (!isTauri()) return;
      const host = await waitForHost();
      if (!host) {
        setError("Browser host not ready.");
        return;
      }
      const rect = host.getBoundingClientRect();
      const next = normalizeUrl(url);
      setBusy(true);
      setError(null);
      try {
        const opened = await invoke<string>("browser_embed", {
          label,
          url: next,
          x: Math.max(0, rect.left),
          y: Math.max(0, rect.top),
          width: Math.max(120, rect.width || 640),
          height: Math.max(120, rect.height || 480),
        });
        readyRef.current = true;
        setReady(true);
        setCurrent(opened);
        setDraft(opened);
        onTitleChange?.(hostLabel(opened));
        if (!activeRef.current) {
          await invoke("browser_set_visible", { label, visible: false });
        } else {
          await invoke("browser_set_visible", { label, visible: true });
          await syncBounds();
        }
      } catch (reason) {
        setError(String(reason));
        setReady(false);
        readyRef.current = false;
      } finally {
        setBusy(false);
      }
    },
    [label, onTitleChange, syncBounds, waitForHost],
  );

  const go = useCallback(
    async (raw?: string) => {
      const next = normalizeUrl(raw ?? draft);
      if (!readyRef.current) {
        await embed(next);
        return;
      }
      setBusy(true);
      setError(null);
      try {
        const opened = await invoke<string>("browser_navigate", {
          label,
          url: next,
        });
        setCurrent(opened);
        setDraft(opened);
        onTitleChange?.(hostLabel(opened));
        if (activeRef.current) {
          await invoke("browser_set_visible", { label, visible: true });
        }
      } catch {
        await embed(next);
      } finally {
        setBusy(false);
      }
    },
    [draft, embed, label, onTitleChange],
  );

  useEffect(() => {
    if (!isTauri()) return;
    void embed(initialUrl);
    return () => {
      readyRef.current = false;
      setReady(false);
      void invoke("close_browser_window", { label }).catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- once per tab
  }, [instanceId]);

  useEffect(() => {
    if (!isTauri() || !readyRef.current) return;
    void invoke("browser_set_visible", { label, visible: active }).catch(
      () => {},
    );
    if (active) void syncBounds();
  }, [active, label, syncBounds]);

  useEffect(() => {
    if (!isTauri() || !hostRef.current) return;
    const node = hostRef.current;
    const ro = new ResizeObserver(() => {
      if (activeRef.current) void syncBounds();
    });
    ro.observe(node);
    const onWin = () => {
      if (activeRef.current) void syncBounds();
    };
    window.addEventListener("resize", onWin);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", onWin);
    };
  }, [syncBounds]);

  if (!isTauri()) {
    return <DesktopRequired view="Browser" />;
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-white/8 bg-[#0a0e14]">
      <form
        className="flex shrink-0 items-center gap-1.5 border-b border-white/8 px-2 py-1.5"
        onSubmit={(event) => {
          event.preventDefault();
          void go();
        }}
      >
        <button
          type="button"
          title="Reload"
          disabled={busy}
          className="grid size-7 place-items-center rounded-md text-slate-500 hover:bg-white/6 hover:text-slate-200 disabled:opacity-40"
          onClick={() => void go(current)}
        >
          ↻
        </button>
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          spellCheck={false}
          placeholder="Search Google or type a URL"
          className="min-w-0 flex-1 rounded-md border border-white/10 bg-[#101620] px-2.5 py-1.5 text-[12px] text-slate-200 outline-hidden focus:border-blue-400/40"
        />
        <button
          type="submit"
          disabled={busy}
          className="rounded-md bg-blue-500 px-2.5 py-1.5 text-[11px] font-semibold text-white hover:bg-blue-400 disabled:opacity-50"
        >
          {busy ? "…" : "Go"}
        </button>
      </form>

      <div ref={hostRef} className="relative min-h-0 flex-1 bg-[#101620]">
        {!ready && !error && (
          <div className="absolute inset-0 grid place-items-center text-[12px] text-slate-500">
            Starting Chromium…
          </div>
        )}
        {error && (
          <div className="absolute inset-0 grid place-items-center p-6 text-center">
            <div>
              <p className="text-sm font-semibold text-slate-200">
                Browser failed to start
              </p>
              <p className="mt-1 max-w-md text-[11px] text-red-300/90">{error}</p>
              <button
                type="button"
                className="mt-3 rounded-md bg-blue-500 px-3 py-1.5 text-[11px] font-semibold text-white"
                onClick={() => void embed("https://www.google.com/")}
              >
                Retry Google
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
