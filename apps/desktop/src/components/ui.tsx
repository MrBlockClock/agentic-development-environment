import { useEffect, useId, useState, type ReactNode } from "react";

/** Hover / focus / tap hint — works on desktop and touch. */
export function Hint({
  text,
  label = "More info",
  className = "",
  children,
}: {
  text: string;
  label?: string;
  className?: string;
  children?: ReactNode;
}) {
  return (
    <span
      className={`ade-hint group relative inline-flex max-w-full items-center align-middle ${className}`}
    >
      {children ?? (
        <button
          type="button"
          aria-label={label}
          className="inline-grid size-4 place-items-center rounded-full border border-white/15 bg-white/5 text-[9px] font-semibold text-slate-400 hover:border-blue-400/40 hover:text-blue-200"
        >
          ?
        </button>
      )}
      <span role="tooltip" className="ade-hint-bubble">
        {text}
      </span>
    </span>
  );
}

type DisclosureProps = {
  title: string;
  subtitle?: string;
  /** Collapsed header chip — count, status, or key value */
  summary?: string;
  hint?: string;
  defaultOpen?: boolean;
  storageKey?: string;
  forceOpen?: boolean;
  lockedOpen?: boolean;
  className?: string;
  children: ReactNode;
};

/**
 * Progressive disclosure: one tap/click expands detail.
 * Same control for mobile and desktop.
 */
export function Disclosure({
  title,
  subtitle,
  summary,
  hint,
  defaultOpen = false,
  storageKey,
  forceOpen = false,
  lockedOpen = false,
  className = "",
  children,
}: DisclosureProps) {
  const panelId = useId();
  const [open, setOpen] = useState(() => {
    if (forceOpen || lockedOpen) return true;
    if (storageKey && typeof window !== "undefined") {
      const stored = window.localStorage.getItem(storageKey);
      if (stored === "1") return true;
      if (stored === "0") return false;
    }
    return defaultOpen;
  });

  useEffect(() => {
    if (forceOpen || lockedOpen) setOpen(true);
  }, [forceOpen, lockedOpen]);

  useEffect(() => {
    if (!storageKey || lockedOpen) return;
    window.localStorage.setItem(storageKey, open ? "1" : "0");
  }, [open, storageKey, lockedOpen]);

  const expanded = lockedOpen ? true : open;

  return (
    <section
      className={`rounded-2xl border border-white/7 bg-[#0d121a]/85 shadow-[0_12px_45px_rgba(0,0,0,0.15)] ${className}`}
    >
      <button
        type="button"
        aria-expanded={expanded}
        aria-controls={panelId}
        disabled={lockedOpen}
        onClick={() => {
          if (!lockedOpen) setOpen((current) => !current);
        }}
        className="flex w-full items-start gap-3 px-4 py-3 text-left disabled:cursor-default sm:px-5 sm:py-3.5"
      >
        <span
          className={`mt-0.5 shrink-0 text-[10px] text-slate-500 transition ${
            expanded ? "rotate-90" : ""
          }`}
          aria-hidden
        >
          ▸
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-semibold text-slate-100">{title}</span>
            {hint ? <Hint text={hint} /> : null}
            {!expanded && summary ? (
              <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] font-medium text-slate-400">
                {summary}
              </span>
            ) : null}
          </span>
          {expanded && subtitle ? (
            <span className="mt-1 block text-[11px] leading-4 text-slate-600">{subtitle}</span>
          ) : null}
          {!expanded && !summary && subtitle ? (
            <span className="mt-1 block truncate text-[11px] leading-4 text-slate-600">
              {subtitle}
            </span>
          ) : null}
        </span>
        {!lockedOpen && (
          <span className="shrink-0 text-[10px] font-semibold uppercase tracking-wide text-slate-500">
            {expanded ? "Hide" : "Show"}
          </span>
        )}
      </button>
      {expanded ? (
        <div
          id={panelId}
          className="border-t border-white/6 px-4 pb-4 pt-3 sm:px-5 sm:pb-4 sm:pt-3"
        >
          {children}
        </div>
      ) : null}
    </section>
  );
}

export function ChipRow({
  label,
  hint,
  children,
}: {
  label?: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      {(label || hint) && (
        <div className="flex items-center gap-2 text-[10px] uppercase tracking-wider text-slate-500">
          {label ? <span>{label}</span> : null}
          {hint ? <Hint text={hint} /> : null}
        </div>
      )}
      <div className="flex flex-wrap gap-1.5">{children}</div>
    </div>
  );
}

export function Chip({
  active,
  onClick,
  children,
  title,
}: {
  active?: boolean;
  onClick: () => void;
  children: ReactNode;
  title?: string;
}) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      className={`rounded-lg border px-2.5 py-1.5 text-[11px] font-medium transition ${
        active
          ? "border-blue-400/40 bg-blue-500/20 text-blue-100"
          : "border-white/8 bg-white/3 text-slate-400 hover:border-white/15 hover:text-slate-200"
      }`}
    >
      {children}
    </button>
  );
}
