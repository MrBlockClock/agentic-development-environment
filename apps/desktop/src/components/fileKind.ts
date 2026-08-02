/** Path-first chat attachments — kind, packaging, and safety helpers. */

export type AttachmentKind =
  | "image"
  | "pdf"
  | "code"
  | "text"
  | "archive"
  | "folder"
  | "url"
  | "ticket"
  | "other";

export type ChatAttachment = {
  id: string;
  name: string;
  /** Path the agent should read (workspace-relative preferred), or URL for url/ticket. */
  path: string;
  /** Absolute path for Open / previews when available. */
  absolute?: string;
  kind: AttachmentKind;
  mime?: string;
  size?: number;
  /** Optional preview for images (data URL or convertFileSrc). */
  previewUrl?: string;
  /** Optional fetched inbox path for URL chips after unfurl. */
  fetchedPath?: string;
  /** Optional PDF extract markdown path after Extract. */
  extractedPath?: string;
};

/** Persistable attachment fields for `chat_save` (no preview blobs). */
export type ChatAttachmentMeta = {
  id: string;
  name: string;
  path: string;
  absolute?: string;
  kind: AttachmentKind;
  mime?: string;
  size?: number;
  fetchedPath?: string;
  extractedPath?: string;
};

export function toAttachmentMeta(item: ChatAttachment): ChatAttachmentMeta {
  return {
    id: item.id,
    name: item.name,
    path: item.path,
    absolute: item.absolute,
    kind: item.kind,
    mime: item.mime,
    size: item.size,
    fetchedPath: item.fetchedPath,
    extractedPath: item.extractedPath,
  };
}

export const ATTACH_MAX_COUNT = 8;
export const ATTACH_MAX_BYTES = 10 * 1024 * 1024;

const IMAGE_EXT = new Set([
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "bmp",
  "svg",
  "ico",
]);
const CODE_EXT = new Set([
  "rs",
  "ts",
  "tsx",
  "js",
  "jsx",
  "mjs",
  "cjs",
  "py",
  "go",
  "java",
  "kt",
  "c",
  "h",
  "cpp",
  "hpp",
  "cs",
  "rb",
  "php",
  "swift",
  "scala",
  "sql",
  "sh",
  "bash",
  "ps1",
  "toml",
  "json",
  "yaml",
  "yml",
  "xml",
  "html",
  "css",
  "scss",
  "vue",
  "svelte",
]);
const TEXT_EXT = new Set([
  "md",
  "mdx",
  "txt",
  "csv",
  "log",
  "rst",
  "adoc",
]);
const ARCHIVE_EXT = new Set(["zip", "tar", "gz", "tgz", "7z", "rar"]);

const BLOCKED_NAMES = new Set([
  ".env",
  ".env.local",
  ".env.production",
  ".env.development",
]);
const BLOCKED_EXT = new Set([
  "pem",
  "key",
  "p12",
  "pfx",
  "exe",
  "dll",
  "bat",
  "cmd",
  "msi",
  "scr",
]);

export function extensionOf(pathOrName: string): string {
  const base = pathOrName.replace(/\\/g, "/").split("/").pop() ?? pathOrName;
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return "";
  return base.slice(dot + 1).toLowerCase();
}

export function fileKindFromName(pathOrName: string): AttachmentKind {
  const ext = extensionOf(pathOrName);
  if (ext === "pdf") return "pdf";
  if (IMAGE_EXT.has(ext)) return "image";
  if (CODE_EXT.has(ext)) return "code";
  if (TEXT_EXT.has(ext)) return "text";
  if (ARCHIVE_EXT.has(ext)) return "archive";
  return "other";
}

export function baseName(pathOrName: string): string {
  const norm = pathOrName.replace(/\\/g, "/");
  const parts = norm.split("/");
  return parts[parts.length - 1] || pathOrName;
}

/** Refuse secrets / executables with an honest reason. */
export function refuseAttachmentReason(pathOrName: string): string | null {
  const name = baseName(pathOrName);
  const lower = name.toLowerCase();
  if (BLOCKED_NAMES.has(lower) || lower.startsWith(".env.")) {
    return `Refused secret-looking file: ${name}`;
  }
  const ext = extensionOf(name);
  if (BLOCKED_EXT.has(ext)) {
    return `Refused blocked type (.${ext}): ${name}`;
  }
  return null;
}

