/** Client-only parsing of assistant choice lists (structured + regex fallback). */

export type OptionItem = {
  label: string;
  /** Text sent on click (defaults to label). */
  prompt: string;
};

export type AssistantSegment =
  | { kind: "markdown"; text: string }
  | { kind: "options"; title: string; items: OptionItem[] };

export const NEXT_ACTIONS_SCHEMA = "ade.next-actions/v1";

/** Lead-ins that usually precede a choice list from the model. */
const OPTION_LEAD =
  /(?:^|\n)((?:#{1,3}\s+)?(?:\*\*)?(?:what would you like|some options|options|next steps|choose one|pick one|how should we proceed|what next)[^\n]*)\n+((?:[ \t]*(?:\d+[.)]|[-*•])[ \t]+.+\n?)+)/gi;

/** Fenced blocks: ade.options / ade.next-actions, or json with schema. */
const STRUCTURED_FENCE =
  /```(?:ade\.options|ade\.next-actions|json)[^\n]*\n([\s\S]*?)```/gi;

type MatchSpan = {
  start: number;
  end: number;
  title: string;
  items: OptionItem[];
};

function stripMdNoise(s: string): string {
  return s
    .replace(/\*\*/g, "")
    .replace(/^#+\s*/, "")
    .replace(/^[ \t]*\d+[.)]\s*/, "")
    .replace(/^[ \t]*[-*•]\s*/, "")
    .trim();
}

function parseListItems(block: string): OptionItem[] {
  return block
    .split(/\n/)
    .map((line) => stripMdNoise(line))
    .filter((line) => line.length > 0)
    .map((line) => ({ label: line, prompt: line }));
}

function asOptionItem(raw: unknown): OptionItem | null {
  if (typeof raw === "string") {
    const label = raw.trim();
    return label ? { label, prompt: label } : null;
  }
  if (!raw || typeof raw !== "object") return null;
  const obj = raw as Record<string, unknown>;
  const label =
    typeof obj.label === "string"
      ? obj.label.trim()
      : typeof obj.text === "string"
        ? obj.text.trim()
        : "";
  if (!label) return null;
  const prompt =
    typeof obj.prompt === "string" && obj.prompt.trim()
      ? obj.prompt.trim()
      : label;
  return { label, prompt };
}

/** Parse a structured next-actions payload; returns null if not our schema. */
export function parseNextActionsPayload(
  raw: string,
  fenceLangHint?: string,
): { title: string; items: OptionItem[] } | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw.trim());
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;
  const obj = parsed as Record<string, unknown>;
  const schema = typeof obj.schema === "string" ? obj.schema.trim() : "";
  const lang = (fenceLangHint ?? "").toLowerCase();
  const langOk =
    lang.includes("ade.options") || lang.includes("ade.next-actions");
  if (schema !== NEXT_ACTIONS_SCHEMA && !langOk) {
    return null;
  }
  const list = Array.isArray(obj.items)
    ? obj.items
    : Array.isArray(obj.options)
      ? obj.options
      : null;
  if (!list) return null;
  const items = list
    .map(asOptionItem)
    .filter((item): item is OptionItem => item !== null);
  if (items.length < 2) return null;
  const title =
    typeof obj.title === "string" && obj.title.trim()
      ? obj.title.trim()
      : "Options";
  return { title, items };
}

function collectStructured(text: string): MatchSpan[] {
  const spans: MatchSpan[] = [];
  const re = new RegExp(STRUCTURED_FENCE.source, STRUCTURED_FENCE.flags);
  let match: RegExpExecArray | null;
  while ((match = re.exec(text)) !== null) {
    const full = match[0];
    const body = match[1] ?? "";
    const langMatch = full.match(/^```([^\n]*)/);
    const lang = langMatch?.[1]?.trim() ?? "";
    const parsed = parseNextActionsPayload(body, lang);
    if (!parsed) continue;
    spans.push({
      start: match.index,
      end: match.index + full.length,
      title: parsed.title,
      items: parsed.items,
    });
  }
  return spans;
}

function collectRegex(text: string, occupied: MatchSpan[]): MatchSpan[] {
  const spans: MatchSpan[] = [];
  const re = new RegExp(OPTION_LEAD.source, OPTION_LEAD.flags);
  let match: RegExpExecArray | null;
  while ((match = re.exec(text)) !== null) {
    const full = match[0];
    const titleRaw = match[1] ?? "";
    const listRaw = match[2] ?? "";
    const start = match.index + (full.startsWith("\n") ? 1 : 0);
    const end = match.index + full.length;
    if (occupied.some((span) => start < span.end && end > span.start)) {
      continue;
    }
    const items = parseListItems(listRaw);
    if (items.length < 2) continue;
    spans.push({
      start,
      end,
      title: stripMdNoise(titleRaw) || "Options",
      items,
    });
  }
  return spans;
}

/**
 * Split assistant text so choice lists render as chips.
 * Structured fences win; prose OPTION_LEAD lists remain the fallback.
 */
export function splitOptionSegments(text: string): AssistantSegment[] {
  if (!text) return [{ kind: "markdown", text: "" }];

  const structured = collectStructured(text);
  const regex = collectRegex(text, structured);
  const all = [...structured, ...regex].sort((a, b) => a.start - b.start);

  // Drop overlapping regex leftovers (structured already preferred via occupied).
  const merged: MatchSpan[] = [];
  for (const span of all) {
    if (merged.some((prev) => span.start < prev.end && span.end > prev.start)) {
      continue;
    }
    merged.push(span);
  }

  if (merged.length === 0) {
    return [{ kind: "markdown", text }];
  }

  const segments: AssistantSegment[] = [];
  let last = 0;
  for (const span of merged) {
    if (span.start > last) {
      segments.push({ kind: "markdown", text: text.slice(last, span.start) });
    }
    segments.push({
      kind: "options",
      title: span.title,
      items: span.items,
    });
    last = span.end;
  }
  if (last < text.length) {
    segments.push({ kind: "markdown", text: text.slice(last) });
  }
  return segments.length > 0 ? segments : [{ kind: "markdown", text }];
}
