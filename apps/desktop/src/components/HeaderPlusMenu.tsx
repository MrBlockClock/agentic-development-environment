import {
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
} from "react";

export type HeaderMenuAction = {
  id: string;
  label: string;
  description?: string;
  icon: ReactNode;
  group?: string;
  desktopOnly?: boolean;
  onSelect: () => void;
};

type HeaderOverflowMenuProps = {
  actions: HeaderMenuAction[];
  isDesktop: boolean;
};

/**
 * Compact ⋯ menu — hangs under the ribbon button (absolute, not a floating panel).
 */
export function HeaderOverflowMenu({
  actions,
  isDesktop,
}: HeaderOverflowMenuProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  useEffect(() => {
    if (!open) return;
    const onPointer = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  if (actions.length === 0) return null;

  return (
    <div ref={rootRef} className="relative shrink-0">
      <button
        type="button"
        aria-label="More"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={listId}
        title="More"
        onClick={(event) => {
          event.stopPropagation();
          setOpen((prev) => !prev);
        }}
        className={`grid size-7 place-items-center rounded-md text-[14px] font-bold leading-none tracking-tighter transition ${
          open
            ? "bg-white/10 text-slate-100"
            : "text-slate-400 hover:bg-white/8 hover:text-slate-100"
        }`}
      >
        <span aria-hidden>⋯</span>
      </button>
      {open && (
        <div
          id={listId}
          role="menu"
          aria-label="More"
          className="absolute right-0 top-full z-[80] mt-1 min-w-44 overflow-hidden rounded-md border border-white/10 bg-[#121820] py-0.5 shadow-lg"
        >
          {actions.map((action) => {
            const locked = Boolean(action.desktopOnly) && !isDesktop;
            return (
              <button
                key={action.id}
                type="button"
                role="menuitem"
                title={
                  locked
                    ? `${action.label} needs ADE Desktop`
                    : (action.description ?? action.label)
                }
                onClick={() => {
                  action.onSelect();
                  setOpen(false);
                }}
                className="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-[11px] text-slate-200 hover:bg-white/6"
              >
                <span className="w-4 text-center text-slate-500" aria-hidden>
                  {action.icon}
                </span>
                <span className="min-w-0 flex-1 truncate font-medium">
                  {action.label}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** @deprecated */
export const HeaderPlusMenu = HeaderOverflowMenu;
export type HeaderPlusAction = HeaderMenuAction;
