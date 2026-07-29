import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke, isTauri } from "../ipc";
import {
  DEFAULT_BASE_URL,
  DEFAULT_CONTEXT_WINDOW,
  DEFAULT_MODEL,
  DEFAULT_PROVIDER,
  PROVIDER_PRESETS,
  canonicalBaseUrl,
  firstModelId,
  formatContextTokens,
  modelContextWindow,
  modelOptionLabel,
  presetById,
  type ProviderPreset,
} from "../providers";
import { DarkSelect } from "./DarkSelect";
import { DesktopRequired } from "./DesktopRequired";
import { Disclosure, Hint } from "./ui";

const AGENT_PROVIDER_KEY = "ade_agent_provider";
const AGENT_BASE_URL_KEY = "ade_agent_base_url";
const AGENT_MODEL_KEY = "ade_agent_model";
const AGENT_CONTEXT_KEY = "ade_agent_context_window";

type ProviderKeyStatus = {
  profile: string;
  provider: string;
  configured: boolean;
};

type ProviderKeyDeleteResult = {
  profile: string;
  provider: string;
  deleted: boolean;
};

type ProviderKeySmokeResult = {
  profile: string;
  provider: string;
  status: "ready" | "passed" | "failed" | "skipped";
  detail: string;
};

type ProviderVaultRow = {
  provider: string;
  configured: boolean;
};

type KeysViewProps = {
  simpleMode?: boolean;
  onContinueToAgent?: () => void;
  onOpenIntegrations?: () => void;
};

type EnvKeyCandidate = {
  provider: string;
  env_var: string;
};

function readStored(key: string, fallback: string): string {
  if (typeof window === "undefined") return fallback;
  return window.localStorage.getItem(key) || fallback;
}

function defaultVisibleProviderIds(active: string): string[] {
  const listed = PROVIDER_PRESETS.filter(
    (preset) => preset.recommended || preset.listed,
  ).map((preset) => preset.id);
  return Array.from(new Set([active, ...listed]));
}

function maskLabel(configured: boolean): string {
  return configured ? "••••••••" : "No key";
}

/**
 * FreeLLMAPI-inspired Keys: compact provider rows, one open editor, + to add.
 * Tier 0 = active provider + model + go Home. Tier 1 = key / test. Tier 2 = base URL / paid test.
 */
