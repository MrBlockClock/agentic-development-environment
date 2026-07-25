import type { ShellTab } from "../shellTabs";

type ShellTabBarProps = {
  tabs: ShellTab[];
  activeTabId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
};

const KIND_ICON: Record<ShellTab["kind"], string> = {
  agent: "⌂",
  browser: "⬚",
  editor: "✎",
  terminal: "▸",
};

/**
 * Cursor-style shell tabs — × always visible and vertically centered.
 */
export function ShellTabBar({
  tabs,
  activeTabId,
  onSelect,
  onClose,
}: ShellTabBarProps) {
  if (tabs.length === 0) return null;

  return (
    <div
      role="tablist"
      aria-label="Open tabs"
      className="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto thin-scrollbar"
    >
      {tabs.map((tab) => {
        const active = tab.id === activeTabId;
        return (
          <div
            key={tab.id}
            role="tab"
            aria-selected={active}
            className={`flex h-7 max-w-52 shrink-0 items-center gap-0.5 rounded-md border pl-1.5 pr-0.5 transition ${
              active
                ? "border-white/12 bg-white/8 text-slate-100"
                : "border-transparent text-slate-400 hover:bg-white/4 hover:text-slate-200"
            }`}
          >
            <button
              type="button"
              className="flex h-full min-w-0 flex-1 items-center gap-1.5 text-left"
              title={tab.title}
              onClick={() => onSelect(tab.id)}
            >
              <span className="shrink-0 text-[10px] text-current/70" aria-hidden>
                {KIND_ICON[tab.kind]}
              </span>
              <span className="truncate text-[11px] font-medium leading-none">
                {tab.title}
              </span>
            </button>
            {tab.closable ? (
              <button
                type="button"
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
