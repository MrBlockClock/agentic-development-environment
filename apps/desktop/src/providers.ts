/** Shared provider presets for Keys + Agent (BYOK OpenAI-compatible gateways). */

export type ProviderPreset = {
  id: string;
  label: string;
  baseUrl: string;
  models: string[];
  hint?: string;
  recommended?: boolean;
};

export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: "opencode",
    label: "OpenCode Zen",
    baseUrl: "https://opencode.ai/zen/v1",
    models: [
      "deepseek-v4-flash-free",
      "big-pickle",
      "mimo-v2.5-free",
      "gpt-5.4-nano",
      "claude-haiku-4-5",
    ],
    hint: "Free Zen models — key from opencode.ai/auth. Claude Haiku supports images.",
    recommended: true,
  },
  {
    id: "freellm",
    label: "FreeLLMAPI",
    baseUrl: "http://127.0.0.1:31415/v1",
    models: [
      "qwen3-coder-480b",
      "glm-5.2",
      "compound",
      "auto",
      "deepseek-v4-flash",
      "big-pickle",
      "qwen2.5-vl-72b",
      "command-a-vision",
    ],
    hint: "Desktop app default :31415 — Docker is :3001 (different DB/key). *-vl* / vision models can see images.",
    recommended: true,
  },
  {
    id: "openai",
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    models: ["gpt-4.1-mini", "gpt-4.1", "o4-mini"],
  },
  {
    id: "anthropic",
    label: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    models: ["claude-sonnet-4-5", "claude-opus-4"],
  },
  {
    id: "openrouter",
    label: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    models: ["openai/gpt-4.1-mini", "anthropic/claude-sonnet-4"],
  },
];

export const DEFAULT_PROVIDER = "opencode";
export const DEFAULT_BASE_URL = "https://opencode.ai/zen/v1";
export const DEFAULT_MODEL = "deepseek-v4-flash-free";

/** Preferred vision model id per provider (must exist in that preset's list when possible). */
const PREFERRED_VISION_MODEL: Record<string, string> = {
  opencode: "claude-haiku-4-5",
  freellm: "qwen2.5-vl-72b",
  openai: "gpt-4.1-mini",
  anthropic: "claude-sonnet-4-5",
  openrouter: "openai/gpt-4.1-mini",
};

/**
 * Conservative: VL-named + common multimodal families.
 * Free text-only Zen presets return false.
 */
export function modelSupportsVision(model: string): boolean {
  const m = model.trim().toLowerCase();
  if (!m) return false;
  if (
    m === "big-pickle" ||
    m === "auto" ||
    m === "compound" ||
    m === "deepseek-v4-flash" ||
    m === "deepseek-v4-flash-free"
  ) {
    return false;
  }
  if (m.endsWith("-free") && !m.includes("vl") && !m.includes("vision")) return false;
  if (m.includes("coder") && !m.includes("vl") && !m.includes("vision")) return false;
  if (m.includes("nano") && !m.includes("vl") && !m.includes("vision")) return false;
  return (
    m.includes("vl") ||
    m.includes("vision") ||
    m.includes("moondream") ||
    m.includes("gpt-4o") ||
    m.includes("gpt-4.1") ||
    m.includes("gpt-5") ||
    m.startsWith("o4") ||
    m.includes("claude-") ||
    m.includes("gemini")
  );
}

/** First vision-capable model in a preset, preferring a known good default. */
export function visionModelInPreset(preset: ProviderPreset | undefined): string | null {
  if (!preset) return null;
  const preferred = PREFERRED_VISION_MODEL[preset.id];
  if (preferred && preset.models.some((id) => id === preferred) && modelSupportsVision(preferred)) {
    return preferred;
  }
  return preset.models.find((id) => modelSupportsVision(id)) ?? null;
}

/**
 * Best CTA target when the current model cannot see images.
 * Prefer same provider; else FreeLLM / OpenAI vision presets.
 */
export function suggestVisionTarget(providerId: string): {
  providerId: string;
  baseUrl: string;
  model: string;
  sameProvider: boolean;
} | null {
  const current = presetById(providerId);
  const same = visionModelInPreset(current);
  if (current && same) {
    return {
      providerId: current.id,
      baseUrl: current.baseUrl,
      model: same,
      sameProvider: true,
    };
  }
  for (const id of ["opencode", "freellm", "openai", "anthropic", "openrouter"] as const) {
    if (id === providerId) continue;
    const preset = presetById(id);
    const model = visionModelInPreset(preset);
    if (preset && model) {
      return {
        providerId: preset.id,
        baseUrl: preset.baseUrl,
        model,
        sameProvider: false,
      };
    }
  }
  return null;
}

