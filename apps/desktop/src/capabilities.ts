/**
 * Single source of truth: what ADE can do in Desktop vs browser preview.
 * Keep nav labels identical; differ only by capability + honest CTAs.
 */

export type AdeShell = "desktop" | "browser";

export type AdeCapability =
  | "home_status"
  | "verify"
  | "recipes_browse_fit"
  | "guidance_read"
  | "atlas_plan_map"
  | "local_api_token"
  | "agent_turns"
  | "provider_keys"
  | "mcp_host"
  | "execute_plan"
  | "recipe_initialize";

export type CapabilityRow = {
  id: AdeCapability;
  label: string;
  desktop: boolean;
  browser: boolean;
  note: string;
};

/** Product capability matrix — labels stay the same across shells. */
export const ADE_CAPABILITY_MATRIX: CapabilityRow[] = [
  {
    id: "home_status",
    label: "Home / dashboard status",
    desktop: true,
    browser: true,
    note: "Via Tauri or `ade serve` loopback API",
  },
  {
    id: "verify",
    label: "Verify (Check)",
    desktop: true,
    browser: true,
    note: "POST /api/verify when API token matches",
  },
  {
    id: "recipes_browse_fit",
    label: "Recipes / Stack Fit",
    desktop: true,
    browser: true,
    note: "Browse + rank over HTTP; initialize may need Desktop",
  },
  {
    id: "guidance_read",
    label: "Guidance / Atlas / Plan Map",
    desktop: true,
    browser: true,
    note: "Reads + profile set over local API",
  },
  {
    id: "local_api_token",
    label: "Local API token setup",
    desktop: false,
    browser: true,
    note: "Browser-only; Desktop uses IPC",
  },
  {
    id: "agent_turns",
    label: "Agent turns",
    desktop: true,
    browser: false,
    note: "No EXECUTE / agent-over-HTTP by design",
  },
  {
    id: "provider_keys",
    label: "Provider Keys (BYOK vault)",
    desktop: true,
    browser: false,
    note: "OS credential vault requires Desktop",
  },
  {
    id: "mcp_host",
    label: "MCP host",
    desktop: true,
    browser: false,
    note: "Process-hosted connections",
  },
  {
    id: "execute_plan",
    label: "Approve & execute plan",
    desktop: true,
    browser: false,
    note: "Mutating execute stays Desktop/CLI",
  },
  {
    id: "recipe_initialize",
    label: "Initialize recipe scaffold",
    desktop: true,
    browser: false,
    note: "Writes workspace files via Desktop",
  },
];

export function capabilityAvailable(
  id: AdeCapability,
  shell: AdeShell,
): boolean {
  const row = ADE_CAPABILITY_MATRIX.find((item) => item.id === id);
  if (!row) return false;
  return shell === "desktop" ? row.desktop : row.browser;
}

/** Views that must show a Desktop funnel in browser preview. */
export const DESKTOP_REQUIRED_VIEWS = new Set([
  "Agent",
  "Keys",
  "MCP",
]);

export function desktopRequiredCopy(view: string): {
  title: string;
  body: string;
  next: string;
} {
  switch (view) {
    case "Keys":
      return {
        title: "Keys need Desktop",
        body: "Provider credentials are stored in the OS vault. Browser preview cannot save or smoke-test API keys.",
        next: "Open ADE Desktop → Keys, then return here for status and checks.",
      };
    case "Agent":
      return {
        title: "Agent needs Desktop",
        body: "Chat turns, Apply/Automate, and EXECUTE stay in the desktop harness. There is no agent-over-HTTP.",
        next: "Open ADE Desktop → Agent. Use this browser for Verify, Recipes, and Trust views.",
      };
    case "MCP":
      return {
        title: "MCP needs Desktop",
        body: "MCP servers are hosted in the desktop process, not the loopback API.",
        next: "Open ADE Desktop → MCP to connect servers.",
      };
    default:
      return {
        title: "Desktop required",
        body: "This action is not available in browser preview.",
        next: "Open the ADE desktop app to continue.",
      };
  }
}
