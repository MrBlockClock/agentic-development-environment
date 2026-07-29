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
  | "analytics"
  | "local_api_token"
  | "agent_turns"
  | "provider_keys"
  | "mcp_host"
  | "integrations"
  | "execute_plan"
  | "recipe_initialize"
  | "in_app_browser"
  | "interactive_terminal"
  | "monaco_editor"
  | "workspaces_switch";

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
    note: "Browse + rank + preview over HTTP; initialize scaffold needs Desktop",
  },
  {
    id: "guidance_read",
    label: "Guidance / Atlas / Plan Map",
    desktop: true,
    browser: true,
    note: "Reads + profile set + guided wins over local API",
  },
  {
    id: "analytics",
    label: "Analytics (spend trend, attribution, reserve Δ)",
    desktop: true,
    browser: true,
    note: "Local usage ledger via IPC or `ade serve` /api/spend/*",
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
    id: "integrations",
    label: "Integrations hub",
    desktop: true,
    browser: false,
    note: "Standing connectors (GitHub, GitLab, Stripe, Azure, …) + MCP recipes; OS vault",
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
  {
    id: "in_app_browser",
    label: "In-app Browser (WebView2)",
    desktop: true,
    browser: false,
    note: "Separate Chromium window; agents also get web_fetch/web_search host tools",
  },
  {
    id: "interactive_terminal",
    label: "Interactive Terminal (PTY)",
    desktop: true,
    browser: false,
    note: "Workspace shell via portable-pty + xterm; agent shell tools come later (WP43)",
  },
  {
    id: "monaco_editor",
    label: "Monaco Editor (text)",
    desktop: true,
    browser: false,
    note: "Light workspace text edit; SensitivePathPolicy; no VS Code extensions",
  },
  {
    id: "workspaces_switch",
    label: "Open / create / switch workspaces",
    desktop: true,
    browser: false,
    note: "Folder picker + AGENTS.md adopt; Environment audits the attached root",
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
  "Keys",
  "Integrations",
  "MCP",
  "Browser",
  "Terminal",
  "Editor",
  "Workspaces",
]);

export function desktopRequiredCopy(view: string): {
  title: string;
  body: string;
  next: string;
} {
  switch (view) {
    case "Keys":
      return {
        title: "Keys need the Desktop app",
        body: "Provider credentials live in the OS vault. This browser preview cannot save or test API keys.",
        next: "Open ADE Desktop → Keys, then return here for status and checks.",
      };
    case "Integrations":
      return {
        title: "Integrations need the Desktop app",
        body: "Connector tokens and MCP recipes use the Desktop vault and process-hosted MCP.",
        next: "Open ADE Desktop → Setup → Integrations.",
      };
    case "MCP":
      return {
        title: "MCP needs Desktop",
        body: "MCP servers are hosted in the desktop process, not the loopback API.",
        next: "Open ADE Desktop → MCP to connect servers.",
      };
    case "Browser":
      return {
        title: "In-app Browser needs Desktop",
        body: "ADE opens a dedicated WebView2 / Chromium window for live browsing. Agents already get web_search and web_fetch on Desktop turns without this window.",
        next: "Open ADE Desktop → Browser, or ask ADE on Home to fetch docs.",
      };
    case "Terminal":
      return {
        title: "Terminal needs Desktop",
        body: "Interactive shells run as a local PTY inside the Desktop app, rooted at the attached workspace.",
        next: "Open ADE Desktop → Terminal.",
      };
    case "Editor":
      return {
        title: "Editor needs Desktop",
        body: "Monaco edits workspace text files through the Desktop FS bridge (SensitivePathPolicy enforced).",
        next: "Open ADE Desktop → Editor.",
      };
    case "Workspaces":
      return {
        title: "Workspaces need Desktop",
        body: "Opening, adopting, and switching folders uses the Desktop file dialog and local ADE state.",
        next: "Open ADE Desktop → Workspaces → New workspace (or Open/Adopt), then use Environment to audit it.",
      };
    default:
      return {
        title: "Desktop required",
        body: "This action is not available in browser preview.",
        next: "Open the ADE desktop app to continue.",
      };
  }
}
