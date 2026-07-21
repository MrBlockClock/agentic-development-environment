import {
  invoke as tauriInvoke,
  isTauri as tauriIsTauri,
} from "@tauri-apps/api/core";

/**
 * Detect the Tauri shell at call time (not module load).
 * A load-time const can race Tauri injection and permanently fall back to
 * browser HTTP — which fails unless `ade serve` is running.
 */
export function isTauri(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    if (tauriIsTauri()) {
      return true;
    }
  } catch {
    // fall through to globals
  }
  const win = window as Window & {
    isTauri?: boolean;
    __TAURI_INTERNALS__?: unknown;
  };
  return Boolean(win.isTauri) || "__TAURI_INTERNALS__" in win;
}

const API_TOKEN_STORAGE_KEY = "ade_api_token";

const apiBase = import.meta.env.VITE_ADE_API_URL ?? "http://127.0.0.1:3210";

export function browserApiBase(): string {
  return apiBase;
}

/** Browser preview bearer: localStorage, then VITE_ADE_API_TOKEN (must match ADE_API_TOKEN). */
export function getBrowserApiToken(): string | null {
  if (typeof window === "undefined") return null;
  const fromStorage = window.localStorage.getItem(API_TOKEN_STORAGE_KEY)?.trim();
  if (fromStorage) return fromStorage;
  const fromEnv = import.meta.env.VITE_ADE_API_TOKEN?.trim();
  if (fromEnv) return fromEnv;
  return null;
}

export function hasStoredBrowserApiToken(): boolean {
  if (typeof window === "undefined") return false;
  return Boolean(window.localStorage.getItem(API_TOKEN_STORAGE_KEY)?.trim());
}

export function setBrowserApiToken(token: string): void {
  window.localStorage.setItem(API_TOKEN_STORAGE_KEY, token.trim());
}

export function clearBrowserApiToken(): void {
  window.localStorage.removeItem(API_TOKEN_STORAGE_KEY);
}

export type AdeApiErrorKind = "offline" | "auth" | "http" | "desktop_only";

export class AdeApiError extends Error {
  readonly kind: AdeApiErrorKind;

  constructor(kind: AdeApiErrorKind, message: string) {
    super(message);
    this.name = "AdeApiError";
    this.kind = kind;
  }

  static is(value: unknown): value is AdeApiError {
    return value instanceof AdeApiError;
  }
}

export type BrowserApiProbe = {
  reachable: boolean;
  apiOk: boolean;
  /** True when /api rejected with 401 (token missing or wrong). */
  authRequired: boolean | null;
  detail: string;
};

/**
 * Probe loopback health (public) then a coordination read (/api/state).
 * Does not invent tokens — uses saved/env bearer only.
 */
export async function probeBrowserApi(): Promise<BrowserApiProbe> {
  try {
    const health = await fetch(`${apiBase}/health/live`, { method: "GET" });
    if (!health.ok) {
      return {
        reachable: false,
        apiOk: false,
        authRequired: null,
        detail: `Health check failed (HTTP ${health.status}).`,
      };
    }
  } catch {
    return {
      reachable: false,
      apiOk: false,
      authRequired: null,
      detail: `Cannot reach ${apiBase}. Start with \`ade serve --bind 127.0.0.1:3210\`.`,
    };
  }

  const token = getBrowserApiToken();
  const headers: Record<string, string> = {};
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  try {
    const response = await fetch(`${apiBase}/api/state`, { headers });
    if (response.status === 401) {
      return {
        reachable: true,
        apiOk: false,
        authRequired: true,
        detail: token
          ? "Bearer token rejected — it must match ADE_API_TOKEN on `ade serve`."
          : "API requires a bearer token. Save ADE_API_TOKEN below (or set VITE_ADE_API_TOKEN).",
      };
    }
    if (!response.ok) {
      return {
        reachable: true,
        apiOk: false,
        authRequired: false,
        detail: `API /state returned HTTP ${response.status}.`,
      };
    }
    return {
      reachable: true,
      apiOk: true,
      authRequired: false,
      detail: token ? "Authorized against local ADE API." : "Local API reachable (no token set).",
    };
  } catch {
    return {
      reachable: true,
      apiOk: false,
      authRequired: null,
      detail: "Health ok but /api/state failed — check CORS or serve logs.",
    };
  }
}

/** Read-only commands that map onto the local ADE HTTP API in browser mode. */
const httpReads: Record<string, string> = {
  get_dashboard: "/api/state",
  list_recipes: "/api/recipes",
  list_rules: "/api/rules",
  list_skills: "/api/skills",
  list_guidance_profiles: "/api/guidance/profiles",
  get_active_guidance_profile: "/api/guidance/active-profile",
  run_global_audit: "/api/guidance/global-audit",
};

/**
 * MCP connections live in the desktop process, not the loopback API.
 * Browser preview reports an empty registry so the MCP panel can load.
 */
const browserEmptyReads = new Set(["mcp_list_servers", "mcp_list_tools"]);

async function http<T>(path: string, init?: RequestInit): Promise<T> {
  const headers: Record<string, string> = {
    ...(init?.headers as Record<string, string> | undefined),
  };
  const token = getBrowserApiToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  let response: Response;
  try {
    response = await fetch(`${apiBase}${path}`, { ...init, headers });
  } catch {
    throw new AdeApiError(
      "offline",
      `Browser mode: cannot reach the local ADE API at ${apiBase}. ` +
        "Start it with `ade serve` (or open ADE in the desktop app).",
    );
  }
  if (response.status === 401) {
    throw new AdeApiError(
      "auth",
      token
        ? "Browser mode: the local ADE API rejected the bearer token. " +
            "Update the Local API token panel so it matches ADE_API_TOKEN on `ade serve` " +
            "(or set VITE_ADE_API_TOKEN at build time). Agent turns stay Desktop-only."
        : "Browser mode: ADE API requires ADE_API_TOKEN. Open the Local API token panel, " +
            "paste the same value used by `ade serve`, then retry. " +
            "Chat and provider Keys still need the Desktop app.",
    );
  }
  if (!response.ok) {
    let detail = `HTTP ${response.status}`;
    try {
      const body = (await response.json()) as { error?: { message?: string } };
      if (body.error?.message) {
        detail = body.error.message;
      }
    } catch {
      // keep status detail
    }
    throw new AdeApiError("http", `ADE API ${path} failed: ${detail}`);
  }
  return (await response.json()) as T;
}

/**
 * Tauri `invoke` with a browser fallback: coordination reads + verify go to the
 * local HTTP API; MCP list returns empty; mutations need desktop.
 */
export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauri()) {
    return tauriInvoke<T>(command, args);
  }
  if (browserEmptyReads.has(command)) {
    return [] as T;
  }
  if (command === "run_verify") {
    return http<T>("/api/verify", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        gate: String(args?.gate ?? "G0"),
        through: Boolean(args?.through),
      }),
    });
  }
  if (command === "set_active_guidance_profile") {
    return http<T>("/api/guidance/active-profile", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id: args?.id ?? null }),
    });
  }
  if (command === "rank_recipes") {
    return http<T>("/api/recipes/fit", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(args?.answers ?? args ?? {}),
    });
  }
  const route = httpReads[command];
  if (route) {
    return http<T>(route);
  }
  throw new AdeApiError(
    "desktop_only",
    `"${command}" requires the ADE desktop app. Browser preview supports ` +
      "dashboard/recipes/verify/rules/skills via the local API; MCP connect and agent turns need Tauri. " +
      "EXECUTE is not available over HTTP by design.",
  );
}
