/** Allow http(s), mailto, and relative/hash links; block javascript: etc. */
export function safeMarkdownHref(href: string | undefined): string | undefined {
  if (!href) return undefined;
  const trimmed = href.trim();
  if (!trimmed) return undefined;
  if (/^(https?:|mailto:|tel:|#|\/|\.\/|\.\.\/)/i.test(trimmed)) {
    return trimmed;
  }
  // Protocol-relative
  if (trimmed.startsWith("//")) return `https:${trimmed}`;
  return undefined;
}

/**
 * Models often emit TeX delimiters KaTeX/remark-math miss by default:
 * \[...\] / \(...\) and bare ```math / ```latex fences.
 */
export function normalizeAssistantMath(text: string): string {
  let out = text;
  // ```math / ```latex fences → $$ blocks
  out = out.replace(/```(?:math|latex|tex)\s*\n([\s\S]*?)```/gi, (_m, body: string) => {
    const inner = String(body).trim();
    return inner ? `\n$$\n${inner}\n$$\n` : "";
  });
  // Display: \[ ... \] (may span lines)
  out = out.replace(/\\\[((?:.|\n)*?)\\\]/g, (_m, body: string) => `$$${body}$$`);
  // Inline: \( ... \)
  out = out.replace(/\\\(((?:.|\n)*?)\\\)/g, (_m, body: string) => `$${body}$`);
  return out;
}
