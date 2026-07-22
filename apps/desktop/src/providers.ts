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
    hint: "Free Zen models — key from opencode.ai/auth",
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
    ],
    hint: "Desktop app default :31415 — Docker is :3001 (different DB/key). Prefer qwen/glm until FreeLLMAPI Keys has a valid OpenCode Zen upstream (auto often fails with Zen 401).",
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

export function presetById(id: string): ProviderPreset | undefined {
  return PROVIDER_PRESETS.find((preset) => preset.id === id);
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
