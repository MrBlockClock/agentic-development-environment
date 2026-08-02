import type { ChatAttachment, AttachmentKind } from "./fileKind";
import { baseName, fileKindFromName, isExtractableKind } from "./fileKind";

function KindIcon({ kind }: { kind: AttachmentKind }) {
  const stroke = "currentColor";
  const common = {
    width: 14,
    height: 14,
    viewBox: "0 0 16 16",
    fill: "none",
    "aria-hidden": true as const,
  };
  switch (kind) {
    case "image":
      return (
        <svg {...common}>
          <rect x="2" y="3" width="12" height="10" rx="1.5" stroke={stroke} strokeWidth="1.25" />
          <circle cx="5.5" cy="6.5" r="1" fill={stroke} />
          <path d="M2.5 11.5 6 8l2.5 2.5L12 7l1.5 1.5" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
    case "pdf":
      return (
        <svg {...common}>
          <path
            d="M4 2.5h5.5L12 5v8.5H4V2.5Z"
            stroke={stroke}
            strokeWidth="1.25"
          />
          <path d="M9.5 2.5V5H12" stroke={stroke} strokeWidth="1.25" />
          <path d="M5.5 9h5M5.5 11.5h3.5" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
    case "office":
      return (
        <svg {...common}>
          <path
            d="M3.5 2.5h6L12.5 5.5v8H3.5v-11Z"
            stroke={stroke}
            strokeWidth="1.25"
          />
          <path d="M9.5 2.5V5.5H12.5" stroke={stroke} strokeWidth="1.25" />
          <path d="M5.5 8h5M5.5 10.5h5M5.5 13h3" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
    case "code":
      return (
        <svg {...common}>
          <path d="M6 4.5 3 8l3 3.5M10 4.5 13 8l-3 3.5" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
    case "text":
      return (
        <svg {...common}>
          <path d="M4 3h8v10H4V3Z" stroke={stroke} strokeWidth="1.25" />
          <path d="M6 6h4M6 8.5h4M6 11h2.5" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
    case "archive":
      return (
        <svg {...common}>
          <path d="M3 5.5h10v7.5H3V5.5Z" stroke={stroke} strokeWidth="1.25" />
          <path d="M5 5.5V4h6v1.5M8 5.5v7.5" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
    case "folder":
      return (
        <svg {...common}>
          <path
            d="M2.5 5.5h4l1.2 1.2H13.5v6.3H2.5V5.5Z"
            stroke={stroke}
            strokeWidth="1.25"
          />
        </svg>
      );
    case "url":
      return (
        <svg {...common}>
          <circle cx="8" cy="8" r="5.25" stroke={stroke} strokeWidth="1.25" />
          <path d="M2.75 8h10.5M8 2.75c1.6 1.7 1.6 8.8 0 10.5M8 2.75c-1.6 1.7-1.6 8.8 0 10.5" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
    case "ticket":
      return (
        <svg {...common}>
          <path
            d="M3 4.5h10v7H3v-7Z"
            stroke={stroke}
            strokeWidth="1.25"
          />
          <path d="M5.5 7h5M5.5 9.5h3.5" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
    default:
      return (
        <svg {...common}>
          <path
            d="M4 2.5h5.5L12 5v8.5H4V2.5Z"
            stroke={stroke}
            strokeWidth="1.25"
          />
          <path d="M9.5 2.5V5H12" stroke={stroke} strokeWidth="1.25" />
        </svg>
      );
  }
}

const kindTone: Record<AttachmentKind, string> = {
  image: "text-cyan-300/90 border-cyan-400/25 bg-cyan-500/10",
  pdf: "text-rose-300/90 border-rose-400/25 bg-rose-500/10",
  office: "text-blue-200/90 border-blue-400/25 bg-blue-500/10",
  code: "text-violet-300/90 border-violet-400/25 bg-violet-500/10",
  text: "text-slate-300 border-white/12 bg-white/5",
  archive: "text-amber-300/90 border-amber-400/25 bg-amber-500/10",
  folder: "text-sky-300/90 border-sky-400/25 bg-sky-500/10",
  url: "text-emerald-300/90 border-emerald-400/25 bg-emerald-500/10",
  ticket: "text-orange-200/90 border-orange-400/25 bg-orange-500/10",
  other: "text-slate-400 border-white/10 bg-white/4",
};

export function AttachmentChips({
  items,
  onRemove,
  onClearAll,
  onOpen,
  onFetch,
  onExtract,
  compact = false,
}: {
  items: ChatAttachment[];
  onRemove?: (id: string) => void;
  onClearAll?: () => void;
  onOpen?: (item: ChatAttachment) => void;
  /** Optional unfurl for url/ticket chips (writes inbox markdown). */
  onFetch?: (item: ChatAttachment) => void;
  /** Optional PDF/Office text extract into inbox markdown. */
  onExtract?: (item: ChatAttachment) => void;
  compact?: boolean;
}) {
  if (items.length === 0) return null;
  return (
    <div className={`flex flex-wrap items-center gap-1.5 ${compact ? "" : "mb-2"}`}>
      {items.map((item) => (
        <div
          key={item.id}
          className={`inline-flex max-w-full items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-medium ${kindTone[item.kind]}`}
          title={item.fetchedPath ? `${item.path}\n→ ${item.fetchedPath}` : item.path}
        >
          <KindIcon kind={item.kind} />
          {item.kind === "image" && item.previewUrl ? (
            <img
              src={item.previewUrl}
              alt=""
              className="size-5 shrink-0 rounded object-cover"
            />
          ) : null}
          {onOpen ? (
            <button
              type="button"
              onClick={() => onOpen(item)}
              className="min-w-0 truncate hover:underline"
            >
              {item.name}
            </button>
          ) : (
            <span className="min-w-0 truncate">{item.name}</span>
          )}
          {onFetch &&
            (item.kind === "url" || item.kind === "ticket") &&
            !item.fetchedPath && (
              <button
                type="button"
                aria-label={`Fetch ${item.name}`}
                title="Fetch page text into .ade/inbox"
                onClick={() => onFetch(item)}
                className="shrink-0 rounded px-1 text-[10px] text-emerald-200/80 hover:bg-white/10 hover:text-emerald-100"
              >
                Fetch
              </button>
            )}
          {onExtract && isExtractableKind(item.kind) && !item.extractedPath && (
            <button
              type="button"
              aria-label={`Extract ${item.name}`}
              title={
                item.kind === "office"
                  ? "Extract .docx/.xlsx text to .ade/inbox/*.extract.md"
                  : "Extract first pages to .ade/inbox/*.extract.md"
              }
              onClick={() => onExtract(item)}
              className={`shrink-0 rounded px-1 text-[10px] hover:bg-white/10 ${
                item.kind === "office"
                  ? "text-blue-200/80 hover:text-blue-100"
                  : "text-rose-200/80 hover:text-rose-100"
              }`}
            >
              Extract
            </button>
          )}
          {item.fetchedPath ? (
            <span className="shrink-0 text-[9px] uppercase tracking-wide opacity-70">
              fetched
            </span>
          ) : null}
          {item.extractedPath ? (
            <span className="shrink-0 text-[9px] uppercase tracking-wide opacity-70">
              extract
            </span>
          ) : null}
          {onRemove && (
            <button
              type="button"
              aria-label={`Remove ${item.name}`}
              onClick={() => onRemove(item.id)}
              className="shrink-0 rounded px-0.5 text-slate-500 hover:bg-white/10 hover:text-slate-200"
            >
              ×
            </button>
          )}
        </div>
      ))}
      {onClearAll && items.length > 1 && (
        <button
          type="button"
          onClick={onClearAll}
          className="rounded-md border border-white/10 px-2 py-1 text-[10px] font-medium text-slate-400 hover:bg-white/5 hover:text-slate-200"
        >
          Clear all
        </button>
      )}
    </div>
  );
}

/** Compact chip for markdown file-like links. */
export function FileLinkChip({
  href,
  label,
  onOpen,
}: {
  href: string;
  label: string;
  onOpen?: (href: string) => void;
}) {
  const kind = fileKindFromName(href);
  const name = label.trim() || baseName(href);
  return (
    <button
      type="button"
      title={href}
      onClick={() => onOpen?.(href)}
      className={`my-1 inline-flex max-w-full items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-medium ${kindTone[kind]}`}
    >
      <KindIcon kind={kind} />
      <span className="min-w-0 truncate">{name}</span>
    </button>
  );
}
