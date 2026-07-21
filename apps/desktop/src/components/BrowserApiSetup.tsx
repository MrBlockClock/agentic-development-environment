import { useCallback, useEffect, useState } from "react";
import {
  browserApiBase,
  clearBrowserApiToken,
  getBrowserApiToken,
  hasStoredBrowserApiToken,
  probeBrowserApi,
  setBrowserApiToken,
  type BrowserApiProbe,
} from "../ipc";
import { Disclosure, Hint } from "./ui";

type BrowserApiSetupProps = {
  /** Bump when parent wants a re-probe (e.g. after refresh). */
  refreshKey?: number;
  onResolved?: () => void;
};

/**
 * Browser-preview only: match `ADE_API_TOKEN` on `ade serve` without hardcoding.
 */
export function BrowserApiSetup({ refreshKey = 0, onResolved }: BrowserApiSetupProps) {
  const [tokenDraft, setTokenDraft] = useState(() => getBrowserApiToken() ?? "");
  const [probe, setProbe] = useState<BrowserApiProbe | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const runProbe = useCallback(async () => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await probeBrowserApi();
      setProbe(result);
      if (result.apiOk) {
        onResolved?.();
      }
    } catch (reason) {
      setProbe({
        reachable: false,
        apiOk: false,
        authRequired: null,
        detail: String(reason),
      });
    } finally {
      setBusy(false);
    }
  }, [onResolved]);

  useEffect(() => {
    void runProbe();
  }, [runProbe, refreshKey]);

  const save = async () => {
    const trimmed = tokenDraft.trim();
    if (!trimmed) {
      setMessage("Paste the same value as ADE_API_TOKEN on `ade serve`.");
      return;
    }
    setBrowserApiToken(trimmed);
    setMessage("Token saved in this browser (localStorage).");
    await runProbe();
  };

  const clear = async () => {
    clearBrowserApiToken();
    setTokenDraft("");
    setMessage("Cleared browser API token.");
    await runProbe();
  };

  const needsAttention = !probe?.apiOk;
  const statusLabel = !probe
    ? "Checking…"
    : probe.apiOk
      ? "Connected"
      : !probe.reachable
        ? "API offline"
        : probe.authRequired
          ? "Token needed"
          : "API error";

  return (
    <div
      className={`mb-5 rounded-xl border px-4 py-3 text-[12px] leading-5 ${
        needsAttention
          ? "border-amber-400/25 bg-amber-400/8 text-amber-100/90"
          : "border-emerald-400/20 bg-emerald-400/6 text-emerald-100/85"
      }`}
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2 font-semibold">
            <span>Browser preview</span>
            <span
              className={`rounded px-1.5 py-0.5 text-[10px] font-semibold ${
                probe?.apiOk
                  ? "bg-emerald-400/15 text-emerald-200"
                  : "bg-amber-400/15 text-amber-100"
              }`}
            >
              {statusLabel}
            </span>
            <Hint text="Chat, provider Keys, and MCP need the Desktop app. This preview talks to `ade serve` over loopback HTTP only." />
          </div>
          <p className="mt-1 text-[11px] opacity-90">
            API <span className="font-mono text-[10px]">{browserApiBase()}</span>
            {probe?.detail ? ` · ${probe.detail}` : ""}
          </p>
          <p className="mt-1 text-[11px] opacity-80">
            Scopes: status/verify/guidance when authorized. No agent turns or EXECUTE over
            HTTP — open Desktop for those.
          </p>
        </div>
        <button
          type="button"
          onClick={() => void runProbe()}
          disabled={busy}
          className="shrink-0 rounded-lg border border-white/15 bg-white/5 px-2.5 py-1 text-[11px] font-semibold text-slate-200 hover:bg-white/10 disabled:opacity-50"
        >
          {busy ? "…" : "Recheck"}
        </button>
      </div>

      <Disclosure
        title="Local API token"
        subtitle="Must match ADE_API_TOKEN on ade serve"
        summary={hasStoredBrowserApiToken() ? "saved" : getBrowserApiToken() ? "from env" : "not set"}
        hint="Stored only in this browser’s localStorage. Never commit it."
        defaultOpen={needsAttention}
        forceOpen={needsAttention}
        storageKey="ade_browser_api_token_panel"
        className="mt-3 border-white/10 bg-black/20"
      >
        <label className="block text-[11px] text-slate-400">
          Bearer token
          <input
            type="password"
            autoComplete="off"
            value={tokenDraft}
            onChange={(event) => setTokenDraft(event.target.value)}
            placeholder="Same value as ADE_API_TOKEN"
            className="mt-1 w-full rounded-lg border border-white/10 bg-[#0c121a] px-3 py-2 font-mono text-[12px] text-slate-100 outline-none focus:border-blue-400/40"
          />
        </label>
        <div className="mt-2 flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void save()}
            disabled={busy || !tokenDraft.trim()}
            className="rounded-lg bg-blue-500 px-3 py-1.5 text-[11px] font-semibold text-white hover:bg-blue-400 disabled:opacity-50"
          >
            Save & check
          </button>
          <button
            type="button"
            onClick={() => void clear()}
            disabled={busy || !hasStoredBrowserApiToken()}
            className="rounded-lg border border-white/15 px-3 py-1.5 text-[11px] font-semibold text-slate-300 hover:bg-white/5 disabled:opacity-50"
          >
            Clear
          </button>
        </div>
        <p className="mt-2 text-[10px] leading-4 text-slate-500">
          Example:{" "}
          <span className="font-mono">
            $env:ADE_API_TOKEN=&apos;ade-local-dev&apos;; ade serve --bind 127.0.0.1:3210
          </span>
          , then paste the same token here. Optional build-time:{" "}
          <span className="font-mono">VITE_ADE_API_TOKEN</span>.
        </p>
        {message && <p className="mt-2 text-[11px] text-slate-300">{message}</p>}
      </Disclosure>
    </div>
  );
}
