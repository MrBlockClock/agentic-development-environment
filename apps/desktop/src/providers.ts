/** Shared provider presets for Keys + Agent (BYOK OpenAI-compatible gateways). */

export type ProviderPreset = {
  id: string;
  label: string;
  baseUrl: string;
  models: string[];
  /** Per-model context window (tokens). Falls back to defaultContextWindow. */
  contextWindows?: Record<string, number>;
  /** Fallback context when a model id is missing from contextWindows. */
  defaultContextWindow?: number;
  hint?: string;
  /** Dogfood defaults — always pinned near the top of Keys. */
  recommended?: boolean;
  /** Free-tier / no-card catalog — shown in Keys without needing +. */
  listed?: boolean;
  /** Freeform base URL + model id + optional context override. */
  custom?: boolean;
  /** No signup key required; Activate stores a local sentinel. */
  keyless?: boolean;
  /** Console / signup URL for Get key ↗ */
  getKeyUrl?: string;
  /** Env var names the Import-from-env action looks for (first wins). */
  envKeys?: string[];
};

const K = 1024;

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
    contextWindows: {
      "deepseek-v4-flash-free": 128 * K,
      "big-pickle": 128 * K,
      "mimo-v2.5-free": 128 * K,
      "gpt-5.4-nano": 128 * K,
      "claude-haiku-4-5": 200 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Free Zen models — key from opencode.ai/auth. Claude Haiku supports images.",
    recommended: true,
    listed: true,
    getKeyUrl: "https://opencode.ai/auth",
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
    contextWindows: {
      "qwen3-coder-480b": 256 * K,
      "glm-5.2": 128 * K,
      compound: 128 * K,
      auto: 128 * K,
      "deepseek-v4-flash": 128 * K,
      "big-pickle": 128 * K,
      "qwen2.5-vl-72b": 128 * K,
      "command-a-vision": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Desktop app default :31415 — Docker is :3001 (different DB/key). *-vl* / vision models can see images.",
    recommended: true,
    listed: true,
    getKeyUrl: "http://127.0.0.1:31415/keys",
    envKeys: ["FREELMAPI_KEY", "ADE_FREELLM_API_KEY"],
  },
  {
    id: "custom",
    label: "Custom API",
    baseUrl: "http://127.0.0.1:11434/v1",
    models: [],
    defaultContextWindow: 128 * K,
    hint: "Any OpenAI-compatible base URL, API key, and model id (Ollama, vLLM, LiteLLM, corporate gateway, …).",
    listed: true,
    custom: true,
    envKeys: ["ADE_CUSTOM_API_KEY", "OPENAI_API_KEY"],
  },
  {
    id: "groq",
    label: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    models: [
      "llama-3.3-70b-versatile",
      "llama-3.1-8b-instant",
      "openai/gpt-oss-120b",
    ],
    contextWindows: {
      "llama-3.3-70b-versatile": 128 * K,
      "llama-3.1-8b-instant": 128 * K,
      "openai/gpt-oss-120b": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Fast free tier — no card. Whisper transcription also on this key.",
    listed: true,
    getKeyUrl: "https://console.groq.com/keys",
    envKeys: ["GROQ_API_KEY", "ADE_GROQ_API_KEY"],
  },
  {
    id: "cerebras",
    label: "Cerebras",
    baseUrl: "https://api.cerebras.ai/v1",
    models: ["llama-3.3-70b", "llama3.1-8b", "gpt-oss-120b"],
    contextWindows: {
      "llama-3.3-70b": 128 * K,
      "llama3.1-8b": 128 * K,
      "gpt-oss-120b": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Very fast free inference — no card for signup.",
    listed: true,
    getKeyUrl: "https://cloud.cerebras.ai",
    envKeys: ["CEREBRAS_API_KEY", "ADE_CEREBRAS_API_KEY"],
  },
  {
    id: "google",
    label: "Google AI Studio",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
    models: [
      "gemini-2.5-flash",
      "gemini-2.5-flash-lite",
      "gemini-2.0-flash",
    ],
    contextWindows: {
      "gemini-2.5-flash": 1_048_576,
      "gemini-2.5-flash-lite": 1_048_576,
      "gemini-2.0-flash": 1_048_576,
    },
    defaultContextWindow: 1_048_576,
    hint: "Generous free Gemini quota via AI Studio OpenAI-compatible endpoint.",
    listed: true,
    getKeyUrl: "https://aistudio.google.com/apikey",
    envKeys: [
      "GEMINI_API_KEY",
      "GOOGLE_API_KEY",
      "GOOGLE_AI_API_KEY",
      "ADE_GOOGLE_API_KEY",
    ],
  },
  {
    id: "openrouter",
    label: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    models: [
      "openrouter/auto",
      "deepseek/deepseek-r1:free",
      "meta-llama/llama-3.3-70b-instruct:free",
      "qwen/qwen3-coder:free",
    ],
    contextWindows: {
      "openrouter/auto": 128 * K,
      "deepseek/deepseek-r1:free": 164 * K,
      "meta-llama/llama-3.3-70b-instruct:free": 128 * K,
      "qwen/qwen3-coder:free": 256 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Many :free models. Prefer free ids to avoid paid routes.",
    listed: true,
    getKeyUrl: "https://openrouter.ai/keys",
    envKeys: ["OPENROUTER_API_KEY", "ADE_OPENROUTER_API_KEY"],
  },
  {
    id: "mistral",
    label: "Mistral",
    baseUrl: "https://api.mistral.ai/v1",
    models: ["mistral-small-latest", "codestral-latest", "open-mistral-nemo"],
    contextWindows: {
      "mistral-small-latest": 128 * K,
      "codestral-latest": 256 * K,
      "open-mistral-nemo": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Free-tier tokens for Small / Codestral / Nemo.",
    listed: true,
    getKeyUrl: "https://console.mistral.ai/api-keys",
    envKeys: ["MISTRAL_API_KEY", "ADE_MISTRAL_API_KEY"],
  },
  {
    id: "github",
    label: "GitHub Models",
    baseUrl: "https://models.github.ai/inference",
    models: [
      "openai/gpt-4.1-mini",
      "openai/gpt-4.1",
      "meta/llama-3.3-70b-instruct",
    ],
    contextWindows: {
      "openai/gpt-4.1-mini": 1_048_576,
      "openai/gpt-4.1": 1_048_576,
      "meta/llama-3.3-70b-instruct": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Use a GitHub PAT (classic; Models works with no scopes).",
    listed: true,
    getKeyUrl: "https://github.com/settings/tokens",
    envKeys: ["GITHUB_TOKEN", "GITHUB_MODELS_TOKEN", "ADE_GITHUB_API_KEY"],
  },
  {
    id: "cohere",
    label: "Cohere",
    baseUrl: "https://api.cohere.ai/compatibility/v1",
    models: ["command-a-03-2025", "command-r-plus", "command-r"],
    contextWindows: {
      "command-a-03-2025": 256 * K,
      "command-r-plus": 128 * K,
      "command-r": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Trial keys via Cohere dashboard — OpenAI-compatible path.",
    listed: true,
    getKeyUrl: "https://dashboard.cohere.com/api-keys",
    envKeys: ["COHERE_API_KEY", "ADE_COHERE_API_KEY"],
  },
  {
    id: "nvidia",
    label: "NVIDIA NIM",
    baseUrl: "https://integrate.api.nvidia.com/v1",
    models: [
      "meta/llama-3.3-70b-instruct",
      "nvidia/llama-3.1-nemotron-70b-instruct",
      "deepseek-ai/deepseek-r1",
    ],
    contextWindows: {
      "meta/llama-3.3-70b-instruct": 128 * K,
      "nvidia/llama-3.1-nemotron-70b-instruct": 128 * K,
      "deepseek-ai/deepseek-r1": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Free credits / generous catalog at build.nvidia.com.",
    listed: true,
    getKeyUrl: "https://build.nvidia.com/",
    envKeys: ["NVIDIA_API_KEY", "ADE_NVIDIA_API_KEY"],
  },
  {
    id: "sambanova",
    label: "SambaNova",
    baseUrl: "https://api.sambanova.ai/v1",
    models: ["Meta-Llama-3.3-70B-Instruct", "DeepSeek-R1"],
    contextWindows: {
      "Meta-Llama-3.3-70B-Instruct": 128 * K,
      "DeepSeek-R1": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Free cloud tier — OpenAI-compatible.",
    listed: true,
    getKeyUrl: "https://cloud.sambanova.ai/",
    envKeys: ["SAMBANOVA_API_KEY", "ADE_SAMBANOVA_API_KEY"],
  },
  {
    id: "deepseek",
    label: "DeepSeek",
    baseUrl: "https://api.deepseek.com/v1",
    models: ["deepseek-chat", "deepseek-reasoner"],
    contextWindows: {
      "deepseek-chat": 128 * K,
      "deepseek-reasoner": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Often has free/cheap credits; OpenAI-compatible.",
    listed: true,
    getKeyUrl: "https://platform.deepseek.com/api_keys",
    envKeys: ["DEEPSEEK_API_KEY", "ADE_DEEPSEEK_API_KEY"],
  },
  {
    id: "together",
    label: "Together AI",
    baseUrl: "https://api.together.xyz/v1",
    models: [
      "meta-llama/Llama-3.3-70B-Instruct-Turbo",
      "Qwen/Qwen2.5-Coder-32B-Instruct",
    ],
    contextWindows: {
      "meta-llama/Llama-3.3-70B-Instruct-Turbo": 128 * K,
      "Qwen/Qwen2.5-Coder-32B-Instruct": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Free starter credits on signup.",
    listed: true,
    getKeyUrl: "https://api.together.xyz/settings/api-keys",
    envKeys: ["TOGETHER_API_KEY", "ADE_TOGETHER_API_KEY"],
  },
  {
    id: "fireworks",
    label: "Fireworks",
    baseUrl: "https://api.fireworks.ai/inference/v1",
    models: [
      "accounts/fireworks/models/llama-v3p3-70b-instruct",
      "accounts/fireworks/models/deepseek-r1",
    ],
    contextWindows: {
      "accounts/fireworks/models/llama-v3p3-70b-instruct": 128 * K,
      "accounts/fireworks/models/deepseek-r1": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Free trial credits — OpenAI-compatible inference API.",
    listed: true,
    getKeyUrl: "https://fireworks.ai/account/api-keys",
    envKeys: ["FIREWORKS_API_KEY", "ADE_FIREWORKS_API_KEY"],
  },
  {
    id: "huggingface",
    label: "Hugging Face",
    baseUrl: "https://router.huggingface.co/v1",
    models: [
      "meta-llama/Llama-3.3-70B-Instruct",
      "Qwen/Qwen2.5-72B-Instruct",
    ],
    contextWindows: {
      "meta-llama/Llama-3.3-70B-Instruct": 128 * K,
      "Qwen/Qwen2.5-72B-Instruct": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "HF token as Bearer against the OpenAI-compatible router.",
    listed: true,
    getKeyUrl: "https://huggingface.co/settings/tokens",
    envKeys: ["HF_TOKEN", "HUGGINGFACE_API_KEY", "ADE_HUGGINGFACE_API_KEY"],
  },
  {
    id: "zhipu",
    label: "Z.ai / GLM",
    baseUrl: "https://api.z.ai/api/paas/v4",
    models: ["glm-4.5-flash", "glm-4-flash"],
    contextWindows: {
      "glm-4.5-flash": 128 * K,
      "glm-4-flash": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Free GLM flash models — endpoint may vary by region (z.ai / bigmodel.cn).",
    listed: true,
    getKeyUrl: "https://z.ai/",
    envKeys: ["ZHIPU_API_KEY", "ZAI_API_KEY", "ADE_ZHIPU_API_KEY"],
  },
  {
    id: "ollama-cloud",
    label: "Ollama Cloud",
    baseUrl: "https://ollama.com/v1",
    models: ["llama3.2", "qwen2.5-coder", "deepseek-r1"],
    contextWindows: {
      "llama3.2": 128 * K,
      "qwen2.5-coder": 128 * K,
      "deepseek-r1": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Ollama cloud free tier — create a key at ollama.com.",
    listed: true,
    getKeyUrl: "https://ollama.com/settings/keys",
    envKeys: ["OLLAMA_API_KEY", "ADE_OLLAMA_CLOUD_API_KEY"],
  },
  {
    id: "cloudflare",
    label: "Cloudflare Workers AI",
    baseUrl:
      "https://api.cloudflare.com/client/v4/accounts/YOUR_ACCOUNT_ID/ai/v1",
    models: [
      "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
      "@cf/meta/llama-3.1-8b-instruct",
    ],
    contextWindows: {
      "@cf/meta/llama-3.3-70b-instruct-fp8-fast": 128 * K,
      "@cf/meta/llama-3.1-8b-instruct": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Needs Account ID in the base URL + Workers AI API token. Replace YOUR_ACCOUNT_ID.",
    listed: true,
    getKeyUrl: "https://dash.cloudflare.com/?to=/:account/ai/workers-ai",
    envKeys: ["CLOUDFLARE_API_TOKEN", "ADE_CLOUDFLARE_API_KEY"],
  },
  {
    id: "llm7",
    label: "LLM7",
    baseUrl: "https://api.llm7.io/v1",
    models: ["gpt-4o-mini-2024-07-18", "llama-3.3-70b"],
    contextWindows: {
      "gpt-4o-mini-2024-07-18": 128 * K,
      "llama-3.3-70b": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Works without a key for light use; optional token raises limits.",
    listed: true,
    keyless: true,
    getKeyUrl: "https://llm7.io/",
    envKeys: ["LLM7_API_KEY", "ADE_LLM7_API_KEY"],
  },
  {
    id: "pollinations",
    label: "Pollinations",
    baseUrl: "https://text.pollinations.ai/openai",
    models: ["openai", "mistral", "qwen"],
    contextWindows: {
      openai: 128 * K,
      mistral: 128 * K,
      qwen: 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Anonymous OpenAI-compatible text API — no signup. Not for confidential prompts.",
    listed: true,
    keyless: true,
    getKeyUrl: "https://pollinations.ai/",
  },
  {
    id: "kilo",
    label: "Kilo Gateway",
    baseUrl: "https://api.kilo.ai/v1",
    models: ["kilo/auto"],
    contextWindows: { "kilo/auto": 128 * K },
    defaultContextWindow: 128 * K,
    hint: "Keyless free aggregator (~200 req/hour/IP). May log prompts — avoid secrets.",
    listed: true,
    keyless: true,
    getKeyUrl: "https://kilo.ai/",
  },
  {
    id: "ovh",
    label: "OVHcloud AI Endpoints",
    baseUrl: "https://oai.endpoints.kepler.ai.cloud.ovh.net/v1",
    models: ["Meta-Llama-3_3-70B-Instruct", "Mistral-Nemo-Instruct-2407"],
    contextWindows: {
      "Meta-Llama-3_3-70B-Instruct": 128 * K,
      "Mistral-Nemo-Instruct-2407": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "Anonymous OpenAI-compatible endpoints (FreeLLMAPI premade). Region/URL can vary.",
    listed: true,
    keyless: true,
    getKeyUrl: "https://endpoints.ai.cloud.ovh.net/",
  },
  {
    id: "agnes",
    label: "Agnes AI",
    baseUrl: "https://api.agnes-ai.com/v1",
    models: ["agnes-default"],
    contextWindows: { "agnes-default": 128 * K },
    defaultContextWindow: 128 * K,
    hint: "FreeLLMAPI premade — grab a free key from their console if required.",
    listed: true,
    getKeyUrl: "https://agnes-ai.com/",
    envKeys: ["AGNES_API_KEY", "ADE_AGNES_API_KEY"],
  },
  {
    id: "reka",
    label: "Reka",
    baseUrl: "https://api.reka.ai/v1",
    models: ["reka-flash", "reka-core"],
    contextWindows: {
      "reka-flash": 128 * K,
      "reka-core": 128 * K,
    },
    defaultContextWindow: 128 * K,
    hint: "FreeLLMAPI premade — Reka OpenAI-compatible chat.",
    listed: true,
    getKeyUrl: "https://platform.reka.ai/",
    envKeys: ["REKA_API_KEY", "ADE_REKA_API_KEY"],
  },
  {
    id: "openai",
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    models: ["gpt-4.1-mini", "gpt-4.1", "o4-mini"],
    contextWindows: {
      "gpt-4.1-mini": 1_048_576,
      "gpt-4.1": 1_048_576,
      "o4-mini": 200 * K,
    },
    defaultContextWindow: 128 * K,
    getKeyUrl: "https://platform.openai.com/api-keys",
    envKeys: ["OPENAI_API_KEY", "ADE_OPENAI_API_KEY"],
  },
  {
    id: "anthropic",
    label: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    models: ["claude-sonnet-4-5", "claude-opus-4"],
    contextWindows: {
      "claude-sonnet-4-5": 200 * K,
      "claude-opus-4": 200 * K,
    },
    defaultContextWindow: 200 * K,
    hint: "Native Anthropic API — ADE expects OpenAI-compatible gateways; prefer FreeLLMAPI/OpenRouter for Claude.",
    getKeyUrl: "https://console.anthropic.com/settings/keys",
    envKeys: ["ANTHROPIC_API_KEY", "ADE_ANTHROPIC_API_KEY"],
  },
];

export const DEFAULT_PROVIDER = "opencode";
export const DEFAULT_BASE_URL = "https://opencode.ai/zen/v1";
export const DEFAULT_MODEL = "deepseek-v4-flash-free";
export const DEFAULT_CONTEXT_WINDOW = 128 * K;

/** Preferred vision model id per provider (must exist in that preset's list when possible). */
const PREFERRED_VISION_MODEL: Record<string, string> = {
  opencode: "claude-haiku-4-5",
  freellm: "qwen2.5-vl-72b",
  google: "gemini-2.5-flash",
  openai: "gpt-4.1-mini",
  anthropic: "claude-sonnet-4-5",
  openrouter: "openrouter/auto",
};

export function firstModelId(preset: ProviderPreset): string {
  return preset.models[0] ?? (preset.custom ? "custom-model" : DEFAULT_MODEL);
}

/** Compact label for context meters / dropdowns (e.g. 128k, 1M). */
export function formatContextTokens(tokens: number): string {
  if (!Number.isFinite(tokens) || tokens <= 0) return "?";
  if (tokens >= 1_000_000) {
    const m = tokens / 1_000_000;
    return Number.isInteger(m) ? `${m}M` : `${m.toFixed(1)}M`;
  }
  if (tokens >= 1024) {
    const k = tokens / 1024;
    return Number.isInteger(k) ? `${k}k` : `${Math.round(k)}k`;
  }
  return String(Math.round(tokens));
}

/**
 * Resolve context window for a provider/model.
 * `override` (e.g. Custom API localStorage) wins when > 0.
 */
export function modelContextWindow(
  providerId: string,
  modelId: string,
  override?: number | null,
): number {
  if (override && override > 0) return Math.floor(override);
  const preset = presetById(providerId);
  const id = modelId.trim();
  if (preset?.contextWindows?.[id]) return preset.contextWindows[id]!;
  if (preset?.defaultContextWindow) return preset.defaultContextWindow;
  return guessContextWindow(id);
}

function guessContextWindow(modelId: string): number {
  const m = modelId.toLowerCase();
  if (m.includes("gemini")) return 1_048_576;
  if (m.includes("gpt-4.1") || m.includes("gpt-4o")) return 1_048_576;
  if (m.includes("claude")) return 200 * K;
  if (m.includes("o4") || m.includes("o3")) return 200 * K;
  if (m.includes("codestral") || m.includes("qwen3-coder")) return 256 * K;
  if (m.includes("command-a")) return 256 * K;
  return DEFAULT_CONTEXT_WINDOW;
}

export function modelOptionLabel(
  providerId: string,
  modelId: string,
  override?: number | null,
): string {
  const ctx = modelContextWindow(providerId, modelId, override);
  return `${modelId} · ${formatContextTokens(ctx)}`;
}

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
  for (const id of ["opencode", "freellm", "google", "openai", "openrouter"] as const) {
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
    (preset?.custom ? "custom-model" : DEFAULT_MODEL);

  switch (slot) {
    case "planner":
      return {
        model: pick([
          "deepseek-v4-flash-free",
          "gpt-5.4-nano",
          "auto",
          "llama-3.1-8b-instant",
          "gemini-2.5-flash-lite",
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
          "llama-3.3-70b-versatile",
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
          "gemini-2.5-flash-lite",
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
  if (providerId === "custom") {
    return trimmed || preset?.baseUrl || "http://127.0.0.1:11434/v1";
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
