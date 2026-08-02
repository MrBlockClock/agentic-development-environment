import { useCallback, useRef, type KeyboardEvent } from "react";
import type { ShellTab } from "../shellTabs";

type ShellTabBarProps = {
  tabs: ShellTab[];
  activeTabId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
};

const KIND_ICON: Record<ShellTab["kind"], string> = {
  agent: "◆",
  browser: "⬚",
  editor: "✎",
  terminal: "▸",
};

/**
 * Cursor-style shell tabs — × always visible and vertically centered.
 * One focusable `role="tab"` per tab; arrow keys move selection.
 */
export function ShellTabBar({
  tabs,
  activeTabId,
  onSelect,
  onClose,
}: ShellTabBarProps) {
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const focusIndex = useCallback(
    (index: number) => {
      const clamped = Math.max(0, Math.min(tabs.length - 1, index));
      const tab = tabs[clamped];
      if (!tab) return;
      onSelect(tab.id);
      requestAnimationFrame(() => {
        tabRefs.current[clamped]?.focus();
      });
    },
    [onSelect, tabs],
  );

  const onTabListKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (tabs.length === 0) return;
    const current = Math.max(
      0,
      tabs.findIndex((tab) => tab.id === activeTabId),
    );
    switch (event.key) {
      case "ArrowRight":
        event.preventDefault();
        focusIndex(current + 1);
        break;
      case "ArrowLeft":
        event.preventDefault();
        focusIndex(current - 1);
        break;
      case "Home":
        event.preventDefault();
        focusIndex(0);
        break;
      case "End":
        event.preventDefault();
        focusIndex(tabs.length - 1);
        break;
      default:
        break;
    }
  };

  if (tabs.length === 0) return null;

  return (
    <div
      role="tablist"
      aria-label="Open tabs"
      onKeyDown={onTabListKeyDown}
      className="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto thin-scrollbar"
    >
      {tabs.map((tab, index) => {
        const active = tab.id === activeTabId;
        return (
          <div
            key={tab.id}
            className={`flex h-7 max-w-52 shrink-0 items-center gap-0.5 rounded-md border pl-1.5 pr-0.5 transition ${
              active
                ? "border-white/12 bg-white/8 text-slate-100"
                : "border-transparent text-slate-400 hover:bg-white/4 hover:text-slate-200"
            }`}
          >
            <button
              ref={(node) => {
                tabRefs.current[index] = node;
              }}
              type="button"
              role="tab"
              aria-selected={active}
              tabIndex={active ? 0 : -1}
              aria-label={tab.title}
              title={tab.title}
              className="flex h-full min-w-0 flex-1 items-center gap-1.5 text-left"
              onClick={() => onSelect(tab.id)}
            >
              <span className="shrink-0 text-[10px] text-current/70" aria-hidden>
                {KIND_ICON[tab.kind]}
              </span>
              <span className="truncate text-[12px] font-medium leading-none tracking-tight">
                {tab.title}
              </span>
            </button>
            {tab.closable ? (
              <button
                type="button"
                tabIndex={-1}
                aria-label={`Close ${tab.title}`}
                title="Close"
                className="inline-flex size-5 shrink-0 items-center justify-center rounded text-slate-500 hover:bg-white/10 hover:text-slate-100"
                onClick={(event) => {
                  event.stopPropagation();
                  onClose(tab.id);
                }}
              >
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 10 10"
                  aria-hidden
                  className="block"
                >
                  <path
                    d="M2 2l6 6M8 2L2 8"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                  />
                </svg>
              </button>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