export function presetById(id: string): ProviderPreset | undefined {
  return PROVIDER_PRESETS.find((preset) => preset.id === id);
}

/** Harness slot for Auto model routing (matches ADE H3 profiles). */
export type SlotKind = "planner" | "worker" | "verifier";

export function slotFromAutonomy(
  autonomy: string,
  slotOverride?: string | null,
): SlotKind {
  if (slotOverride === "verifier") return "verifier";
  if (autonomy === "act" || autonomy === "automate") return "worker";
  return "planner";
}

/**
 * Pick a model for Auto mode by slot × provider preset.
 * Prefers free/fast for planner, stronger for worker, compact for verifier.
 */
export function autoModelForSlot(
  providerId: string,
  slot: SlotKind,
): { model: string; profileId: string; label: string } {
  const preset = presetById(providerId);
  const models = preset?.models ?? [];
  const pick = (candidates: string[]) =>
    candidates.find((id) => models.includes(id)) ??
    models[0] ??
    DEFAULT_MODEL;

  switch (slot) {
    case "planner":
      return {
        model: pick([
          "deepseek-v4-flash-free",
          "gpt-5.4-nano",
          "auto",
          "gpt-4.1-mini",
          "mimo-v2.5-free",
        ]),
        profileId: "planner-fast",
        label: "Planner (fast)",
      };
    case "worker":
      return {
        model: pick([
          "big-pickle",
          "mimo-v2.5-free",
          "qwen3-coder-480b",
          "gpt-4.1",
          "claude-sonnet-4-5",
          "deepseek-v4-flash-free",
        ]),
        profileId: "worker-strong",
        label: "Worker (strong)",
      };
    case "verifier":
      return {
        model: pick([
          "gpt-5.4-nano",
          "deepseek-v4-flash-free",
          "claude-haiku-4-5",
          "gpt-4.1-mini",
          "auto",
        ]),
        profileId: "verifier-independent",
        label: "Verifier",
      };
  }
}

export function normalizeBaseUrl(raw: string): string {
  return raw.trim().replace(/\/+$/, "");
}

/**
 * Repair known-bad stored URLs (e.g. OpenCode `/api/v1` instead of Zen `/zen/v1`).
 * Zen keys 401 against `/api/v1`.
 */
export function canonicalBaseUrl(providerId: string, raw: string): string {
  const preset = presetById(providerId);
  const trimmed = normalizeBaseUrl(raw);
  if (providerId === "opencode") {
    const zen = preset?.baseUrl ?? DEFAULT_BASE_URL;
    if (!trimmed) return zen;
    if (/opencode\.ai/i.test(trimmed) && !/\/zen(\/|$)/i.test(trimmed)) {
      return zen;
    }
    return trimmed;
  }
  if (!trimmed) return preset?.baseUrl ?? DEFAULT_BASE_URL;
  return trimmed;
}

/**
 * List models from an OpenAI-compatible GET /models.
 * Falls back to preset list on failure (CORS, offline, auth).
 */
export async function fetchProviderModels(
  baseUrl: string,
  apiKey?: string | null,
  fallback: string[] = [],
): Promise<{ models: string[]; source: "api" | "preset"; detail?: string }> {
  const root = normalizeBaseUrl(baseUrl);
  if (!root) {
    return { models: fallback, source: "preset", detail: "No base URL" };
  }
  const headers: Record<string, string> = { Accept: "application/json" };
  if (apiKey?.trim()) {
    headers.Authorization = `Bearer ${apiKey.trim()}`;
  }
  try {
    const response = await fetch(`${root}/models`, {
      method: "GET",
      headers,
    });
    if (!response.ok) {
      return {
        models: fallback,
        source: "preset",
        detail: `HTTP ${response.status}`,
      };
    }
    const body = (await response.json()) as {
      data?: { id?: string }[];
      models?: { id?: string }[];
    };
    const ids = (body.data ?? body.models ?? [])
      .map((item) => item.id?.trim())
      .filter((id): id is string => Boolean(id));
    if (ids.length === 0) {
      return { models: fallback, source: "preset", detail: "Empty catalog" };
    }
    // Prefer free / auto first when present
    const ranked = [...ids].sort((a, b) => {
      const score = (id: string) =>
        id === "auto" || id.endsWith("-free") || id === "big-pickle" ? 0 : 1;
      return score(a) - score(b) || a.localeCompare(b);
    });
    return { models: ranked, source: "api" };
  } catch (error) {
    return {
      models: fallback,
      source: "preset",
      detail: error instanceof Error ? error.message : "fetch failed",
    };
  }
}
