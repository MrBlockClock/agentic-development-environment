/**
 * Standing connectors (Integrations) vs per-turn host/MCP tools (Tools).
 * Tokens reuse the OS vault via `vaultId`; LLM BYOK stays on Keys.
 */

export type IntegrationKind =
  | "token"
  | "mcp"
  | "builtin"
  | "keys"
  | "surface";

export type IntegrationCategory =
  | "source"
  | "cloud"
  | "collab"
  | "payments"
  | "host";

export type McpRecipe = {
  name: string;
  /** Windows Desktop uses npx.cmd; Unix uses npx. */
  commandWin: string;
  commandUnix: string;
  args: string[];
  /** Env var the MCP server expects (user must export or we note it). */
  envHint?: string;
  docsUrl?: string;
};

export type IntegrationDef = {
  id: string;
  label: string;
  category: IntegrationCategory;
  kind: IntegrationKind;
  blurb: string;
  /** OS vault account id when kind is token (or shared github PAT). */
  vaultId?: string;
  getKeyUrl?: string;
  envVars?: string[];
  mcpRecipe?: McpRecipe;
  /** Jump target for surface / keys kinds. */
  openView?: "Keys" | "MCP" | "Browser" | "Terminal" | "Workspaces";
  /** Mark as foundation for Tier 0 strip ordering. */
  fundamental?: boolean;
};

export const INTEGRATION_CATEGORY_LABEL: Record<IntegrationCategory, string> = {
  source: "Source control",
  cloud: "Cloud & infra",
  payments: "Payments",
  collab: "Collaboration",
  host: "Host & local",
};

/** Built-in Desktop tools available on agent turns (not MCP). */
export const HOST_TOOLS: { id: string; label: string; note: string }[] = [
  { id: "fs", label: "Filesystem", note: "Read / write under workspace + leases" },
  { id: "shell", label: "Shell", note: "Scoped Workspace or Home cwd" },
  { id: "web_fetch", label: "Web fetch", note: "HTTP fetch for docs and APIs" },
  { id: "web_search", label: "Web search", note: "Search when the turn needs it" },
  { id: "browser", label: "Browser", note: "In-app Browser surface + agent browse" },
];

