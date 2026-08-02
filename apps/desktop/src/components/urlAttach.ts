/** URL / GitHub ticket chip helpers for composer attachments. */

export type ParsedHttpUrl = {
  url: string;
  host: string;
  pathname: string;
};

export type ParsedGitHubTicket = {
  url: string;
  owner: string;
  repo: string;
  number: number;
  kind: "issue" | "pull";
  label: string;
};

/** True when trimmed text is a single http(s) URL (no newlines). */
export function looksLikeHttpUrl(text: string): boolean {
  const t = text.trim().replace(/^["']|["']$/g, "");
  if (!t || t.includes("\n") || t.length > 2048) return false;
  return /^https?:\/\/[^\s]+$/i.test(t);
}

export function parseHttpUrl(text: string): ParsedHttpUrl | null {
  if (!looksLikeHttpUrl(text)) return null;
  const raw = text.trim().replace(/^["']|["']$/g, "");
  try {
    const url = new URL(raw);
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    return {
      url: url.toString(),
      host: url.hostname,
      pathname: url.pathname,
    };
  } catch {
    return null;
  }
}

/**
 * Parse GitHub issue/PR URLs into a ticket label.
 * Supports: github.com/owner/repo/issues|pull|pulls/N
 */
export function parseGitHubTicket(text: string): ParsedGitHubTicket | null {
  const parsed = parseHttpUrl(text);
  if (!parsed) return null;
  const host = parsed.host.toLowerCase();
  if (host !== "github.com" && host !== "www.github.com") return null;
  const match = parsed.pathname.match(
    /^\/([^/]+)\/([^/]+)\/(issues|pull|pulls)\/(\d+)\/?$/i,
  );
  if (!match) return null;
  const owner = match[1]!;
  const repo = match[2]!;
  const kindRaw = match[3]!.toLowerCase();
  const number = Number(match[4]);
  if (!Number.isFinite(number) || number <= 0) return null;
  const kind = kindRaw.startsWith("pull") ? "pull" : "issue";
  return {
    url: parsed.url,
    owner,
    repo,
    number,
    kind,
    label: `${owner}/${repo}#${number}`,
  };
}

export function urlChipLabel(url: string): string {
  const ticket = parseGitHubTicket(url);
  if (ticket) return ticket.label;
  const parsed = parseHttpUrl(url);
  if (!parsed) return url;
  const path = parsed.pathname === "/" ? "" : parsed.pathname;
  const label = `${parsed.host}${path}`;
  return label.length > 48 ? `${label.slice(0, 45)}…` : label;
}