export function newAttachmentId(): string {
  return `att-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function fromAttachmentMeta(meta: ChatAttachmentMeta): ChatAttachment {
  return {
    id: meta.id || newAttachmentId(),
    name: meta.name,
    path: meta.path,
    absolute: meta.absolute,
    kind: meta.kind,
    mime: meta.mime,
    size: meta.size,
    fetchedPath: meta.fetchedPath,
    extractedPath: meta.extractedPath,
  };
}

export function makeAttachment(input: {
  path: string;
  name?: string;
  absolute?: string;
  mime?: string;
  size?: number;
  previewUrl?: string;
  kind?: AttachmentKind;
  fetchedPath?: string;
  extractedPath?: string;
}): ChatAttachment {
  const name = input.name?.trim() || baseName(input.path);
  return {
    id: newAttachmentId(),
    name,
    path: input.path,
    absolute: input.absolute,
    kind: input.kind ?? fileKindFromName(name),
    mime: input.mime,
    size: input.size,
    previewUrl: input.previewUrl,
    fetchedPath: input.fetchedPath,
    extractedPath: input.extractedPath,
  };
}

/** Append honest path list for the model / tools. */
export function packagePromptWithAttachments(
  prompt: string,
  attachments: ChatAttachment[],
  opts?: { visionCapable?: boolean },
): string {
  const body = prompt.trimEnd();
  if (attachments.length === 0) return body;
  const vision = Boolean(opts?.visionCapable);
  const lines = attachments.map((item) => {
    const kind = item.kind;
    if (kind === "image") {
      return vision
        ? `- ${item.path} (image/${extensionOf(item.name) || "bin"}; binary — also sent as a vision part)`
        : `- ${item.path} (image/${extensionOf(item.name) || "bin"}; binary — vision unavailable on this model; switch to a vision-capable model)`;
    }
    if (kind === "pdf") {
      const extracted = item.extractedPath
        ? `; extract → ${item.extractedPath}`
        : "; prefer Extract to inbox markdown if you need text";
      return `- ${item.path} (pdf${extracted})`;
    }
    if (kind === "archive") {
      return `- ${item.path} (archive; do not unpack into context)`;
    }
    if (kind === "folder") {
      return `- ${item.path} (folder path; list/search inside — do not dump the whole tree)`;
    }
    if (kind === "url") {
      const fetched = item.fetchedPath
        ? `; fetched → ${item.fetchedPath}`
        : "; URL only — fetch to inbox if you need page text";
      return `- ${item.path} (url${fetched})`;
    }
    if (kind === "ticket") {
      const fetched = item.fetchedPath
        ? `; fetched → ${item.fetchedPath}`
        : "";
      return `- ${item.path} (ticket ${item.name}${fetched})`;
    }
    return `- ${item.path}`;
  });
  const block = ["", "Attached:", ...lines].join("\n");
  return body ? `${body}\n${block}` : `Attached:\n${lines.join("\n")}`;
}

export type ParsedUserMessage = {
  text: string;
  attachments: ChatAttachment[];
};

/** Path from an `Attached:` line, stripping kind annotations after ` (`. */
function attachmentFromAttachedLine(line: string): ChatAttachment | null {
  const raw = line.replace(/^- /, "").trim();
  if (!raw) return null;
  const paren = raw.indexOf(" (");
  const path = paren > 0 ? raw.slice(0, paren).trim() : raw;
  if (!path) return null;
  const att = makeAttachment({ path });
  if (/\(folder path;/i.test(raw)) att.kind = "folder";
  if (/\(url/i.test(raw)) att.kind = "url";
  if (/\(ticket /i.test(raw)) {
    att.kind = "ticket";
    const ticketName = raw.match(/\(ticket ([^);]+)/i)?.[1]?.trim();
    if (ticketName) att.name = ticketName;
  }
  const fetched = raw.match(/fetched → ([^\s)]+)/i)?.[1]?.trim();
  if (fetched) att.fetchedPath = fetched;
  return att;
}

function attachmentsFromAttachedBody(body: string): ChatAttachment[] {
  return body
    .split("\n")
    .map((line) => attachmentFromAttachedLine(line))
    .filter((item): item is ChatAttachment => item !== null);
}

/** Split a stored user message back into text + path chips. */
export function parseAttachedBlock(message: string): ParsedUserMessage {
  const marker = /\n\nAttached:\n((?:- .+\n?)+)\s*$/;
  const match = message.match(marker);
  if (!match) {
    // Also accept trailing Attached without blank line
    const alt = message.match(/\nAttached:\n((?:- .+\n?)+)\s*$/);
    if (!alt) return { text: message, attachments: [] };
    return {
      text: message.slice(0, alt.index).trimEnd(),
      attachments: attachmentsFromAttachedBody(alt[1]),
    };
  }
  return {
    text: message.slice(0, match.index).trimEnd(),
    attachments: attachmentsFromAttachedBody(match[1]),
  };
}

/** True when an href looks like a downloadable/file-ish link for chip UI. */
export function isFileLikeHref(href: string): boolean {
  const clean = href.split(/[?#]/)[0] ?? href;
  const ext = extensionOf(clean);
  if (!ext) return false;
  return (
    IMAGE_EXT.has(ext) ||
    CODE_EXT.has(ext) ||
    TEXT_EXT.has(ext) ||
    ARCHIVE_EXT.has(ext) ||
    ext === "pdf"
  );
}

/** True when clipboard/pasted text looks like a filesystem path. */
export function looksLikeFilesystemPath(text: string): boolean {
  const t = text.trim().replace(/^["']|["']$/g, "");
  if (!t || t.includes("\n") || t.length > 512) return false;
  if (/^https?:\/\//i.test(t) || /^mailto:/i.test(t)) return false;
  // Windows absolute, UNC, or POSIX absolute / home-relative
  if (/^[a-zA-Z]:[\\/]/.test(t)) return true;
  if (t.startsWith("\\\\")) return true;
  if (t.startsWith("/") || t.startsWith("~/")) return true;
  // Workspace-relative with a separator and extension or trailing slash
  if (/[\\/]/.test(t) && (/\.[a-z0-9]{1,8}$/i.test(t) || /[\\/]$/.test(t))) {
    return true;
  }
  return false;
}

/** Case-insensitive path-prefix check (Windows + POSIX separators). */
export function isUnderWorkspace(path: string, workspaceRoot: string): boolean {
  const norm = (value: string) =>
    value.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  const root = norm(workspaceRoot);
  const full = norm(path);
  if (!root || !full) return false;
  return full === root || full.startsWith(`${root}/`);
}