export const INTEGRATIONS: IntegrationDef[] = [
  {
    id: "keys",
    label: "Model providers",
    category: "host",
    kind: "keys",
    blurb: "BYOK LLM keys (OpenAI, Anthropic, FreeLLM, …) live under Keys — not duplicated here.",
    openView: "Keys",
    fundamental: true,
  },
  {
    id: "mcp-host",
    label: "MCP host",
    category: "host",
    kind: "surface",
    blurb: "Connect stdio MCP servers so the agent can call their tools this session.",
    openView: "MCP",
    fundamental: true,
  },
  {
    id: "browser",
    label: "In-app Browser",
    category: "host",
    kind: "builtin",
    blurb: "Dedicated WebView for live browsing; agents also get web tools on Desktop turns.",
    openView: "Browser",
    fundamental: true,
  },
  {
    id: "terminal",
    label: "Terminal",
    category: "host",
    kind: "builtin",
    blurb: "Interactive PTY rooted at the attached workspace.",
    openView: "Terminal",
    fundamental: true,
  },
  {
    id: "github",
    label: "GitHub",
    category: "source",
    kind: "token",
    blurb: "PAT for repos, PRs, and Issues. Same token powers GitHub Models under Keys when you use that provider.",
    vaultId: "github",
    getKeyUrl: "https://github.com/settings/tokens",
    envVars: ["GITHUB_TOKEN", "GITHUB_PERSONAL_ACCESS_TOKEN"],
    mcpRecipe: {
      name: "github",
      commandWin: "npx.cmd",
      commandUnix: "npx",
      args: ["-y", "@modelcontextprotocol/server-github"],
      envHint: "GITHUB_PERSONAL_ACCESS_TOKEN",
      docsUrl: "https://github.com/modelcontextprotocol/servers/tree/main/src/github",
    },
    fundamental: true,
  },
  {
    id: "gitlab",
    label: "GitLab",
    category: "source",
    kind: "token",
    blurb: "Personal access token for projects, MRs, and pipelines.",
    vaultId: "gitlab",
    getKeyUrl: "https://gitlab.com/-/user_settings/personal_access_tokens",
    envVars: ["GITLAB_TOKEN", "GITLAB_PERSONAL_ACCESS_TOKEN"],
    mcpRecipe: {
      name: "gitlab",
      commandWin: "npx.cmd",
      commandUnix: "npx",
      args: ["-y", "@modelcontextprotocol/server-gitlab"],
      envHint: "GITLAB_PERSONAL_ACCESS_TOKEN",
      docsUrl: "https://gitlab.com/gitlab-org/modelops/applied-ml/code-suggestions/ai-assist/-/tree/main",
    },
    fundamental: true,
  },
  {
    id: "azure",
    label: "Azure",
    category: "cloud",
    kind: "token",
    blurb: "Service principal secret or PAT for Azure DevOps / ARM scripts. Azure OpenAI keys stay under Keys as azure-openai.",
    vaultId: "azure",
    getKeyUrl: "https://portal.azure.com/#view/Microsoft_Azure_Billing/SubscriptionsBlade",
    envVars: ["AZURE_CLIENT_SECRET", "AZURE_DEVOPS_EXT_PAT", "AZURE_PAT"],
    mcpRecipe: {
      name: "azure",
      commandWin: "npx.cmd",
      commandUnix: "npx",
      args: ["-y", "@azure/mcp@latest", "server", "start"],
      envHint: "AZURE_TENANT_ID / AZURE_CLIENT_ID / AZURE_CLIENT_SECRET",
      docsUrl: "https://learn.microsoft.com/azure/developer/azure-mcp-server/",
    },
    fundamental: true,
  },
  {
    id: "aws",
    label: "AWS",
    category: "cloud",
    kind: "token",
    blurb: "Access key secret for CLI/SDK work the agent runs under shell leases.",
    vaultId: "aws",
    getKeyUrl: "https://console.aws.amazon.com/iam/",
    envVars: ["AWS_SECRET_ACCESS_KEY", "AWS_ACCESS_KEY_ID"],
    fundamental: true,
  },
  {
    id: "google-cloud",
    label: "Google Cloud",
    category: "cloud",
    kind: "token",
    blurb: "Service account JSON key or access token for gcloud / APIs.",
    vaultId: "google-cloud",
    getKeyUrl: "https://console.cloud.google.com/iam-admin/serviceaccounts",
    envVars: ["GOOGLE_APPLICATION_CREDENTIALS", "GCP_SA_KEY"],
    fundamental: true,
  },
  {
    id: "stripe",
    label: "Stripe",
    category: "payments",
    kind: "token",
    blurb: "Secret key for billing, webhooks, and payment tooling the agent may script.",
    vaultId: "stripe",
    getKeyUrl: "https://dashboard.stripe.com/apikeys",
    envVars: ["STRIPE_SECRET_KEY", "STRIPE_API_KEY"],
    mcpRecipe: {
      name: "stripe",
      commandWin: "npx.cmd",
      commandUnix: "npx",
      args: ["-y", "@stripe/mcp", "--tools=all"],
      envHint: "STRIPE_SECRET_KEY",
      docsUrl: "https://docs.stripe.com/mcp",
    },
    fundamental: true,
  },
  {
    id: "slack",
    label: "Slack",
    category: "collab",
    kind: "token",
    blurb: "Bot token for posting and reading channels the workspace allows.",
    vaultId: "slack",
    getKeyUrl: "https://api.slack.com/apps",
    envVars: ["SLACK_BOT_TOKEN", "SLACK_TOKEN"],
    mcpRecipe: {
      name: "slack",
      commandWin: "npx.cmd",
      commandUnix: "npx",
      args: ["-y", "@modelcontextprotocol/server-slack"],
      envHint: "SLACK_BOT_TOKEN",
    },
    fundamental: true,
  },
  {
    id: "linear",
    label: "Linear",
    category: "collab",
    kind: "token",
    blurb: "API key for issues and project status the agent can query or update.",
    vaultId: "linear",
    getKeyUrl: "https://linear.app/settings/api",
    envVars: ["LINEAR_API_KEY"],
    mcpRecipe: {
      name: "linear",
      commandWin: "npx.cmd",
      commandUnix: "npx",
      args: ["-y", "mcp-linear"],
      envHint: "LINEAR_API_KEY",
    },
    fundamental: true,
  },
  {
    id: "jira",
    label: "Jira",
    category: "collab",
    kind: "token",
    blurb: "Atlassian API token for Jira Cloud issues and boards.",
    vaultId: "jira",
    getKeyUrl: "https://id.atlassian.com/manage-profile/security/api-tokens",
    envVars: ["JIRA_API_TOKEN", "ATLASSIAN_API_TOKEN"],
    fundamental: true,
  },
  {
    id: "notion",
    label: "Notion",
    category: "collab",
    kind: "token",
    blurb: "Internal integration token for pages and databases.",
    vaultId: "notion",
    getKeyUrl: "https://www.notion.so/my-integrations",
    envVars: ["NOTION_TOKEN", "NOTION_API_KEY"],
  },
  {
    id: "discord",
    label: "Discord",
    category: "collab",
    kind: "token",
    blurb: "Bot token for guild automation the agent runs under Apply.",
    vaultId: "discord",
    getKeyUrl: "https://discord.com/developers/applications",
    envVars: ["DISCORD_BOT_TOKEN", "DISCORD_TOKEN"],
  },
];

export function integrationsByCategory(): {
  category: IntegrationCategory;
  label: string;
  items: IntegrationDef[];
}[] {
  const order: IntegrationCategory[] = [
    "host",
    "source",
    "cloud",
    "payments",
    "collab",
  ];
  return order.map((category) => ({
    category,
    label: INTEGRATION_CATEGORY_LABEL[category],
    items: INTEGRATIONS.filter((item) => item.category === category),
  }));
}

export function mcpCommandForPlatform(recipe: McpRecipe): string {
  if (typeof navigator !== "undefined" && /Win/i.test(navigator.platform)) {
    return recipe.commandWin;
  }
  return recipe.commandUnix;
}
