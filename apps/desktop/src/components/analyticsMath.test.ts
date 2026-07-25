import assert from "node:assert/strict";
import test from "node:test";
import { signedUsd, usd } from "../format";
import {
  attribute,
  dailyBuckets,
  dayLabel,
  filterWindow,
  summarize,
  type LedgerEntry,
} from "./analyticsMath";

function row(overrides: Partial<LedgerEntry> = {}): LedgerEntry {
  return {
    id: overrides.id ?? Math.random().toString(36).slice(2),
    created_at: "2026-07-24T12:00:00.000Z",
    status: "committed",
    scope: "workspace",
    period_key: "day:2026-07-24",
    provider: "anthropic",
    model: "claude-sonnet",
    reserved_usd: 0,
    actual_usd: 0,
    input_tokens: 0,
    output_tokens: 0,
    ...overrides,
  };
}

test("summarize keeps committed actuals and open reserves separate", () => {
  const stats = summarize([
    row({ status: "committed", reserved_usd: 0.5, actual_usd: 0.4 }),
    row({ status: "reserved", reserved_usd: 0.3 }),
  ]);

  assert.equal(stats.actual, 0.4);
  assert.equal(stats.openReserved, 0.3);
  assert.equal(stats.committedReserve, 0.5);
  // Open reserves must never leak into the invoice-class figure.
  assert.equal(stats.committed.length, 1);
  assert.equal(stats.open.length, 1);
  assert.equal(stats.turns, 2);
});

test("summarize delta is reserved minus actual on settled rows only", () => {
  const over = summarize([row({ reserved_usd: 1, actual_usd: 0.25 })]);
  assert.ok(over.delta > 0, "holding back more than spent is a positive delta");
  assert.equal(Number(over.delta.toFixed(4)), 0.75);

  const under = summarize([row({ reserved_usd: 0.1, actual_usd: 0.4 })]);
  assert.ok(under.delta < 0, "under-reserving is a negative delta");
});

test("summarize flags priced turns that report $0 actual", () => {
  const stats = summarize([
    row({ reserved_usd: 0.2, actual_usd: 0 }),
    row({ reserved_usd: 0.2, actual_usd: 0.19 }),
    // A $0 reserve with $0 actual is unmetered, not dishonest.
    row({ reserved_usd: 0, actual_usd: 0 }),
  ]);
  assert.equal(stats.zeroActualPriced, 1);
});

test("summarize on an empty window is all zeroes, not NaN", () => {
  const stats = summarize([]);
  assert.equal(stats.actual, 0);
  assert.equal(stats.delta, 0);
  assert.equal(stats.turns, 0);
  assert.equal(stats.zeroActualPriced, 0);
});

test("attribute groups by key and ranks by committed spend", () => {
  const rows = [
    row({ model: "fast", reserved_usd: 0.02, actual_usd: 0.01, input_tokens: 100 }),
    row({ model: "strong", reserved_usd: 0.9, actual_usd: 0.8, output_tokens: 500 }),
    row({ model: "strong", reserved_usd: 0.4, actual_usd: 0.3 }),
    row({ model: "strong", status: "reserved", reserved_usd: 0.5 }),
  ];
  const byModel = attribute(rows, (entry) => entry.model ?? "unknown");

  assert.deepEqual(
    byModel.map((entry) => entry.key),
    ["strong", "fast"],
  );
  const strong = byModel[0];
  assert.equal(strong.turns, 3);
  assert.equal(strong.committedTurns, 2);
  assert.equal(Number(strong.actual.toFixed(4)), 1.1);
  assert.equal(Number(strong.openReserved.toFixed(4)), 0.5);
  assert.equal(strong.tokensOut, 500);
});

test("attribute labels missing model or provider as unknown", () => {
  const byProvider = attribute([row({ provider: null })], (e) => e.provider ?? "unknown");
  assert.equal(byProvider[0].key, "unknown");
});

test("filterWindow keeps today only for the today window", () => {
  const now = new Date("2026-07-24T18:00:00");
  const rows = [
    row({ id: "today", created_at: new Date("2026-07-24T09:00:00").toISOString() }),
    row({ id: "yesterday", created_at: new Date("2026-07-23T09:00:00").toISOString() }),
  ];
  assert.deepEqual(
    filterWindow(rows, "today", now).map((r) => r.id),
    ["today"],
  );
  assert.deepEqual(
    filterWindow(rows, "7d", now).map((r) => r.id),
    ["today", "yesterday"],
  );
  assert.equal(filterWindow(rows, "all", now).length, 2);
});

test("filterWindow drops unparseable timestamps rather than guessing", () => {
  const now = new Date("2026-07-24T18:00:00");
  const rows = [row({ id: "bad", created_at: "not-a-date" })];
  assert.equal(filterWindow(rows, "7d", now).length, 0);
  // "all" does no time maths, so it keeps the row for the ledger table.
  assert.equal(filterWindow(rows, "all", now).length, 1);
});

test("dailyBuckets seeds quiet days and sorts ascending", () => {
  const now = new Date("2026-07-24T18:00:00");
  const buckets = dailyBuckets(
    [
      row({ created_at: new Date("2026-07-24T10:00:00").toISOString(), actual_usd: 0.5 }),
      row({
        created_at: new Date("2026-07-22T10:00:00").toISOString(),
        status: "reserved",
        reserved_usd: 0.2,
      }),
    ],
    3,
    now,
  );

  assert.equal(buckets.length, 3);
  assert.deepEqual(
    buckets.map((b) => b.key),
    ["2026-07-22", "2026-07-23", "2026-07-24"],
  );
  assert.equal(buckets[0].reserved, 0.2);
  assert.equal(buckets[1].actual, 0, "quiet day reads zero instead of vanishing");
  assert.equal(buckets[2].actual, 0.5);
});

test("dailyBuckets over the all window only emits days that have rows", () => {
  const buckets = dailyBuckets(
    [row({ created_at: new Date("2026-01-02T10:00:00").toISOString(), actual_usd: 1 })],
    null,
    new Date("2026-07-24T18:00:00"),
  );
  assert.equal(buckets.length, 1);
  assert.equal(buckets[0].key, "2026-01-02");
});

test("usd keeps sub-cent precision instead of rounding to $0.00", () => {
  assert.equal(usd(0), "$0.00");
  assert.equal(usd(0.0004), "$0.0004");
  assert.equal(usd(1.239), "$1.24");
});

test("signedUsd marks direction with a real minus sign", () => {
  assert.equal(signedUsd(0), "$0.00");
  assert.equal(signedUsd(0.5), "+$0.50");
  assert.equal(signedUsd(-0.5), "−$0.50");
});

test("dayLabel drops zero padding and survives junk keys", () => {
  assert.equal(dayLabel("2026-07-04"), "7/4");
  assert.equal(dayLabel("unknown"), "unknown");
});
