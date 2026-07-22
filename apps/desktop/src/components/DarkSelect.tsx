import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";

export type DarkSelectOption = {
  value: string;
  label: string;
  group?: string;
};

type DarkSelectProps = {
  ariaLabel: string;
  value: string;
  options: DarkSelectOption[];
  onChange: (value: string) => void;
  title?: string;
  className?: string;
  /** Truncate trigger label for tight composer footers. */
  maxLabelChars?: number;
};

/**
 * Custom select so the open menu matches ADE’s dark chrome.
 * Native &lt;select&gt; popups on Windows stay OS-light and wash out the composer.
 * Menu uses fixed positioning so narrow / overflow-hidden parents cannot clip it.
 */
export function DarkSelect({
  ariaLabel,
  value,
  options,
  onChange,
  title,
  className = "",
  maxLabelChars = 28,
}: DarkSelectProps) {
  const [open, setOpen] = useState(false);
  const [menuStyle, setMenuStyle] = useState<CSSProperties>({});
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  useLayoutEffect(() => {
    if (!open || !rootRef.current) return;
    const place = () => {
      const rect = rootRef.current!.getBoundingClientRect();
      const pad = 8;
      // Keep menus out from under the fixed/static sidebar (~15.5rem).
      const sidebarSafe = 248;
      const menuWidth = Math.min(288, Math.max(176, rect.width));
      let left = rect.left;
      if (left + menuWidth > window.innerWidth - pad) {
        left = Math.max(pad, window.innerWidth - pad - menuWidth);
      }
      left = Math.max(left, Math.min(sidebarSafe, window.innerWidth - menuWidth - pad));
      if (left < pad) left = pad;
      const spaceAbove = rect.top - pad;
      const spaceBelow = window.innerHeight - rect.bottom - pad;
      const preferAbove = spaceAbove >= 160 || spaceAbove >= spaceBelow;
      if (preferAbove) {
        setMenuStyle({
          position: "fixed",
          left,
          width: menuWidth,
          bottom: window.innerHeight - rect.top + 6,
          top: "auto",
          maxHeight: Math.min(224, Math.max(120, spaceAbove)),
        });
      } else {
        setMenuStyle({
          position: "fixed",
          left,
          width: menuWidth,
          top: rect.bottom + 6,
          bottom: "auto",
          maxHeight: Math.min(224, Math.max(120, spaceBelow)),
        });
      }
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onPointer = (event: MouseEvent) => {
      const target = event.target as Node;
      // Trigger lives in rootRef; ignore so the button can toggle closed.
      if (rootRef.current?.contains(target) || menuRef.current?.contains(target)) {
        return;
      }
      setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    // Prefer click over mousedown so the trigger's toggle runs first.
    document.addEventListener("click", onPointer, true);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onPointer, true);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const selected = options.find((opt) => opt.value === value);
  const rawLabel = selected?.label ?? (value || "Select…");
  const label =
    rawLabel.length > maxLabelChars
      ? `${rawLabel.slice(0, Math.max(1, maxLabelChars - 1))}…`
      : rawLabel;

  const groups = groupOptions(options);

  return (
    <div ref={rootRef} className={`relative inline-flex shrink-0 ${className}`}>
      <button
        type="button"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        title={title ?? rawLabel}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((prev) => !prev);
        }}
        className="flex h-7 shrink-0 items-center gap-1 rounded-md border border-white/8 bg-white/3 py-0 pl-2 pr-1.5 text-left text-[10px] font-medium text-slate-300 outline-hidden hover:bg-white/6 hover:text-slate-100 focus-visible:ring-1 focus-visible:ring-blue-400/50"
      >
        <span className="whitespace-nowrap">{label}</span>
        <span className="shrink-0 text-[8px] text-slate-600" aria-hidden>
          ▾
        </span>
      </button>
      {open && (
        <div
          ref={menuRef}
          id={listId}
          role="listbox"
          aria-label={ariaLabel}
          style={menuStyle}
          className="z-[80] overflow-y-auto rounded-lg border border-white/12 bg-[#121820] py-1"
        >
          {groups.map((group) => (
            <div key={group.name ?? "default"}>
              {group.name && (
                <div className="px-2.5 pb-0.5 pt-1.5 text-[9px] font-semibold uppercase tracking-wider text-slate-600">
                  {group.name}
                </div>
              )}
              {group.items.map((opt) => {
                const active = opt.value === value;
                return (
                  <button
                    key={opt.value}
                    type="button"
                    role="option"
                    aria-selected={active}
                    title={opt.label}
                    onClick={() => {
                      onChange(opt.value);
                      setOpen(false);
                    }}
                    className={`flex w-full items-center px-2.5 py-1.5 text-left text-[11px] transition ${
                      active
                        ? "bg-blue-500/20 text-blue-100"
                        : "text-slate-300 hover:bg-white/6 hover:text-slate-100"
                    }`}
                  >
                    <span className="truncate font-mono">{opt.label}</span>
                  </button>
                );
              })}
            </div>
          ))}
          {options.length === 0 && (
            <div className="px-2.5 py-2 text-[11px] text-slate-500">No options</div>
          )}
        </div>
      )}
    </div>
  );
}

function groupOptions(options: DarkSelectOption[]): {
  name: string | null;
  items: DarkSelectOption[];
}[] {
  const order: string[] = [];
  const map = new Map<string, DarkSelectOption[]>();
  for (const opt of options) {
    const key = opt.group ?? "";
    if (!map.has(key)) {
      map.set(key, []);
      order.push(key);
    }
    map.get(key)!.push(opt);
  }
  return order.map((key) => ({
    name: key || null,
    items: map.get(key)!,
  }));
}

/** Compact gear mark for Settings affordances. */
export function GearIcon({ className = "size-3.5" }: { className?: string }): ReactNode {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1.1-1.55 1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.55-1.1 1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V9c0 .69.4 1.31 1.02 1.58.2.09.42.13.63.13H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.51 1Z" />
    </svg>
  );
}
