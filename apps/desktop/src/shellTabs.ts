export type ShellTabKind = "agent" | "browser" | "editor" | "terminal";

export type ShellTab = {
  id: string;
  kind: ShellTabKind;
  title: string;
  /** Primary Home agent cannot be closed. */
  closable: boolean;
  /** Agent-only: skip workspace chat_load / chat_save. */
  ephemeral?: boolean;
  /** Browser starting URL. */
  url?: string;
  /** Editor workspace-relative (or absolute) path. */
  path?: string;
};

export function newTabId(prefix: string): string {
  return `${prefix}-${Math.random().toString(36).slice(2, 9)}`;
}

export function leafName(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function viewForTabKind(kind: ShellTabKind): string {
  switch (kind) {
    case "agent":
      return "Home";
    case "browser":
      return "Browser";
    case "editor":
      return "Editor";
    case "terminal":
      return "Terminal";
  }
}

export function defaultBrowserUrl(): string {
  return "https://www.google.com/";
}

/** Default titles before the first user prompt renames the tab. */
export function isDefaultAgentTabTitle(title: string): boolean {
  return title === "Agent" || title === "New Agent";
}

/** One-line tab label from the first user message. */
export function agentTabTitleFromPrompt(prompt: string, maxLen = 28): string {
  const firstLine =
    prompt
      .split(/\r?\n/)
      .map((line) => line.trim())
      .find((line) => line.length > 0) ?? "";
  // Drop common attachment packaging headers if present.
  const withoutPack = firstLine.replace(/^attachments?:\s*/i, "").trim();
  const cleaned = withoutPack.replace(/\s+/g, " ");
  if (!cleaned) return "";
  if (cleaned.length <= maxLen) return cleaned;
  return `${cleaned.slice(0, Math.max(1, maxLen - 1)).trimEnd()}…`;
}
