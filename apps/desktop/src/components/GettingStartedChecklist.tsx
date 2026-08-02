import type { ReactNode } from "react";

export type GettingStartedWin = {
  id: string;
  title: string;
  detail?: string;
  done: boolean;
  busy?: boolean;
  onClick: () => void;
  /** Optional right-side mark (Keys / Integrations deep-link affordance). */
  trailing?: ReactNode;
};

/**
 * ChatGPT-style Getting started checklist: progress header, N of M, thin bar,
 * pending / busy / done (strikethrough) rows.
 *
 * Trailing actions are sibling controls (not nested buttons) for a11y.
 */
export function GettingStartedChecklist({
  steps,
  className = "",
}: {
  steps: GettingStartedWin[];
  className?: string;
}) {
  const total = steps.length;
  const doneCount = steps.filter((step) => step.done).length;
  const progress = total === 0 ? 0 : doneCount / total;

  if (total === 0) return null;

  return (
    <section
      className={`rounded-2xl border border-line bg-surface-2/90 text-left shadow-panel ${className}`}
      data-testid="ade-getting-started"
    >
      <div className="flex items-center justify-between gap-3 px-4 pt-3.5">
        <h3 className="text-[13px] font-semibold text-ink">Getting started</h3>
        <span className="text-[11px] text-ink-faint">
          {doneCount} of {total} complete
        </span>
      </div>
      <div className="mx-4 mt-2.5 h-0.5 overflow-hidden rounded-full bg-white/8">
        <div
          className="h-full rounded-full bg-ink transition-[width] duration-300"
          style={{ width: `${progress * 100}%` }}
        />
      </div>
      <ul className="mt-1 px-1.5 pb-2">
        {steps.map((step) => (
          <li key={step.id}>
            <div className="flex w-full items-center gap-1 rounded-xl px-1 py-1 hover:bg-white/[0.03]">
              <button
                type="button"
                disabled={step.busy || step.done}
                onClick={step.onClick}
                className="flex min-w-0 flex-1 items-center gap-3 rounded-xl px-1.5 py-1.5 text-left transition disabled:hover:bg-transparent"
              >
                <span
                  className={`grid size-5 shrink-0 place-items-center rounded-full border text-[10px] ${
                    step.done
                      ? "border-ink bg-ink text-surface-0"
                      : step.busy
                        ? "border-line text-ink-faint"
                        : "border-line text-transparent"
                  }`}
                  aria-hidden
                >
                  {step.done ? "✓" : step.busy ? "…" : ""}
                </span>
                <span className="min-w-0 flex-1">
                  <span
                    className={`block text-[13px] font-medium ${
                      step.done ? "text-ink-faint line-through" : "text-ink"
                    }`}
                  >
                    {step.title}
                  </span>
                  {step.detail && !step.done && (
                    <span className="mt-0.5 block truncate text-[11px] text-ink-faint">
                      {step.detail}
                    </span>
                  )}
                </span>
              </button>
              {step.trailing ? (
                <span className="shrink-0 pr-1.5">{step.trailing}</span>
              ) : null}
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
