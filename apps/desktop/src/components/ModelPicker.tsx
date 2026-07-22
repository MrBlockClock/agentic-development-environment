import { useEffect, useMemo, useState } from "react";
import { Hint } from "./ui";
import {
  fetchProviderModels,
  presetById,
  type ProviderPreset,
  PROVIDER_PRESETS,
} from "../providers";

const selectClass =
  "w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 text-xs text-slate-200 outline-none focus:border-blue-400/40";

export function ProviderSelect({
  value,
  onChange,
  showRecommended = true,
}: {
  value: string;
  onChange: (preset: ProviderPreset) => void;
  showRecommended?: boolean;
}) {
  const recommended = PROVIDER_PRESETS.filter((preset) => preset.recommended);
  const current = presetById(value);

  return (
    <div className="space-y-2">
      {showRecommended && recommended.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {recommended.map((preset) => (
            <button
              key={preset.id}
              type="button"
              title={preset.hint}
              onClick={() => onChange(preset)}
              className={`rounded-lg border px-2.5 py-1 text-[11px] font-medium transition ${
                value === preset.id
                  ? "border-blue-400/40 bg-blue-500/20 text-blue-100"
                  : "border-white/8 bg-white/3 text-slate-400 hover:border-white/15 hover:text-slate-200"
              }`}
            >
              {preset.label}
            </button>
          ))}
        </div>
      )}
      <label className="block text-[11px] text-slate-500">
        <span className="mb-1.5 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-wider text-slate-600">
          Provider
          <Hint text="Vault id + default base URL for Agent turns." />
        </span>
        <select
          className={selectClass}
          value={PROVIDER_PRESETS.some((p) => p.id === value) ? value : ""}
          onChange={(event) => {
            const preset = presetById(event.target.value);
            if (preset) onChange(preset);
          }}
        >
          {!current && <option value="">Custom / unknown…</option>}
          {PROVIDER_PRESETS.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.label}
              {preset.recommended ? " (recommended)" : ""}
            </option>
          ))}
        </select>
      </label>
      {current?.hint && (
        <p className="text-[10px] leading-4 text-slate-500">{current.hint}</p>
      )}
    </div>
  );
}

export function ModelPicker({
  providerId,
  baseUrl,
  value,
  onChange,
  apiKey,
  allowCustom = true,
  label = "Model",
}: {
  providerId: string;
  baseUrl: string;
  value: string;
  onChange: (model: string) => void;
  /** Optional bearer for authenticated /models (e.g. key draft on Keys). */
  apiKey?: string | null;
  allowCustom?: boolean;
  label?: string;
}) {
  const fallback = presetById(providerId)?.models ?? [];
  const [models, setModels] = useState<string[]>(fallback);
  const [query, setQuery] = useState("");

  useEffect(() => {
    let cancelled = false;
    void fetchProviderModels(baseUrl, apiKey, fallback).then((result) => {
      if (cancelled) return;
      setModels(result.models);
      if (result.models.length > 0 && !result.models.includes(value) && !allowCustom) {
        onChange(result.models[0]!);
      }
    });
    return () => {
      cancelled = true;
    };
    // Re-fetch when gateway or key draft changes
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentionally omit onChange/value
  }, [baseUrl, providerId, apiKey]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return models;
    return models.filter((id) => id.toLowerCase().includes(q));
  }, [models, query]);

  const options = useMemo(() => {
    if (value && !filtered.includes(value) && !models.includes(value)) {
      return [value, ...filtered];
    }
    if (value && !filtered.includes(value) && models.includes(value)) {
      return [value, ...filtered.filter((id) => id !== value)];
    }
    return filtered;
  }, [filtered, models, value]);

  return (
    <div className="space-y-2">
      <label className="block text-[11px] text-slate-500">
        <span className="mb-1.5 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-wider text-slate-600">
          {label}
          <Hint text="Live catalog from GET /v1/models when reachable; otherwise preset list." />
        </span>
        {models.length > 12 && (
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search models…"
            className={`${selectClass} mb-1.5`}
          />
        )}
        <select
          className={selectClass}
          value={options.includes(value) ? value : value || ""}
          onChange={(event) => onChange(event.target.value)}
        >
          {options.length === 0 && <option value={value || ""}>{value || "No models"}</option>}
          {options.map((id) => (
            <option key={id} value={id}>
              {id.endsWith("-free") || id === "big-pickle" || id === "auto"
                ? `${id} · free`
                : id}
            </option>
          ))}
        </select>
      </label>
      {allowCustom && (
        <button
          type="button"
          className="text-[10px] text-blue-300/80 hover:text-blue-200"
          onClick={() => {
            const next = window.prompt("Exact model id", value);
            if (next?.trim()) onChange(next.trim());
          }}
        >
          Exact id…
        </button>
      )}
    </div>
  );
}
