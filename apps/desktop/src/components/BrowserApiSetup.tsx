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
 * Browser-preview only: compact status strip + optional token panel.
 * Keeps first paint product-shaped — not a README billboard.
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
      setMessage("Paste the token used when you started the local ADE server.");
      return;
    }
    setBrowserApiToken(trimmed);
    setMessage("Token saved in this browser.");
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
        ? "Offline"
        : probe.authRequired
          ? "Token needed"
          : "Error";

  return (
    <div
      className={`mb-4 rounded-lg border px-3 py-2 text-[12px] ${
        needsAttention
          ? "border-amber-400/20 bg-amber-400/6 text-amber-100/90"
          : "border-emerald-400/15 bg-emerald-400/5 text-emerald-100/85"
      }`}
    >
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-medium text-slate-200">Local API</span>
        <span
          className={`rounded px-1.5 py-0.5 text-[10px] font-semibold ${
            probe?.apiOk
              ? "bg-emerald-400/15 text-emerald-200"
              : "bg-amber-400/15 text-amber-100"
          }`}
        >
          {statusLabel}
        </span>
        <span className="min-w-0 flex-1 truncate font-mono text-[10px] text-slate-500">
          {browserApiBase()}
        </span>
        <Hint text="This browser talks to a local ADE server. Chat, keys, and MCP need the Desktop app." />
        <button
          type="button"
          onClick={() => void runProbe()}
          disabled={busy}
          className="shrink-0 rounded-md border border-white/12 bg-white/4 px-2 py-0.5 text-[10px] font-semibold text-slate-300 hover:bg-white/8 disabled:opacity-50"
        >
          {busy ? "…" : "Recheck"}
        </button>
      </div>

      {needsAttention && (
        <p className="mt-1.5 text-[11px] leading-4 text-slate-400">
          Start ADE on this machine, then paste the matching token below — or open
          the Desktop app for the full shell.
        </p>
      )}

      <Disclosure
        title="Connect token"
        subtitle="Same value as the local ADE server"
        summary={
          hasStoredBrowserApiToken()
            ? "saved"
            : getBrowserApiToken()
              ? "from env"
              : "not set"
        }
        hint="Stored only in this browser. Never commit it."
        defaultOpen={needsAttention}
        storageKey="ade_browser_api_token_panel"
        className="mt-2 border-white/8 bg-black/15"
      >
        <label className="block text-[11px] text-slate-400">
          Token
          <input
            type="password"
            autoComplete="off"
            value={tokenDraft}
            onChange={(event) => setTokenDraft(event.target.value)}
            placeholder="Paste token…"
            className="mt-1 w-full rounded-lg border border-white/10 bg-[#0c121a] px-3 py-2 font-mono text-[12px] text-slate-100 outline-hidden focus:border-blue-400/40"
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
        <Disclosure
          title="Developer setup"
          summary="CLI"
          defaultOpen={false}
          storageKey="ade_browser_api_dev_setup"
          className="mt-2 border-white/6 bg-transparent"
        >
          <p className="text-[10px] leading-4 text-slate-500">
            <span className="font-mono">
              $env:ADE_API_TOKEN=&apos;ade-local-dev&apos;; cargo run -p ade-cli --
              serve --bind 127.0.0.1:3210
            </span>
            <br />
            Optional build-time:{" "}
            <span className="font-mono">VITE_ADE_API_TOKEN</span>
          </p>
        </Disclosure>
        {message && <p className="mt-2 text-[11px] text-slate-300">{message}</p>}
      </Disclosure>
    </div>
  );
}
