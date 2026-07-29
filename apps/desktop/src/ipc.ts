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

export class AdeApiError extends Error {
  readonly kind: "offline" | "auth" | "http" | "desktop_only";

  constructor(
    kind: AdeApiError["kind"],
    message: string,
  ) {
    super(message);
    this.name = "AdeApiError";
    this.kind = kind;
  }
}

export type BrowserApiProbe = {
  reachable: boolean;
  apiOk: boolean;
  authRequired: boolean | null;
  detail: string;
};

export async function probeBrowserApi(): Promise<BrowserApiProbe> {
  try {
    const health = await fetch(`${apiBase}/health/live`, { method: "GET" });
    if (!health.ok) {
      return {
        reachable: false,
        apiOk: false,
        authRequired: null,
        detail: `Health check failed (${health.status}).`,
      };
    }
  } catch {
    return {
      reachable: false,
      apiOk: false,
      authRequired: null,
      detail: `Cannot reach ${apiBase}.`,
    };
  }

  try {
    await http("/api/state");
    return {
      reachable: true,
      apiOk: true,
      authRequired: false,
      detail: "Connected to local ADE API.",
    };
  } catch (reason) {
    if (reason instanceof AdeApiError && reason.kind === "auth") {
      return {
        reachable: true,
        apiOk: false,
        authRequired: true,
        detail: reason.message,
      };
    }
    return {
      reachable: true,
      apiOk: false,
      authRequired: null,
      detail:
        reason instanceof Error
          ? reason.message
          : "Health ok but /api/state failed — check CORS or serve logs.",
    };
  }
}

/** Read-only / coordination commands that map onto the local ADE HTTP API. */
const httpReads: Record<string, string> = {
  get_dashboard: "/api/state",
  list_recipes: "/api/recipes",
  list_rules: "/api/rules",
  list_skills: "/api/skills",
  list_guidance_profiles: "/api/guidance/profiles",
  get_active_guidance_profile: "/api/guidance/active-profile",
  run_global_audit: "/api/guidance/global-audit",
  guided_wins_status: "/api/guided/wins",
  list_workspaces: "/api/workspaces/list",
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
 * Tauri `invoke` with a browser fallback: coordination + analytics + guided
 * wins go to the local HTTP API; MCP list returns empty; agent/vault/PTY stay Desktop.
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
  if (command === "preview_recipe_scaffold") {
    return http<T>("/api/recipes/preview", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        recipe: args?.recipe,
        project_name: args?.projectName ?? args?.project_name ?? null,
        force: Boolean(args?.force),
      }),
    });
  }
  if (command === "guided_mark_win") {
    return http<T>("/api/guided/wins", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ win: String(args?.win ?? "") }),
    });
  }
  if (command === "guided_understand_project") {
    return http<T>("/api/guided/understand", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: "{}",
    });
  }
  if (command === "spend_ledger_recent") {
    const limit = Number(args?.limit ?? 40);
    return http<T>(`/api/spend/ledger?limit=${encodeURIComponent(String(limit))}`);
  }
  if (command === "spend_summary") {
    const params = new URLSearchParams();
    const session = args?.sessionCapUsd ?? args?.session_cap_usd;
    const daily = args?.dailyCapUsd ?? args?.daily_cap_usd;
    if (session != null) params.set("session_cap_usd", String(session));
    if (daily != null) params.set("daily_cap_usd", String(daily));
    const qs = params.toString();
    return http<T>(`/api/spend/summary${qs ? `?${qs}` : ""}`);
  }
  const route = httpReads[command];
  if (route) {
    return http<T>(route);
  }
  throw new AdeApiError(
    "desktop_only",
    `"${command}" requires the ADE desktop app. Browser preview supports ` +
      "dashboard, recipes, verify, guidance, guided wins, analytics, and workspace list via `ade serve`. " +
      "Keys vault, MCP host, Agent turns, Editor, Terminal, and EXECUTE stay on Desktop.",
  );
}
