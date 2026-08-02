import { useEffect, useMemo, useRef } from "react";

export type MentionItem = {
  id: string;
  kind: "path" | "mcp";
  /** Inserted into the composer (e.g. `@src/App.tsx` or `@mcp:server/tool`). */
  insert: string;
  label: string;
  detail?: string;
};

export function MentionPalette({
  items,
  activeIndex,
  onActiveIndexChange,
  onPick,
  onClose,
}: {
  items: MentionItem[];
  activeIndex: number;
  onActiveIndexChange: (index: number) => void;
  onPick: (item: MentionItem) => void;
  onClose: () => void;
}) {
  const listRef = useRef<HTMLDivElement>(null);
  const clamped = useMemo(() => {
    if (items.length === 0) return 0;
    return Math.max(0, Math.min(activeIndex, items.length - 1));
  }, [activeIndex, items.length]);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(
      `[data-mention-index="${clamped}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }, [clamped]);

  if (items.length === 0) {
    return (
      <div
        className="absolute bottom-full left-0 z-30 mb-1 w-72 max-w-[min(100%,20rem)] rounded-xl border border-line bg-surface-2 p-2 text-[11px] text-ink-faint shadow-panel"
        role="listbox"
        aria-label="Mentions"
      >
        No matches — type a path or MCP tool name
        <button
          type="button"
          className="mt-1 block text-[10px] text-ink-dim hover:text-ink"
          onClick={onClose}
        >
          Esc to close
        </button>
      </div>
    );
  }

  return (
    <div
      ref={listRef}
      className="absolute bottom-full left-0 z-30 mb-1 max-h-56 w-80 max-w-[min(100%,22rem)] overflow-y-auto rounded-xl border border-line bg-surface-2 py-1 shadow-panel"
      role="listbox"
      aria-label="Mentions"
    >
      {items.map((item, index) => (
        <button
          key={item.id}
          type="button"
          role="option"
          aria-selected={index === clamped}
          data-mention-index={index}
          className={`flex w-full items-start gap-2 px-2.5 py-1.5 text-left ${
            index === clamped ? "bg-white/8" : "hover:bg-white/5"
          }`}
          onMouseEnter={() => onActiveIndexChange(index)}
          onClick={() => onPick(item)}
        >
          <span
            className={`mt-0.5 shrink-0 rounded px-1 py-0.5 text-[9px] font-semibold uppercase tracking-wide ${
              item.kind === "mcp"
                ? "bg-violet-500/15 text-violet-200"
                : "bg-sky-500/15 text-sky-200"
            }`}
          >
            {item.kind === "mcp" ? "mcp" : "path"}
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[12px] font-medium text-ink">
              {item.label}
            </span>
            {item.detail ? (
              <span className="mt-0.5 block truncate text-[10px] text-ink-faint">
                {item.detail}
              </span>
            ) : null}
          </span>
        </button>
      ))}
    </div>
  );
}
