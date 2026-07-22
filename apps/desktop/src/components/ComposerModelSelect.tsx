import { useEffect, useMemo, useState } from "react";
import {
  fetchProviderModels,
  presetById,
  PROVIDER_PRESETS,
  type ProviderPreset,
} from "../providers";
import { DarkSelect } from "./DarkSelect";

const SEP = "::";

function encodeChoice(providerId: string, modelId: string): string {
  return `${providerId}${SEP}${modelId}`;
}

function decodeChoice(value: string): { providerId: string; modelId: string } | null {
  const idx = value.indexOf(SEP);
  if (idx <= 0) return null;
  return {
    providerId: value.slice(0, idx),
    modelId: value.slice(idx + SEP.length),
  };
}

/** One composer control: pick a model, grouped by provider. */
export function ComposerModelSelect({
  providerId,
  baseUrl,
  model,
  onProviderChange,
  onModelChange,
}: {
  providerId: string;
  baseUrl: string;
  model: string;
  onProviderChange: (preset: ProviderPreset) => void;
  onModelChange: (model: string) => void;
}) {
  const [liveModels, setLiveModels] = useState<string[]>(
    () => presetById(providerId)?.models ?? [],
  );

  useEffect(() => {
    let cancelled = false;
    const presetModels = presetById(providerId)?.models ?? [];
    void fetchProviderModels(baseUrl, null, presetModels).then((result) => {
      if (cancelled) return;
      setLiveModels(result.models.length > 0 ? result.models : presetModels);
    });
    return () => {
      cancelled = true;
    };
  }, [baseUrl, providerId]);

  const options = useMemo(() => {
    const out: { value: string; label: string; group: string }[] = [];
    const seen = new Set<string>();

    for (const preset of PROVIDER_PRESETS) {
      const models =
        preset.id === providerId
          ? mergeModels(liveModels, preset.models, model)
          : preset.models;

      for (const id of models.slice(0, 24)) {
        const value = encodeChoice(preset.id, id);
        if (seen.has(value)) continue;
        seen.add(value);
        out.push({
          value,
          label: id,
          group: preset.label,
        });
      }
    }

    // Keep an unknown custom selection visible.
    const current = encodeChoice(providerId, model);
    if (model && !seen.has(current)) {
      const label =
        PROVIDER_PRESETS.find((p) => p.id === providerId)?.label ?? providerId;
      out.unshift({
        value: current,
        label: model,
        group: label || "Current",
      });
    }

    return out;
  }, [liveModels, model, providerId]);

  const value = encodeChoice(providerId, model);
  const providerLabel =
    PROVIDER_PRESETS.find((p) => p.id === providerId)?.label ?? providerId;

  return (
    <DarkSelect
      ariaLabel="Provider and model"
      value={value}
      options={options}
      title={`${providerLabel} · ${model}`}
      maxLabelChars={22}
      onChange={(next) => {
        const parsed = decodeChoice(next);
        if (!parsed) return;
        const preset = presetById(parsed.providerId);
        if (preset && parsed.providerId !== providerId) {
          onProviderChange(preset);
        }
        if (parsed.modelId !== model || parsed.providerId !== providerId) {
          onModelChange(parsed.modelId);
        }
      }}
    />
  );
}

function mergeModels(
  primary: string[],
  secondary: string[],
  current: string,
): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const id of [current, ...primary, ...secondary]) {
    if (!id || seen.has(id)) continue;
    seen.add(id);
    out.push(id);
  }
  return out;
}
