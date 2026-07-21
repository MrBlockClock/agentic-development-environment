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

const apiBase = import.meta.env.VITE_ADE_API_URL ?? "http://127.0.0.1:3210";

/** Browser preview bearer: localStorage, then VITE_ADE_API_TOKEN (must match ADE_API_TOKEN). */
function browserApiToken(): string | null {
  const fromStorage = window.localStorage.getItem("ade_api_token")?.trim();
  if (fromStorage) return fromStorage;
  const fromEnv = import.meta.env.VITE_ADE_API_TOKEN?.trim();
  if (fromEnv) return fromEnv;
  return null;
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
  const token = browserApiToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  let response: Response;
  try {
    response = await fetch(`${apiBase}${path}`, { ...init, headers });
  } catch {
    throw new Error(
      `Browser mode: cannot reach the local ADE API at ${apiBase}. ` +
        "Start it with `ade serve` (or open ADE in the desktop app).",
    );
  }
  if (response.status === 401) {
    throw new Error(
      "Browser mode: the local ADE API rejected the bearer token. " +
        "Set localStorage ade_api_token (or VITE_ADE_API_TOKEN) to match ADE_API_TOKEN on `ade serve`.",
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
    throw new Error(`ADE API ${path} failed: ${detail}`);
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
  throw new Error(
    `"${command}" requires the ADE desktop app. Browser preview supports ` +
      "dashboard/recipes/verify/rules/skills via the local API; MCP connect and agent turns need Tauri.",
  );
}
