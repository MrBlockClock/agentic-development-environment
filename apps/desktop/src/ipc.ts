import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/** True when running inside the Tauri desktop shell. */
export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const apiBase = import.meta.env.VITE_ADE_API_URL ?? "http://127.0.0.1:3210";

/** Read-only commands that map onto the local ADE HTTP API in browser mode. */
const httpReads: Record<string, string> = {
  get_dashboard: "/api/state",
  list_recipes: "/api/recipes",
  list_rules: "/api/rules",
  list_skills: "/api/skills",
};

/**
 * MCP connections live in the desktop process, not the loopback API.
 * Browser preview reports an empty registry so the MCP panel can load.
 */
const browserEmptyReads = new Set(["mcp_list_servers", "mcp_list_tools"]);

async function http<T>(
  path: string,
  init?: RequestInit,
): Promise<T> {
  const headers: Record<string, string> = {
    ...(init?.headers as Record<string, string> | undefined),
  };
  const token = window.localStorage.getItem("ade_api_token");
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
      "Browser mode: the local ADE API requires a bearer token. " +
        'Set it via localStorage.setItem("ade_api_token", "<token>").',
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
  if (isTauri) {
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
  const route = httpReads[command];
  if (route) {
    return http<T>(route);
  }
  throw new Error(
    `"${command}" requires the ADE desktop app. Browser preview supports ` +
      "dashboard/recipes/verify/rules/skills via the local API; MCP connect and agent turns need Tauri.",
  );
}