export function KeysView({
  simpleMode = false,
  onContinueToAgent,
  onOpenIntegrations,
}: KeysViewProps) {
  const profile = "local";
  const [provider, setProvider] = useState(() =>
    readStored(AGENT_PROVIDER_KEY, DEFAULT_PROVIDER),
  );
  const [secret, setSecret] = useState("");
  const [baseUrl, setBaseUrl] = useState(() => {
    const storedProvider = readStored(AGENT_PROVIDER_KEY, DEFAULT_PROVIDER);
    return canonicalBaseUrl(
      storedProvider,
      readStored(AGENT_BASE_URL_KEY, DEFAULT_BASE_URL),
    );
  });
  const [model, setModel] = useState(() =>
    readStored(AGENT_MODEL_KEY, DEFAULT_MODEL),
  );
  const [contextWindow, setContextWindow] = useState(() => {
    const stored = Number(readStored(AGENT_CONTEXT_KEY, ""));
    if (Number.isFinite(stored) && stored > 0) return String(Math.floor(stored));
    const providerId = readStored(AGENT_PROVIDER_KEY, DEFAULT_PROVIDER);
    const modelId = readStored(AGENT_MODEL_KEY, DEFAULT_MODEL);
    return String(modelContextWindow(providerId, modelId));
  });
  const [inputCostPerMtok, setInputCostPerMtok] = useState("");
  const [outputCostPerMtok, setOutputCostPerMtok] = useState("");
  const [maxCostUsd, setMaxCostUsd] = useState("0.05");
  const [approveLiveCost, setApproveLiveCost] = useState(false);
  const [vaultRows, setVaultRows] = useState<ProviderVaultRow[]>([]);
  const [status, setStatus] = useState<ProviderKeyStatus | null>(null);
  const [smoke, setSmoke] = useState<ProviderKeySmokeResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(() =>
    readStored(AGENT_PROVIDER_KEY, DEFAULT_PROVIDER),
  );
  const [visibleIds, setVisibleIds] = useState<string[]>(() =>
    defaultVisibleProviderIds(
      readStored(AGENT_PROVIDER_KEY, DEFAULT_PROVIDER),
    ),
  );
  const [envCandidates, setEnvCandidates] = useState<EnvKeyCandidate[]>([]);
  const [addOpen, setAddOpen] = useState(false);
  const addMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!addOpen) return;
    const onPointer = (event: MouseEvent) => {
      if (!addMenuRef.current?.contains(event.target as Node)) {
        setAddOpen(false);
      }
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setAddOpen(false);
    };
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [addOpen]);

  const refreshAll = useCallback(async () => {
    if (!isTauri()) return;
    setBusy(true);
    setMessage(null);
    try {
      const [rows, candidates] = await Promise.all([
        invoke<ProviderVaultRow[]>("key_status_all", { profile }),
        invoke<EnvKeyCandidate[]>("key_env_candidates").catch(
          () => [] as EnvKeyCandidate[],
        ),
      ]);
      setVaultRows(rows);
      setEnvCandidates(candidates);
      const configuredIds = rows
        .filter((row) => row.configured)
        .map((row) => row.provider);
      setVisibleIds((prev) =>
        Array.from(new Set([...prev, ...configuredIds])),
      );
      if (provider.trim()) {
        const one = await invoke<ProviderKeyStatus>("key_status", {
          provider: provider.trim(),
          profile,
        });
        setStatus(one);
      }
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }, [profile, provider]);

  useEffect(() => {
    void refreshAll();
  }, [refreshAll]);

  useEffect(() => {
    window.localStorage.setItem(
      AGENT_PROVIDER_KEY,
      provider.trim() || DEFAULT_PROVIDER,
    );
  }, [provider]);
  useEffect(() => {
    if (baseUrl.trim()) {
      window.localStorage.setItem(AGENT_BASE_URL_KEY, baseUrl.trim());
    }
  }, [baseUrl]);
  useEffect(() => {
    if (model.trim()) {
      window.localStorage.setItem(AGENT_MODEL_KEY, model.trim());
    }
  }, [model]);
  useEffect(() => {
    const n = Number(contextWindow);
    if (Number.isFinite(n) && n > 0) {
      window.localStorage.setItem(AGENT_CONTEXT_KEY, String(Math.floor(n)));
    }
  }, [contextWindow]);

  const configuredMap = useMemo(() => {
    const map = new Map<string, boolean>();
    for (const row of vaultRows) {
      map.set(row.provider, row.configured);
    }
    if (status && status.provider === provider) {
      map.set(status.provider, status.configured);
    }
    return map;
  }, [vaultRows, status, provider]);

  const configuredCount = useMemo(
    () => [...configuredMap.values()].filter(Boolean).length,
    [configuredMap],
  );

  const currentPreset = presetById(provider);
  const isCustom = Boolean(currentPreset?.custom);
  const contextOverride = Number(contextWindow);
  const activeContext = modelContextWindow(
    provider,
    model,
    isCustom && Number.isFinite(contextOverride) && contextOverride > 0
      ? contextOverride
      : null,
  );
  const modelOptions =
    currentPreset?.models?.length
      ? currentPreset.models
      : model.trim()
        ? [model.trim()]
        : [DEFAULT_MODEL];

  const listedPresets = useMemo(
    () =>
      PROVIDER_PRESETS.filter((preset) => visibleIds.includes(preset.id)),
    [visibleIds],
  );

  const addablePresets = useMemo(
    () => PROVIDER_PRESETS.filter((preset) => !visibleIds.includes(preset.id)),
    [visibleIds],
  );

  const applyPreset = (preset: ProviderPreset, expand = true) => {
    setProvider(preset.id);
    setBaseUrl(
      preset.custom
        ? readStored(AGENT_BASE_URL_KEY, preset.baseUrl) || preset.baseUrl
        : preset.baseUrl,
    );
    const nextModel = preset.custom
      ? readStored(AGENT_MODEL_KEY, firstModelId(preset)) || firstModelId(preset)
      : firstModelId(preset);
    setModel(nextModel);
    setContextWindow(
      String(
        modelContextWindow(
          preset.id,
          nextModel,
          preset.custom
            ? Number(readStored(AGENT_CONTEXT_KEY, "")) || null
            : null,
        ),
      ),
    );
    setStatus(null);
    setSmoke(null);
    setSecret("");
    setVisibleIds((prev) =>
      prev.includes(preset.id) ? prev : [...prev, preset.id],
    );
    if (expand) setExpandedId(preset.id);
  };

  const selectActive = (preset: ProviderPreset) => {
    setProvider(preset.id);
    if (preset.custom) {
      setBaseUrl(
        baseUrl.trim() ||
          readStored(AGENT_BASE_URL_KEY, preset.baseUrl) ||
          preset.baseUrl,
      );
    } else {
      setBaseUrl(preset.baseUrl);
    }
    if (preset.models.length > 0 && !preset.models.includes(model)) {
      const next = firstModelId(preset);
      setModel(next);
      setContextWindow(String(modelContextWindow(preset.id, next)));
    } else if (!preset.custom) {
      setContextWindow(String(modelContextWindow(preset.id, model)));
    }
    setSmoke(null);
  };

  const save = async (andContinue: boolean) => {
    if (!secret.trim()) return;
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeyStatus>("key_set", {
        provider: provider.trim(),
        profile,
        secret,
      });
      setSecret("");
      setStatus(result);
      setSmoke(null);
      window.localStorage.setItem(AGENT_PROVIDER_KEY, result.provider);
      window.localStorage.setItem(AGENT_BASE_URL_KEY, baseUrl.trim());
      window.localStorage.setItem(AGENT_MODEL_KEY, model.trim());
      const ctx = Number(contextWindow);
      if (Number.isFinite(ctx) && ctx > 0) {
        window.localStorage.setItem(
          AGENT_CONTEXT_KEY,
          String(Math.floor(ctx)),
        );
      }
      await refreshAll();
      setMessage(
        andContinue ? "Key saved. Opening Home…" : "Key saved to OS vault.",
      );
      if (andContinue) onContinueToAgent?.();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (providerId: string) => {
    if (
      !window.confirm(
        `Delete the ${providerId} credential from profile ${profile}?`,
      )
    ) {
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeyDeleteResult>("key_delete", {
        provider: providerId,
        profile,
      });
      if (providerId === provider) {
        setStatus({
          profile: result.profile,
          provider: result.provider,
          configured: false,
        });
      }
      setSmoke(null);
      setMessage(
        result.deleted ? "Credential deleted." : "No credential was configured.",
      );
      await refreshAll();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const runSmoke = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeySmokeResult>("key_smoke", {
        provider: provider.trim(),
        profile,
      });
      setSmoke(result);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const runLiveSmoke = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeySmokeResult>("key_live_smoke", {
        provider: provider.trim(),
        profile,
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        inputCostPerMtok: Number(inputCostPerMtok),
        outputCostPerMtok: Number(outputCostPerMtok),
        maxCostUsd: Number(maxCostUsd),
        approveCost: approveLiveCost,
      });
      setSmoke(result);
      setApproveLiveCost(false);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const importOpenCodeAuth = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<{
        imported: string[];
        skipped: string[];
        detail: string;
      }>("key_import_opencode_auth", { profile });
      setMessage(
        `Imported ${result.imported.join(", ") || "(none)"}. ${result.detail}`,
      );
      if (result.imported.includes("opencode")) {
        applyPreset(presetById("opencode") ?? PROVIDER_PRESETS[0], true);
      }
      await refreshAll();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const importEnvKeys = async (force = false, onlyProvider?: string) => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<{
        imported: string[];
        skipped: string[];
        detail: string;
      }>("key_import_env", {
        profile,
        force,
        provider: onlyProvider ?? null,
      });
      setMessage(
        `${result.detail}${
          result.imported.length
            ? ` · ${result.imported.join(", ")}`
            : ""
        }`,
      );
      if (result.imported.length > 0) {
        setVisibleIds((prev) =>
          Array.from(new Set([...prev, ...result.imported])),
        );
        const first = presetById(result.imported[0]!);
        if (first) applyPreset(first, true);
      }
      await refreshAll();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const activateKeyless = async (preset: ProviderPreset) => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeyStatus>("key_activate_keyless", {
        profile,
        provider: preset.id,
      });
      setStatus(result);
      applyPreset(preset, true);
      setMessage(`Activated ${preset.label} (keyless).`);
      await refreshAll();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const envCandidateMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const row of envCandidates) {
      map.set(row.provider, row.env_var);
    }
    return map;
  }, [envCandidates]);

  const activeConfigured = Boolean(configuredMap.get(provider));
  const canGoHome = activeConfigured || Boolean(secret.trim());

  if (!isTauri()) {
    return <DesktopRequired view="Keys" />;
  }

  return (
    <div className="mx-auto w-full max-w-2xl space-y-4">
      <h1 className="text-lg font-semibold tracking-tight text-slate-50">
        API keys
      </h1>
      <p className="text-[12px] leading-5 text-slate-500">
        Pick a provider, paste a key, choose a model — then go Home.
      </p>

      {/* Tier 0 — active strip */}
      <section className="rounded-xl border border-white/8 bg-[#0d121a] p-3 sm:p-3.5">
        <div className="flex flex-wrap items-center gap-2">
          <div className="min-w-0 flex-1">
            <div className="text-[10px] font-semibold uppercase tracking-wider text-slate-600">
              Active for Home
            </div>
            <div className="mt-0.5 flex flex-wrap items-center gap-2">
              <span className="text-[13px] font-semibold text-slate-100">
                {currentPreset?.label ?? provider}
              </span>
              <span
                className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium ${
                  activeConfigured
                    ? "bg-emerald-400/10 text-emerald-300"
                    : "bg-amber-400/10 text-amber-200"
                }`}
              >
                <span
                  className={`size-1.5 rounded-full ${
                    activeConfigured ? "bg-emerald-400" : "bg-amber-300"
                  }`}
                  aria-hidden
                />
                {busy && expandedId === provider
                  ? "…"
                  : activeConfigured
                    ? "Connected"
                    : "Needs key"}
              </span>
            </div>
          </div>
          {isCustom ? (
            <input
              value={model}
              onChange={(event) => setModel(event.target.value)}
              spellCheck={false}
              placeholder="model id"
              className="min-w-40 rounded-lg border border-white/10 bg-[#101620] px-2.5 py-1.5 font-mono text-[11px] text-slate-200 outline-hidden focus:border-blue-400/40"
            />
          ) : (
            <DarkSelect
              ariaLabel="Model for Home"
              value={
                modelOptions.includes(model) ? model : modelOptions[0] ?? ""
              }
              options={modelOptions.map((id) => ({
                value: id,
                label: modelOptionLabel(provider, id),
              }))}
              onChange={(next) => {
                setModel(next);
                setContextWindow(String(modelContextWindow(provider, next)));
              }}
              className="min-w-44"
              maxLabelChars={40}
            />
          )}
          <span className="rounded-md border border-white/8 bg-white/3 px-2 py-1 text-[10px] font-medium text-slate-400">
            {formatContextTokens(activeContext)} context
          </span>
          <button
            type="button"
            onClick={() => {
              if (activeConfigured) {
                onContinueToAgent?.();
              }
            }}
            disabled={busy || !provider.trim() || !canGoHome || !activeConfigured}
            className="rounded-lg bg-blue-500 px-3.5 py-1.5 text-[11px] font-semibold text-white hover:bg-blue-400 disabled:opacity-50"
          >
            Go Home
          </button>
          {onOpenIntegrations && (
            <button
              type="button"
              onClick={onOpenIntegrations}
              className="rounded-lg border border-white/12 bg-white/5 px-3 py-1.5 text-[11px] font-semibold text-slate-200 hover:bg-white/8"
              title="GitHub, Stripe, Azure tokens and MCP recipes"
            >
              Integrations
            </button>
          )}
        </div>
        {secret.trim() && expandedId === provider && (
          <p className="mt-2 text-[11px] text-amber-200/80">
            Unsaved key in the editor below — use Save key there before Go Home.
          </p>
        )}
      </section>

      {/* Provider list */}
      <section className="overflow-hidden rounded-xl border border-white/8 bg-[#0d121a]">
        <div className="flex items-center justify-between gap-2 border-b border-white/6 px-3.5 py-2.5">
          <div>
            <h2 className="text-[12px] font-semibold text-slate-200">
              Providers
            </h2>
            <p className="text-[10px] text-slate-600">
              {configuredCount} key{configuredCount === 1 ? "" : "s"} in vault ·
              {envCandidates.length > 0
                ? ` ${envCandidates.length} in env · `
                : " "}
              expand one to edit
            </p>
          </div>
          <div className="flex items-center gap-1.5">
            <button
              type="button"
              disabled={busy || envCandidates.length === 0}
              onClick={() => void importEnvKeys(false)}
              className="rounded-md border border-white/10 px-2 py-1 text-[10px] font-medium text-slate-400 hover:bg-white/5 hover:text-slate-200 disabled:opacity-40"
              title={
                envCandidates.length
                  ? `Import ${envCandidates.map((c) => c.env_var).join(", ")}`
                  : "No GROQ_API_KEY / FREELMAPI_KEY / … in this process env"
              }
            >
              Import env
            </button>
            <div className="relative" ref={addMenuRef}>
              <button
                type="button"
                aria-expanded={addOpen}
                aria-haspopup="menu"
                disabled={addablePresets.length === 0}
                onClick={() => setAddOpen((open) => !open)}
                className="grid size-7 place-items-center rounded-md border border-white/10 bg-white/3 text-[15px] font-medium text-slate-300 hover:bg-white/6 disabled:opacity-40"
                title={
                  addablePresets.length
                    ? "Add provider"
                    : "All known providers listed"
                }
              >
                +
              </button>
              {addOpen && addablePresets.length > 0 && (
                <div
                  role="menu"
                  className="absolute right-0 z-40 mt-1.5 w-52 overflow-hidden rounded-lg border border-white/12 bg-[#121820] py-1 shadow-[0_12px_40px_rgba(0,0,0,0.45)]"
                >
                  {addablePresets.map((preset) => (
                    <button
                      key={preset.id}
                      type="button"
                      role="menuitem"
                      className="flex w-full flex-col px-3 py-1.5 text-left hover:bg-white/6"
                      onClick={() => {
                        applyPreset(preset, true);
                        setAddOpen(false);
                      }}
                    >
                      <span className="text-[11px] font-medium text-slate-200">
                        {preset.label}
                      </span>
                      <span className="truncate text-[10px] text-slate-600">
                        {preset.hint ?? preset.baseUrl}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        <ul className="divide-y divide-white/6">
          {listedPresets.map((preset) => {
            const isActive = provider === preset.id;
            const isExpanded = expandedId === preset.id;
            const configured = Boolean(configuredMap.get(preset.id));
            const getKeyUrl = preset.getKeyUrl;
            const envVar = envCandidateMap.get(preset.id);

            return (
              <li key={preset.id}>
                <div
                  className={`flex items-center gap-2 px-3 py-2.5 sm:px-3.5 ${
                    isActive ? "bg-blue-500/6" : ""
                  }`}
                >
                  <button
                    type="button"
                    role="radio"
                    aria-checked={isActive}
                    title="Use for Home"
                    onClick={() => selectActive(preset)}
                    className={`grid size-4 shrink-0 place-items-center rounded-full border transition ${
                      isActive
                        ? "border-blue-400 bg-blue-500"
                        : "border-white/20 bg-transparent hover:border-white/35"
                    }`}
                  >
                    {isActive && (
                      <span className="size-1.5 rounded-full bg-white" />
                    )}
                  </button>

                  <button
                    type="button"
                    className="min-w-0 flex-1 text-left"
                    onClick={() => {
                      selectActive(preset);
                      setExpandedId(isExpanded ? null : preset.id);
                      setSecret("");
                      setSmoke(null);
                    }}
                  >
                    <span className="flex flex-wrap items-center gap-2">
                      <span className="text-[12px] font-semibold text-slate-100">
                        {preset.label}
                      </span>
                      <span
                        className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium ${
                          configured
                            ? "bg-emerald-400/10 text-emerald-300"
                            : envVar
                              ? "bg-sky-400/10 text-sky-300"
                              : "bg-amber-400/10 text-amber-200"
                        }`}
                      >
                        <span
                          className={`size-1.5 rounded-full ${
                            configured
                              ? "bg-emerald-400"
                              : envVar
                                ? "bg-sky-400"
                                : "bg-amber-400/80"
                          }`}
                          aria-hidden
                        />
                        {configured
                          ? isActive
                            ? "Active · connected"
                            : "Connected"
                          : envVar
                            ? "Env ready"
                            : "Needs key"}
                      </span>
                    </span>
                    <span className="mt-0.5 block truncate text-[10px] text-slate-500">
                      {preset.hint ??
                        (preset.recommended
                          ? "Recommended"
                          : preset.keyless
                            ? "No key required"
                            : maskLabel(configured))}
                    </span>
                  </button>

                  {getKeyUrl && (
                    <a
                      href={getKeyUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="hidden shrink-0 text-[10px] font-medium text-slate-500 hover:text-slate-300 sm:inline"
                      onClick={(event) => event.stopPropagation()}
                    >
                      Get key ↗
                    </a>
                  )}

                  <button
                    type="button"
                    aria-expanded={isExpanded}
                    aria-label={isExpanded ? "Collapse" : "Expand"}
                    onClick={() => {
                      selectActive(preset);
                      setExpandedId(isExpanded ? null : preset.id);
                      setSecret("");
                      setSmoke(null);
                    }}
                    className={`grid size-7 shrink-0 place-items-center rounded-md text-[10px] text-slate-500 transition hover:bg-white/5 hover:text-slate-300 ${
                      isExpanded ? "rotate-90" : ""
                    }`}
                  >
                    ▸
                  </button>
                </div>

                {isExpanded && (
                  <div className="space-y-3 border-t border-white/5 bg-[#0a0e14] px-3.5 py-3 sm:px-4">
                    {preset.hint && (
                      <p className="text-[10px] leading-4 text-slate-600">
                        {preset.hint}
                      </p>
                    )}

                    {preset.custom && (
                      <div className="grid gap-2 sm:grid-cols-2">
                        <label className="block sm:col-span-2">
                          <span className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-slate-600">
                            Base URL
                          </span>
                          <input
                            value={baseUrl}
                            onChange={(event) => setBaseUrl(event.target.value)}
                            spellCheck={false}
                            placeholder="https://host/v1"
                            className="w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs text-slate-200 outline-hidden focus:border-blue-400/40"
                          />
                        </label>
                        <label className="block">
                          <span className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-slate-600">
                            Model id
                          </span>
                          <input
                            value={model}
                            onChange={(event) => setModel(event.target.value)}
                            spellCheck={false}
                            placeholder="llama3.2"
                            className="w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs text-slate-200 outline-hidden focus:border-blue-400/40"
                          />
                        </label>
                        <label className="block">
                          <span className="mb-1 flex items-center gap-1 text-[10px] font-semibold uppercase tracking-wider text-slate-600">
                            Context window
                            <Hint text="Tokens the model can see. Used by the Home context meter." />
                          </span>
                          <input
                            type="number"
                            min={1024}
                            step={1024}
                            value={contextWindow}
                            onChange={(event) =>
                              setContextWindow(event.target.value)
                            }
                            placeholder={String(DEFAULT_CONTEXT_WINDOW)}
                            className="w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs text-slate-200 outline-hidden focus:border-blue-400/40"
                          />
                          <span className="mt-1 block text-[10px] text-slate-600">
                            {formatContextTokens(activeContext)} tokens
                          </span>
                        </label>
                      </div>
                    )}

                    {!preset.custom && preset.models.length > 0 && (
                      <div className="rounded-lg border border-white/6 bg-black/20 px-2.5 py-2">
                        <div className="text-[10px] font-semibold uppercase tracking-wider text-slate-600">
                          Models · context
                        </div>
                        <ul className="mt-1.5 max-h-28 space-y-0.5 overflow-y-auto font-mono text-[10px] text-slate-400">
                          {preset.models.map((id) => (
                            <li
                              key={id}
                              className="flex items-center justify-between gap-2"
                            >
                              <span className="truncate text-slate-300">
                                {id}
                              </span>
                              <span className="shrink-0 text-slate-500">
                                {formatContextTokens(
                                  modelContextWindow(preset.id, id),
                                )}
                              </span>
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}

                    <label className="block">
                      <span className="mb-1 flex items-center justify-between gap-2 text-[10px] font-semibold uppercase tracking-wider text-slate-600">
                        <span className="inline-flex items-center gap-1">
                          API key
                          <Hint text="Stored in the OS vault. ADE never shows the secret again." />
                        </span>
                        <span
                          className={
                            configured
                              ? "normal-case text-emerald-400"
                              : "normal-case text-amber-300"
                          }
                        >
                          {configured ? "Saved" : "Needed"}
                        </span>
                      </span>
                      <input
                        type="password"
                        value={secret}
                        onChange={(event) => setSecret(event.target.value)}
                        autoComplete="new-password"
                        spellCheck={false}
                        placeholder={
                          configured
                            ? "Paste a new key to replace"
                            : "Paste key here"
                        }
                        className="w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs text-slate-200 outline-hidden focus:border-blue-400/40"
                      />
                    </label>

                    <div className="flex flex-wrap items-center gap-2">
                      <button
                        type="button"
                        onClick={() => void save(false)}
                        disabled={busy || !secret.trim()}
                        className="rounded-md bg-blue-500 px-2.5 py-1.5 text-[11px] font-semibold text-white hover:bg-blue-400 disabled:opacity-50"
                      >
                        Save key
                      </button>
                      <button
                        type="button"
                        onClick={() => void runSmoke()}
                        disabled={busy}
                        className="rounded-md border border-white/10 px-2.5 py-1.5 text-[11px] text-slate-300 hover:bg-white/5 disabled:opacity-50"
                      >
                        Check
                      </button>
                      <button
                        type="button"
                        onClick={() => void remove(preset.id)}
                        disabled={busy || !configured}
                        className="rounded-md px-2.5 py-1.5 text-[11px] text-red-300/90 hover:bg-red-400/5 disabled:opacity-40"
                      >
                        Remove
                      </button>
                      {preset.id === "opencode" && (
                        <button
                          type="button"
                          onClick={() => void importOpenCodeAuth()}
                          disabled={busy}
                          className="rounded-md px-2.5 py-1.5 text-[11px] text-slate-500 hover:text-slate-300 disabled:opacity-50"
                        >
                          Import OpenCode
                        </button>
                      )}
                      {envVar && (
                        <button
                          type="button"
                          onClick={() => void importEnvKeys(true, preset.id)}
                          disabled={busy}
                          className="rounded-md px-2.5 py-1.5 text-[11px] text-sky-300/90 hover:bg-sky-400/10 disabled:opacity-50"
                          title={`Import ${envVar} into ADE vault`}
                        >
                          Import {envVar}
                        </button>
                      )}
                      {preset.keyless && !configured && (
                        <button
                          type="button"
                          onClick={() => void activateKeyless(preset)}
                          disabled={busy}
                          className="rounded-md bg-emerald-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-emerald-200 hover:bg-emerald-500/25 disabled:opacity-50"
                        >
                          Activate
                        </button>
                      )}
                      {getKeyUrl && (
                        <a
                          href={getKeyUrl}
                          target="_blank"
                          rel="noreferrer"
                          className="ml-auto text-[10px] font-medium text-slate-500 hover:text-slate-300 sm:hidden"
                        >
                          Get key ↗
                        </a>
                      )}
                    </div>

                    {smoke && smoke.provider === preset.id && (
                      <p className="text-[11px] leading-5 text-slate-400">
                        <span className="font-semibold text-slate-300">
                          {smoke.status}
                        </span>
                        {" — "}
                        {smoke.detail}
                      </p>
                    )}

                    <Disclosure
                      title="Base URL & paid test"
                      summary={baseUrl.replace(/^https?:\/\//, "").slice(0, 28)}
                      defaultOpen={false}
                      storageKey={`ade_keys_adv_${preset.id}`}
                      className="!rounded-lg !shadow-none"
                    >
                      <div className="space-y-3 px-1 pb-1">
                        <label className="block text-[11px] text-slate-500">
                          API base URL
                          <input
                            value={baseUrl}
                            onChange={(event) => setBaseUrl(event.target.value)}
                            className="mt-1.5 w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs text-slate-200"
                          />
                        </label>

                        {!simpleMode && (
                          <div className="space-y-2 rounded-lg border border-white/6 bg-black/20 p-2.5">
                            <p className="text-[10px] font-medium text-slate-400">
                              Paid test call
                              <span className="ml-1.5 font-normal text-slate-600">
                                one short request with a cost cap
                              </span>
                            </p>
                            <div className="grid grid-cols-3 gap-2">
                              <MiniField
                                label="In $/MTok"
                                value={inputCostPerMtok}
                                onChange={setInputCostPerMtok}
                              />
                              <MiniField
                                label="Out $/MTok"
                                value={outputCostPerMtok}
                                onChange={setOutputCostPerMtok}
                              />
                              <MiniField
                                label="Max $"
                                value={maxCostUsd}
                                onChange={setMaxCostUsd}
                              />
                            </div>
                            <label className="flex items-start gap-2 text-[10px] text-slate-400">
                              <input
                                type="checkbox"
                                checked={approveLiveCost}
                                onChange={(event) =>
                                  setApproveLiveCost(event.target.checked)
                                }
                                className="mt-0.5"
                              />
                              Allow one paid request up to the max.
                            </label>
                            <button
                              type="button"
                              onClick={() => void runLiveSmoke()}
                              disabled={
                                busy ||
                                !configured ||
                                !model.trim() ||
                                !baseUrl.trim() ||
                                !inputCostPerMtok.trim() ||
                                !outputCostPerMtok.trim() ||
                                !maxCostUsd.trim() ||
                                !approveLiveCost
                              }
                              className="rounded-md border border-blue-400/30 px-2.5 py-1.5 text-[11px] font-semibold text-blue-200 disabled:opacity-40"
                            >
                              Run paid test
                            </button>
                          </div>
                        )}
                      </div>
                    </Disclosure>
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      </section>

      {message && (
        <p className="text-[11px] leading-5 text-slate-400">{message}</p>
      )}
    </div>
  );
}

function MiniField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block text-[10px] text-slate-500">
      {label}
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="mt-1 w-full rounded-md border border-white/10 bg-[#101620] px-2 py-1.5 text-[11px] text-slate-200"
      />
    </label>
  );
}
