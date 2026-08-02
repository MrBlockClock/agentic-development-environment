import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Page } from "@playwright/test";

/**
 * Desktop-only surfaces (Analytics reads the usage ledger over IPC) cannot be
 * exercised against `vite preview`. Rather than leave them uncovered, install a
 * `__TAURI_INTERNALS__` shim so the app takes its Tauri code path and answers
 * from fixtures. `dashboard.json` is a captured `/api/state` response, so the
 * shape stays honest to the Rust serializers.
 */

const dashboard = JSON.parse(
  readFileSync(
    fileURLToPath(new URL("./dashboard.json", import.meta.url)),
    "utf8",
  ),
) as Record<string, unknown>;

export type LedgerRow = {
  id: string;
  created_at: string;
  status: string;
  scope: string;
  period_key: string;
  provider: string | null;
  model: string | null;
  reserved_usd: number;
  actual_usd: number;
  input_tokens: number;
  output_tokens: number;
};

/**
 * Deterministic ledger anchored to `now` so day windows and the trend chart
 * have something to bucket. Covers the cases Analytics must not blur together:
 * settled rows that came in under reserve, an open reserve, and a settled row
 * priced at $0 while a reserve was taken (the H1 detector).
 */
export function ledgerFixture(now = new Date()): LedgerRow[] {
  const at = (daysAgo: number, hour: number): string => {
    const date = new Date(now);
    date.setDate(date.getDate() - daysAgo);
    date.setHours(hour, 0, 0, 0);
    return date.toISOString();
  };

  const row = (
    id: string,
    daysAgo: number,
    hour: number,
    provider: string,
    model: string,
    status: string,
    reserved: number,
    actual: number,
    tokensIn: number,
    tokensOut: number,
  ): LedgerRow => ({
    id,
    created_at: at(daysAgo, hour),
    status,
    scope: "workspace",
    period_key: "day",
    provider,
    model,
    reserved_usd: reserved,
    actual_usd: actual,
    input_tokens: tokensIn,
    output_tokens: tokensOut,
  });

  return [
    row("l1", 0, 9, "anthropic", "claude-sonnet-4", "committed", 0.4, 0.28, 18_400, 3_100),
    row("l2", 0, 11, "anthropic", "claude-sonnet-4", "committed", 0.4, 0.31, 21_050, 4_260),
    row("l3", 0, 13, "openai", "gpt-5", "committed", 0.25, 0.19, 9_800, 1_450),
    row("l4", 0, 15, "openai", "gpt-5", "reserved", 0.25, 0, 0, 0),
    // Settled at $0 while a reserve was held — must surface, not vanish.
    row("l5", 1, 10, "google", "gemini-2.5-pro", "committed", 0.18, 0, 7_200, 900),
    row("l6", 1, 14, "anthropic", "claude-opus-4", "committed", 1.2, 0.94, 33_700, 6_800),
    row("l7", 3, 9, "anthropic", "claude-sonnet-4", "committed", 0.4, 0.36, 24_900, 5_050),
    row("l8", 6, 16, "openai", "gpt-5", "committed", 0.25, 0.22, 11_300, 2_400),
    // Outside the 7-day window; only "30 days" and "All" should count it.
    row("l9", 12, 12, "anthropic", "claude-opus-4", "committed", 1.2, 1.05, 41_000, 7_900),
  ];
}

export const spendSummaryFixture = {
  daily_usd: 1.03,
  used_usd: 0.78,
  reserved_usd: 0.25,
  remaining_usd: 8.97,
  daily_cap_usd: 10,
  session_cap_usd: 1,
  period_key: "day",
};

/** Route the app onto its Tauri path with fixture answers. */
export async function installTauriStub(page: Page): Promise<void> {
  await page.addInitScript(
    ([state, ledger, summary]) => {
      const answers: Record<string, unknown> = {
        get_dashboard: state,
        spend_ledger_recent: ledger,
        spend_summary: summary,
        mcp_list_servers: [],
        mcp_list_tools: [],
        mcp_connect: true,
        mcp_disconnect: true,
        key_status: { configured: false, source: null },
        key_status_all: [],
        key_env_candidates: [],
        list_recipes: [],
        list_rules: [],
        list_skills: [],
        list_guidance_profiles: [],
        get_active_guidance_profile: null,
        guided_wins_status: {
          understand: false,
          verify: false,
          improve_ade: false,
          understand_artifact: null,
        },
        chat_load: { id: "stub", updatedAt: new Date().toISOString(), turns: [] },
        chat_save: null,
        chat_clear: null,
        chat_stage_path: {
          name: "stub.txt",
          path: ".ade/inbox/stub.txt",
          absolute: "/tmp/stub.txt",
          bytes: 4,
          staged: true,
          isDir: false,
        },
        chat_stage_bytes: {
          name: "paste.bin",
          path: ".ade/inbox/paste.bin",
          absolute: "/tmp/paste.bin",
          bytes: 0,
          staged: true,
          isDir: false,
        },
        chat_open_path: null,
        chat_fetch_url: {
          name: "fetch-stub.md",
          path: ".ade/inbox/fetch-stub.md",
          absolute: "/tmp/fetch-stub.md",
          bytes: 12,
          staged: true,
          isDir: false,
        },
        workspace_mention_candidates: ["AGENTS.md", "README.md"],
        goal_active: null,
        list_workspaces: {
          current: (state as { workspace_root?: string }).workspace_root ?? "",
          entries: [],
          ade_source_root: null,
        },
      };
      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        value: {
          invoke: (command: string, _args?: unknown) =>
            command in answers
              ? Promise.resolve(answers[command])
              : Promise.reject(new Error(`stub: unhandled command "${command}"`)),
          transformCallback: (callback: unknown) => callback,
        },
        configurable: true,
      });
    },
    [dashboard, ledgerFixture(), spendSummaryFixture] as const,
  );
}
