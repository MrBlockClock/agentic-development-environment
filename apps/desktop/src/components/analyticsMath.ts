/**
 * Pure aggregation for the Analytics surface.
 *
 * Kept out of the component so the money math is testable without Tauri: the
 * ledger is Desktop-only, so browser/e2e smoke can never cover it. Committed
 * actuals and open reserves are never summed into one "spend" figure — the H1
 * spend-honesty contract depends on keeping them distinct.
 */

/** One reserve/commit row from the usage ledger (`spend_ledger_recent`). */
export type LedgerEntry = {
  id: string;
  created_at: string;
  status: string;
  scope: string;
  period_key: string;
  provider: string | null;
  model: string | null;
  reserved_usd: number;
  actual_usd: number;
  input_tokens: number;
  output_tokens: number;
};

export type WindowId = "today" | "7d" | "30d" | "all";

export const WINDOWS: { id: WindowId; label: string; days: number | null }[] = [
  { id: "today", label: "Today", days: 1 },
  { id: "7d", label: "7 days", days: 7 },
  { id: "30d", label: "30 days", days: 30 },
  { id: "all", label: "All", days: null },
];

export function windowDays(id: WindowId): number | null {
  return WINDOWS.find((entry) => entry.id === id)?.days ?? null;
}

export function dayKey(iso: string | Date): string {
  const date = iso instanceof Date ? iso : new Date(iso);
  if (Number.isNaN(date.getTime())) return "unknown";
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(
    date.getDate(),
  ).padStart(2, "0")}`;
}

export function dayLabel(key: string): string {
  const [, month, day] = key.split("-");
  if (!month || !day) return key;
  return `${Number(month)}/${Number(day)}`;
}

/** Local-day window, inclusive of today. Unparseable timestamps are dropped. */
export function filterWindow(
  rows: LedgerEntry[],
  id: WindowId,
  now: Date = new Date(),
): LedgerEntry[] {
  const days = windowDays(id);
  if (days === null) return rows;
  const cutoff = new Date(now);
  cutoff.setHours(0, 0, 0, 0);
  cutoff.setDate(cutoff.getDate() - (days - 1));
  return rows.filter((row) => {
    const at = new Date(row.created_at);
    return !Number.isNaN(at.getTime()) && at >= cutoff;
  });
}

export type WindowStats = {
  committed: LedgerEntry[];
  open: LedgerEntry[];
  /** Committed actuals — invoice class. */
  actual: number;
  /** Reserves still open (estimates). */
  openReserved: number;
  /** Reserves belonging to rows that have since settled. */
  committedReserve: number;
  /** committedReserve − actual. Positive = ADE held back more than it spent. */
  delta: number;
  tokensIn: number;
  tokensOut: number;
  turns: number;
  /** H1 detector: settled rows priced at $0 despite a non-zero reserve. */
  zeroActualPriced: number;
};

export function summarize(rows: LedgerEntry[]): WindowStats {
  const committed = rows.filter((row) => row.status === "committed");
  const open = rows.filter((row) => row.status === "reserved");
  const actual = committed.reduce((sum, row) => sum + row.actual_usd, 0);
  const openReserved = open.reduce((sum, row) => sum + row.reserved_usd, 0);
  const committedReserve = committed.reduce((sum, row) => sum + row.reserved_usd, 0);
  return {
    committed,
    open,
    actual,
    openReserved,
    committedReserve,
    delta: committedReserve - actual,
    tokensIn: rows.reduce((sum, row) => sum + row.input_tokens, 0),
    tokensOut: rows.reduce((sum, row) => sum + row.output_tokens, 0),
    turns: rows.length,
    zeroActualPriced: committed.filter(
      (row) => row.actual_usd === 0 && row.reserved_usd > 0,
    ).length,
  };
}

export type Attribution = {
  key: string;
  turns: number;
  committedTurns: number;
  actual: number;
  reserved: number;
  openReserved: number;
  tokensIn: number;
  tokensOut: number;
  zeroActualPriced: number;
};

/** Group by model or provider, ranked by committed spend. */
export function attribute(
  rows: LedgerEntry[],
  pick: (entry: LedgerEntry) => string,
): Attribution[] {
  const map = new Map<string, Attribution>();
  for (const row of rows) {
    const key = pick(row) || "unknown";
    const bucket =
      map.get(key) ??
      ({
        key,
        turns: 0,
        committedTurns: 0,
        actual: 0,
        reserved: 0,
        openReserved: 0,
        tokensIn: 0,
        tokensOut: 0,
        zeroActualPriced: 0,
      } satisfies Attribution);
    bucket.turns += 1;
    bucket.reserved += row.reserved_usd;
    bucket.tokensIn += row.input_tokens;
    bucket.tokensOut += row.output_tokens;
    if (row.status === "committed") {
      bucket.committedTurns += 1;
      bucket.actual += row.actual_usd;
      if (row.actual_usd === 0 && row.reserved_usd > 0) {
        bucket.zeroActualPriced += 1;
      }
    } else if (row.status === "reserved") {
      bucket.openReserved += row.reserved_usd;
    }
    map.set(key, bucket);
  }
  return [...map.values()].sort((a, b) => b.actual - a.actual || b.turns - a.turns);
}

export type DayBucket = { key: string; label: string; actual: number; reserved: number };

/**
 * Per-local-day totals, ascending. A bounded window seeds every day so quiet
 * days read as zero instead of vanishing from the trend.
 */
export function dailyBuckets(
  rows: LedgerEntry[],
  days: number | null,
  now: Date = new Date(),
): DayBucket[] {
  const buckets = new Map<string, { actual: number; reserved: number }>();
  if (days !== null && days > 1) {
    for (let offset = days - 1; offset >= 0; offset -= 1) {
      const date = new Date(now);
      date.setHours(0, 0, 0, 0);
      date.setDate(date.getDate() - offset);
      buckets.set(dayKey(date), { actual: 0, reserved: 0 });
    }
  }
  for (const row of rows) {
    const key = dayKey(row.created_at);
    const bucket = buckets.get(key) ?? { actual: 0, reserved: 0 };
    if (row.status === "committed") bucket.actual += row.actual_usd;
    else if (row.status === "reserved") bucket.reserved += row.reserved_usd;
    buckets.set(key, bucket);
  }
  return [...buckets.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => ({ key, label: dayLabel(key), ...value }));
}
