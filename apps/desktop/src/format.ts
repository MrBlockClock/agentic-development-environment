/**
 * Display formatting shared by every surface that shows money or counts.
 *
 * Spend appears in Trust, Analytics, the composer, and the audit log; when each
 * of those rolled its own `toFixed`, the same $8.97 rendered as `$8.9700` in one
 * place and `$8.97` in another. One formatter keeps them agreeing.
 */

/**
 * Cents by default, four decimals only when the value is smaller than a cent —
 * per-turn costs are often sub-cent and would otherwise all read `$0.00`.
 */
export function usd(value: number): string {
  if (value === 0) return "$0.00";
  if (Math.abs(value) < 0.01) return `$${value.toFixed(4)}`;
  return `$${value.toFixed(2)}`;
}

/** Explicit sign, for deltas where direction is the point. */
export function signedUsd(value: number): string {
  const sign = value > 0 ? "+" : value < 0 ? "−" : "";
  return `${sign}${usd(Math.abs(value))}`;
}

export function compactCount(value: number): string {
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${(value / 1000).toFixed(1)}k`;
  return `${(value / 1_000_000).toFixed(1)}M`;
}
