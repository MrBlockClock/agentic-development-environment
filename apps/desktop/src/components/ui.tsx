import { useEffect, useId, useMemo, useState, type ReactNode } from "react";

/** Semantic tone shared by panels, metrics, bars, and Atlas nodes. */
export type Tone =
  | "neutral"
  | "accent"
  | "ready"
  | "warn"
  | "danger"
  | "info"
  | "authority";

export const TONE_TEXT: Record<Tone, string> = {
  neutral: "text-slate-300",
  accent: "text-blue-300",
  ready: "text-emerald-300",
  warn: "text-amber-300",
  danger: "text-red-300",
  info: "text-sky-300",
  authority: "text-violet-300",
};

export const TONE_FILL: Record<Tone, string> = {
  neutral: "#475569",
  accent: "#58a6ff",
  ready: "#34d399",
  warn: "#fbbf24",
  danger: "#f87171",
  info: "#38bdf8",
  authority: "#a78bfa",
};

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
            <span className="text-sm font-semibold text-slate-100">
              {title}
            </span>
            {hint ? <Hint text={hint} /> : null}
            {!expanded && summary ? (
              <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] font-medium text-slate-400">
                {summary}
              </span>
            ) : null}
          </span>
          {expanded && subtitle ? (
            <span className="mt-1 block text-[11px] leading-4 text-slate-600">
              {subtitle}
            </span>
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

/** Standard panel chrome. `actions` renders right-aligned in the header row. */
export function Panel({
  title,
  subtitle,
  actions,
  dense = false,
  className = "",
  testId,
  children,
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  dense?: boolean;
  className?: string;
  testId?: string;
  children: ReactNode;
}) {
  return (
    <section
      data-testid={testId}
      className={`rounded-2xl border border-line bg-surface-2/85 shadow-panel ${
        dense ? "p-4" : "p-5"
      } ${className}`}
    >
      <div
        className={`flex items-start gap-3 ${
          subtitle ? (dense ? "mb-3" : "mb-5") : dense ? "mb-3" : "mb-4"
        }`}
      >
        <div className="min-w-0 flex-1">
          <h2 className="text-sm font-semibold">{title}</h2>
          {subtitle ? (
            <p className="mt-0.5 text-[11px] text-slate-600">{subtitle}</p>
          ) : null}
        </div>
        {actions ? <div className="shrink-0">{actions}</div> : null}
      </div>
      {children}
    </section>
  );
}

/**
 * Single figure. `estimated` marks numbers derived from a reserve rather than a
 * committed actual — never show an estimate as an exact-looking dollar.
 */
export function MetricCard({
  label,
  value,
  accent,
  sub,
  hint,
  estimated = false,
  dense = false,
  testId,
}: {
  label: string;
  value: string;
  accent: "blue" | "green" | "red" | "violet" | "slate" | "amber" | "sky";
  sub?: string;
  hint?: string;
  estimated?: boolean;
  dense?: boolean;
  testId?: string;
}) {
  const colors: Record<string, string> = {
    blue: TONE_TEXT.accent,
    green: TONE_TEXT.ready,
    red: TONE_TEXT.danger,
    violet: TONE_TEXT.authority,
    slate: TONE_TEXT.neutral,
    amber: TONE_TEXT.warn,
    sky: TONE_TEXT.info,
  };
  return (
    <div
      data-testid={testId}
      className={`rounded-xl border border-line bg-surface-2/80 ${
        dense ? "px-3 py-2.5" : "px-4 py-4"
      }`}
    >
      <div className="flex items-center gap-1.5 text-[10px] uppercase tracking-[0.12em] text-slate-600">
        <span className="min-w-0 truncate">{label}</span>
        {hint ? <Hint text={hint} /> : null}
      </div>
      <div className="mt-1 flex items-baseline gap-1.5">
        <span
          className={`font-semibold ${dense ? "text-lg" : "text-xl"} ${colors[accent]}`}
        >
          {value}
        </span>
        {estimated ? (
          <span
            className="rounded bg-amber-400/12 px-1 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-amber-300/90"
            title="Estimated from an open reserve, not a committed actual"
          >
            est
          </span>
        ) : null}
      </div>
      {sub ? (
        <div className="mt-0.5 text-[10px] text-slate-600">{sub}</div>
      ) : null}
    </div>
  );
}

export type SubTabItem = {
  id: string;
  label: string;
  badge?: string;
  hint?: string;
};

/** Segmented sub-navigation inside a single nav destination. */
export function SubTabs({
  items,
  activeId,
  onSelect,
  className = "",
  ariaLabel = "Sections",
}: {
  items: SubTabItem[];
  activeId: string;
  onSelect: (id: string) => void;
  className?: string;
  ariaLabel?: string;
}) {
  return (
    <div
      role="tablist"
      aria-label={ariaLabel}
      className={`flex flex-wrap items-center gap-0.5 rounded-xl border border-line bg-surface-2/70 p-1 ${className}`}
    >
      {items.map((item) => {
        const active = item.id === activeId;
        return (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={active}
            title={item.hint}
            onClick={() => onSelect(item.id)}
            className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition ${
              active
                ? "bg-blue-500/18 text-blue-100"
                : "text-slate-500 hover:bg-white/4 hover:text-slate-200"
            }`}
          >
            <span>{item.label}</span>
            {item.badge ? (
              <span
                className={`rounded px-1 py-0.5 text-[9px] font-semibold ${
                  active
                    ? "bg-blue-400/20 text-blue-100"
                    : "bg-white/6 text-slate-500"
                }`}
              >
                {item.badge}
              </span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
}

export type BarSegment = { value: number; tone: Tone; label?: string };
export type SeriesBar = { key: string; label: string; segments: BarSegment[] };

/**
 * Stacked column series (inline SVG — no charting dependency).
 * Bars keep source order; empty windows render an honest baseline.
 */
export function BarSeries({
  bars,
  height = 120,
  formatTotal,
  emptyLabel = "No data in this window",
  reference,
}: {
  bars: SeriesBar[];
  height?: number;
  formatTotal?: (total: number) => string;
  emptyLabel?: string;
  /** Optional horizontal reference line, e.g. a daily cap. */
  reference?: { value: number; label: string };
}) {
  const totals = bars.map((bar) =>
    bar.segments.reduce((sum, segment) => sum + Math.max(segment.value, 0), 0),
  );
  const dataPeak = Math.max(...totals, 0);

  if (bars.length === 0 || dataPeak <= 0) {
    return (
      <p className="py-6 text-center text-[11px] text-slate-600">
        {emptyLabel}
      </p>
    );
  }

  /*
   * A cap that dwarfs actual usage must not set the scale, or a real $1 day
   * renders as a 2px sliver. Keep the line when it is close enough to frame the
   * data; otherwise plot to the data and say the cap is off scale.
   */
  const refFitsScale = reference ? reference.value <= dataPeak * 1.6 : false;
  const peak = refFitsScale
    ? Math.max(dataPeak, reference?.value ?? 0)
    : dataPeak;
  const refRatio = reference && refFitsScale ? reference.value / peak : null;
  const scaleLabel = formatTotal ? formatTotal(peak) : String(peak);

  return (
    <div>
      <div className="mb-1 flex items-baseline justify-between text-[9px] text-slate-600">
        <span>peak {scaleLabel}</span>
        {reference && !refFitsScale ? (
          <span className="text-amber-300/60">
            {reference.label}{" "}
            {formatTotal ? formatTotal(reference.value) : reference.value} —
            above this range
          </span>
        ) : null}
      </div>
      <div className="relative flex items-end gap-1" style={{ height }}>
        {refRatio !== null ? (
          <div
            className="pointer-events-none absolute inset-x-0 border-t border-dashed border-amber-400/40"
            style={{ bottom: `${refRatio * 100}%` }}
          >
            <span className="absolute right-0 -top-4 text-[9px] text-amber-300/70">
              {reference?.label}
            </span>
          </div>
        ) : null}
        {bars.map((bar, index) => {
          const total = totals[index];
          return (
            <div
              key={bar.key}
              className="group relative flex min-w-0 flex-1 flex-col justify-end"
              style={{ height: "100%" }}
              title={`${bar.label} · ${formatTotal ? formatTotal(total) : total}`}
            >
              {bar.segments
                .filter((segment) => segment.value > 0)
                .map((segment, segmentIndex) => (
                  <div
                    key={`${bar.key}-${segmentIndex}`}
                    style={{
                      height: `${(segment.value / peak) * 100}%`,
                      background: TONE_FILL[segment.tone],
                      opacity: segment.tone === "warn" ? 0.55 : 0.85,
                    }}
                    className={segmentIndex === 0 ? "rounded-t-sm" : ""}
                  />
                ))}
              {total === 0 ? <div className="h-px w-full bg-white/10" /> : null}
            </div>
          );
        })}
      </div>
      <div className="mt-1.5 flex gap-1">
        {bars.map((bar) => (
          <div
            key={`${bar.key}-label`}
            className="min-w-0 flex-1 truncate text-center text-[9px] text-slate-600"
          >
            {bar.label}
          </div>
        ))}
      </div>
    </div>
  );
}

/** Horizontal share bar for attribution rows. */
export function StatBar({
  label,
  value,
  max,
  tone = "accent",
  right,
  sub,
  testId,
}: {
  label: string;
  value: number;
  max: number;
  tone?: Tone;
  right?: string;
  sub?: string;
  testId?: string;
}) {
  const ratio = max > 0 ? Math.min(Math.max(value / max, 0), 1) : 0;
  return (
    <div data-testid={testId} className="space-y-1">
      <div className="flex items-baseline gap-2 text-[11px]">
        <span className="min-w-0 flex-1 truncate text-slate-300">{label}</span>
        {right ? (
          <span className={`shrink-0 font-medium ${TONE_TEXT[tone]}`}>
            {right}
          </span>
        ) : null}
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-white/6">
        <div
          className="h-full rounded-full"
          style={{ width: `${ratio * 100}%`, background: TONE_FILL[tone] }}
        />
      </div>
      {sub ? <div className="text-[10px] text-slate-600">{sub}</div> : null}
    </div>
  );
}

/** Honest empty state: what is missing and the one action that fixes it. */
export function EmptyState({
  title,
  body,
  actionLabel,
  onAction,
}: {
  title: string;
  body: string;
  actionLabel?: string;
  onAction?: () => void;
}) {
  return (
    <div className="rounded-xl border border-dashed border-line-strong/60 px-4 py-6 text-center">
      <p className="text-xs font-semibold text-slate-300">{title}</p>
      <p className="mx-auto mt-1 max-w-sm text-[11px] leading-4 text-slate-600">
        {body}
      </p>
      {actionLabel && onAction ? (
        <button
          type="button"
          onClick={onAction}
          className="mt-3 rounded-lg border border-blue-400/25 bg-blue-500/10 px-3 py-1.5 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/20"
        >
          {actionLabel}
        </button>
      ) : null}
    </div>
  );
}

export type LegendItem = {
  label: string;
  color: string;
  /** `line` renders a stroke sample instead of a filled swatch. */
  shape?: "swatch" | "line";
  dashed?: boolean;
};

export function Legend({
  items,
  label,
  className = "",
}: {
  items: LegendItem[];
  label?: string;
  className?: string;
}) {
  const rendered = useMemo(() => items.filter((item) => item.label), [items]);
  return (
    <div className={`flex flex-wrap items-center gap-x-3 gap-y-1 ${className}`}>
      {label ? (
        <span className="text-[10px] uppercase tracking-wider text-slate-600">
          {label}
        </span>
      ) : null}
      {rendered.map((item) => (
        <span
          key={item.label}
          className="flex items-center gap-1.5 text-[10px] text-slate-500"
        >
          {item.shape === "line" ? (
            <span
              className="h-0 w-3.5"
              style={{
                borderTop: `${item.dashed ? "1px dashed" : "1.5px solid"} ${item.color}`,
              }}
            />
          ) : (
            <span
              className="size-2 rounded-sm"
              style={{ background: item.color }}
            />
          )}
          {item.label}
        </span>
      ))}
    </div>
  );
}
