import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/** True when running inside the Tauri desktop shell. */
export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const apiBase = import.meta.env.VITE_ADE_API_URL ?? "http://127.0.0.1:3210";

/** Read-only commands that map onto the local ADE HTTP API in browser mode. */
const httpReads: Record<string, string> = {
  get_dashboard: "/api/state",
  list_recipes: "/api/recipes",
};

async function http<T>(path: string): Promise<T> {
  const headers: Record<string, string> = {};
  const token = window.localStorage.getItem("ade_api_token");
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  let response: Response;
  try {
    response = await fetch(`${apiBase}${path}`, { headers });
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
    throw new Error(`ADE API ${path} failed: HTTP ${response.status}`);
  }
  return (await response.json()) as T;
}

/**
 * Tauri `invoke` with a browser fallback: read-only commands are served by the
 * local HTTP API; everything else needs the desktop shell.
 */
export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauri) {
    return tauriInvoke<T>(command, args);
  }
  const route = httpReads[command];
  if (route) {
    return http<T>(route);
  }
  throw new Error(
    `"${command}" requires the ADE desktop app. The browser preview is ` +
      "read-only (dashboard and recipes via the local ADE API).",
  );
}
