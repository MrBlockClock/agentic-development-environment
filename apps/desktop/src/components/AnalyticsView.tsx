import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke, isTauri } from "../ipc";
import {
  BarSeries,
  Chip,
  ChipRow,
  Disclosure,
  EmptyState,
  Legend,
  MetricCard,
  Panel,
  StatBar,
  TONE_FILL,
  type SeriesBar,
} from "./ui";
import { compactCount, signedUsd, usd } from "../format";
import {
  attribute,
  dailyBuckets,
  filterWindow,
  summarize,
  windowDays,
  WINDOWS,
  type Attribution,
  type LedgerEntry,
  type WindowId,
} from "./analyticsMath";

type SpendSummary = {
  daily_usd: number;
  used_usd: number;
  reserved_usd: number;
  remaining_usd: number;
  daily_cap_usd: number;
  session_cap_usd: number;
  period_key: string;
};

type VerifyResultLite = {
  gate: string;
  passed: boolean;
  status?: string;
};

type TaskLite = { status: string };

type HandoffLite = { turn_status: string | null };

const LEDGER_LIMIT = 200;

/**
 * Analytics (N1–N6): what the work cost and whether it worked.
 * Trust keeps the audit log; this surface owns trend, attribution, and honesty Δ.
 */
export function AnalyticsView({
  verifyResults,
  tasks,
  handoffs,
  onOpenSettings,
  onOpenTrust,
  onOpenVerify,
}: {
  verifyResults: VerifyResultLite[];
  tasks: TaskLite[];
  handoffs: HandoffLite[];
  onOpenSettings?: () => void;
  onOpenTrust?: () => void;
  onOpenVerify?: () => void;
}) {
  const [ledger, setLedger] = useState<LedgerEntry[] | null>(null);
  const [summary, setSummary] = useState<SpendSummary | null>(null);
  const [windowId, setWindowId] = useState<WindowId>("7d");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    if (!isTauri()) return;
    setLoading(true);
    setError(null);
    const sessionCap = Number(
      window.localStorage.getItem("ade_session_cap_usd") || "0",
    );
    const dailyCap = Number(window.localStorage.getItem("ade_daily_cap_usd") || "0");
    try {
      const [rows, spend] = await Promise.all([
        invoke<LedgerEntry[]>("spend_ledger_recent", { limit: LEDGER_LIMIT }),
        invoke<SpendSummary>("spend_summary", {
          sessionCapUsd: sessionCap > 0 ? sessionCap : null,
          dailyCapUsd: dailyCap > 0 ? dailyCap : null,
        }).catch(() => null),
      ]);
      setLedger(rows);
      setSummary(spend);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setLedger([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const windowed = useMemo(
    () => filterWindow(ledger ?? [], windowId),
    [ledger, windowId],
  );

  const stats = useMemo(() => summarize(windowed), [windowed]);

  const trend = useMemo<SeriesBar[]>(
    () =>
      dailyBuckets(windowed, windowDays(windowId)).map((bucket) => ({
        key: bucket.key,
        label: bucket.label,
        segments: [
          { value: bucket.reserved, tone: "warn" as const, label: "open reserve" },
          { value: bucket.actual, tone: "accent" as const, label: "committed" },
        ],
      })),
    [windowed, windowId],
  );

  const byModel = useMemo(
    () => attribute(windowed, (row) => row.model ?? "unknown"),
    [windowed],
  );
  const byProvider = useMemo(
    () => attribute(windowed, (row) => row.provider ?? "unknown"),
    [windowed],
  );

  const outcome = useMemo(() => {
    const gates = verifyResults.filter((r) => r.status !== "skipped");
    const gatesPassed = gates.filter((r) => r.passed).length;
    const completedTasks = tasks.filter((t) => t.status === "completed").length;
    const failedTasks = tasks.filter(
      (t) => t.status === "failed" || t.status === "cancelled",
    ).length;
    const turnStatus = handoffs.reduce<Record<string, number>>((acc, item) => {
      const key = item.turn_status ?? "unknown";
      acc[key] = (acc[key] ?? 0) + 1;
      return acc;
    }, {});
    const completedTurns = turnStatus.completed ?? 0;
    return {
      gates,
      gatesPassed,
      completedTasks,
      failedTasks,
      turnStatus,
      completedTurns,
    };
  }, [verifyResults, tasks, handoffs]);

  const costPerVerifiedTurn =
    outcome.completedTurns > 0 ? stats.actual / outcome.completedTurns : null;
  const costPerCompletedTask =
    outcome.completedTasks > 0 ? stats.actual / outcome.completedTasks : null;

  if (!isTauri()) {
    return (
      <div className="space-y-4">
        <AnalyticsHeader
          windowId={windowId}
          onWindow={setWindowId}
          rowCount={0}
          loading={false}
          onRefresh={() => {}}
        />
        <EmptyState
          title="Analytics needs Desktop"
          body="The usage ledger lives in the local ADE database and is read over IPC. Browser preview has no ledger access by design."
          actionLabel={onOpenTrust ? "Open Trust" : undefined}
          onAction={onOpenTrust}
        />
      </div>
    );
  }

  const noData = (ledger ?? []).length === 0;

  return (
    <div className="space-y-4" data-testid="ade-analytics">
      <AnalyticsHeader
        windowId={windowId}
        onWindow={setWindowId}
        rowCount={windowed.length}
        loading={loading}
        onRefresh={() => void load()}
      />

      {error ? (
        <p className="rounded-xl border border-red-400/25 bg-red-500/8 px-3 py-2 text-[11px] text-red-200">
          {error}
        </p>
      ) : null}

      {noData && !loading ? (
        <EmptyState
          title="No priced turns yet"
          body="Analytics aggregates the local usage ledger. Run an agent turn with a provider key and spend caps set, then come back — nothing is uploaded anywhere."
          actionLabel={onOpenSettings ? "Set spend caps" : undefined}
          onAction={onOpenSettings}
        />
      ) : (
        <>
          <div className="grid gap-2.5 sm:grid-cols-2 xl:grid-cols-3">
            <MetricCard
              testId="ade-metric-committed-spend"
              label="Committed spend"
              value={usd(stats.actual)}
              accent="blue"
              sub={`${stats.committed.length} settled turn${stats.committed.length === 1 ? "" : "s"}`}
              hint="Invoice-class actuals reported by the provider — not estimates."
            />
            <MetricCard
              testId="ade-metric-open-reserve"
              label="Open reserve"
              value={usd(stats.openReserved)}
              accent="amber"
              estimated={stats.openReserved > 0}
              sub={`${stats.open.length} turn${stats.open.length === 1 ? "" : "s"} not settled`}
              hint="Budget held for turns that have not reported usage yet. Estimated from message size."
            />
            <MetricCard
              testId="ade-metric-remaining-today"
              label="Remaining today"
              value={summary ? usd(summary.remaining_usd) : "—"}
              accent={
                summary && summary.remaining_usd <= 0
                  ? "red"
                  : summary && summary.remaining_usd < summary.daily_cap_usd * 0.25
                    ? "amber"
                    : "green"
              }
              sub={summary ? `of ${usd(summary.daily_cap_usd)} daily cap` : "no cap set"}
              hint="Daily cap minus used and reserved, straight from SpendGuard."
            />
            <MetricCard
              label="Tokens in / out"
              value={`${compactCount(stats.tokensIn)} / ${compactCount(stats.tokensOut)}`}
              accent="sky"
              sub={`${stats.turns} ledger row${stats.turns === 1 ? "" : "s"}`}
            />
            <MetricCard
              label="Cost per verified turn"
              value={costPerVerifiedTurn === null ? "—" : usd(costPerVerifiedTurn)}
              accent="violet"
              sub={
                outcome.completedTurns > 0
                  ? `${outcome.completedTurns} completed turn${outcome.completedTurns === 1 ? "" : "s"}`
                  : "no completed turns recorded"
              }
              hint="Committed spend divided by turns that ended completed. The number that survives model routing changes."
            />
            <MetricCard
              label="Cost per completed task"
              value={costPerCompletedTask === null ? "—" : usd(costPerCompletedTask)}
              accent="violet"
              sub={
                outcome.completedTasks > 0
                  ? `${outcome.completedTasks} task${outcome.completedTasks === 1 ? "" : "s"} done`
                  : "no completed tasks in queue"
              }
              hint="Optimise this, not raw token spend."
            />
          </div>

          <Panel
            title="Spend trend"
            subtitle="Committed actuals stacked under open reserves, by local day."
            actions={
              // The cap is labelled by the chart itself — as a line when it
              // fits the scale, as an off-scale note when it does not.
              <Legend
                items={[
                  { label: "committed", color: TONE_FILL.accent },
                  { label: "open reserve", color: TONE_FILL.warn },
                ]}
              />
            }
          >
            <BarSeries
              bars={trend}
              formatTotal={usd}
              emptyLabel="No priced turns in this window"
              reference={
                summary && summary.daily_cap_usd > 0
                  ? { value: summary.daily_cap_usd, label: "cap" }
                  : undefined
              }
            />
          </Panel>

          <div className="grid gap-3 lg:grid-cols-2">
            <Panel
              testId="ade-analytics-by-model"
              title="By model"
              subtitle="Committed spend share, with tokens and turn count."
              dense
            >
              <AttributionList rows={byModel} total={stats.actual} tone="accent" />
            </Panel>
            <Panel
              title="By provider"
              subtitle="Where the money actually went."
              dense
            >
              <AttributionList rows={byProvider} total={stats.actual} tone="info" />
            </Panel>
          </div>

          <Panel
            testId="ade-analytics-reserve-accuracy"
            title="Reserve accuracy (Δ)"
            subtitle="Reserved estimate minus committed actual. Positive means ADE held back more than the turn cost."
            dense
            actions={
              <span
                className={`rounded-md px-2 py-0.5 text-[10px] font-semibold ${
                  stats.zeroActualPriced > 0
                    ? "bg-red-500/15 text-red-200"
                    : "bg-emerald-500/12 text-emerald-200"
                }`}
                title="H1: a priced turn that reports $0 actual means provider usage went missing."
              >
                {stats.zeroActualPriced > 0
                  ? `${stats.zeroActualPriced} priced turn${stats.zeroActualPriced === 1 ? "" : "s"} reported $0`
                  : "no $0 priced turns"}
              </span>
            }
          >
            {stats.committed.length === 0 ? (
              <p className="py-4 text-center text-[11px] text-slate-600">
                No settled turns in this window — Δ needs a committed actual.
              </p>
            ) : (
              <>
                <div className="mb-3 flex flex-wrap items-baseline gap-x-4 gap-y-1 text-[11px] text-slate-500">
                  <span>
                    Window Δ{" "}
                    <span
                      className={
                        Math.abs(stats.delta) < 0.005
                          ? "font-semibold text-emerald-300"
                          : stats.delta > 0
                            ? "font-semibold text-amber-300"
                            : "font-semibold text-red-300"
                      }
                    >
                      {signedUsd(stats.delta)}
                    </span>
                  </span>
                  <span>
                    reserved {usd(stats.committedReserve)} → actual {usd(stats.actual)}
                  </span>
                  <span className="text-slate-600">
                    {stats.delta < 0
                      ? "Under-reserving: caps could be breached mid-turn."
                      : "Over-reserving throttles throughput before the cap is real."}
                  </span>
                </div>
                <DeltaTable rows={byModel.filter((row) => row.committedTurns > 0)} />
              </>
            )}
          </Panel>

          <Panel
            title="Outcome"
            subtitle="Verify is the truth signal — spend without a passing gate is not progress."
            dense
            actions={
              onOpenVerify ? (
                <button
                  type="button"
                  onClick={onOpenVerify}
                  className="rounded-lg border border-blue-400/25 bg-blue-500/10 px-2.5 py-1 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/20"
                >
                  Run gates
                </button>
              ) : undefined
            }
          >
            <div className="grid gap-2.5 sm:grid-cols-3">
              <MetricCard
                dense
                label="Gates passing"
                value={
                  outcome.gates.length === 0
                    ? "—"
                    : `${outcome.gatesPassed}/${outcome.gates.length}`
                }
                accent={
                  outcome.gates.length === 0
                    ? "slate"
                    : outcome.gatesPassed === outcome.gates.length
                      ? "green"
                      : "red"
                }
                sub={
                  outcome.gates.length === 0
                    ? "no verify run recorded"
                    : outcome.gates.map((g) => g.gate).join(" · ")
                }
              />
              <MetricCard
                dense
                label="Turns completed"
                value={String(outcome.completedTurns)}
                accent="green"
                sub={Object.entries(outcome.turnStatus)
                  .filter(([key]) => key !== "completed")
                  .map(([key, count]) => `${count} ${key}`)
                  .join(" · ")}
              />
              <MetricCard
                dense
                label="Tasks done"
                value={String(outcome.completedTasks)}
                accent={outcome.failedTasks > 0 ? "amber" : "green"}
                sub={
                  outcome.failedTasks > 0
                    ? `${outcome.failedTasks} failed or cancelled`
                    : "queue clean"
                }
              />
            </div>
          </Panel>

          <Disclosure
            title="Ledger rows"
            subtitle={`Most recent ${LEDGER_LIMIT} reserve/commit rows for this workspace. Trust keeps the full audit log.`}
            summary={`${windowed.length} in window`}
            storageKey="ade_analytics_ledger_open"
          >
            <LedgerTable rows={windowed} />
            <div className="mt-3 flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => {
                  void navigator.clipboard
                    ?.writeText(JSON.stringify(windowed, null, 2))
                    .catch(() => {});
                }}
                className="rounded-lg border border-white/10 bg-white/4 px-2.5 py-1 text-[11px] text-slate-300 hover:bg-white/8"
              >
                Copy JSON
              </button>
              {onOpenTrust ? (
                <button
                  type="button"
                  onClick={onOpenTrust}
                  className="rounded-lg border border-white/10 bg-white/4 px-2.5 py-1 text-[11px] text-slate-300 hover:bg-white/8"
                >
                  Audit log in Trust →
                </button>
              ) : null}
            </div>
          </Disclosure>
        </>
      )}
    </div>
  );
}

function AnalyticsHeader({
  windowId,
  onWindow,
  rowCount,
  loading,
  onRefresh,
}: {
  windowId: WindowId;
  onWindow: (id: WindowId) => void;
  rowCount: number;
  loading: boolean;
  onRefresh: () => void;
}) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-3">
      <ChipRow
        label="Window"
        hint="Aggregated from the most recent ledger rows held locally — not a billing statement."
      >
        {WINDOWS.map((w) => (
          <Chip key={w.id} active={w.id === windowId} onClick={() => onWindow(w.id)}>
            {w.label}
          </Chip>
        ))}
      </ChipRow>
      <div className="flex items-center gap-2 text-[10px] text-slate-600">
        <span>
          {loading ? "loading…" : `${rowCount} row${rowCount === 1 ? "" : "s"}`}
        </span>
        <button
          type="button"
          onClick={onRefresh}
          className="rounded-lg border border-white/10 bg-white/4 px-2.5 py-1 text-[11px] text-slate-300 hover:bg-white/8"
        >
          Refresh
        </button>
      </div>
    </div>
  );
}

function AttributionList({
  rows,
  total,
  tone,
}: {
  rows: Attribution[];
  total: number;
  tone: "accent" | "info";
}) {
  if (rows.length === 0) {
    return (
      <p className="py-4 text-center text-[11px] text-slate-600">
        Nothing attributed in this window.
      </p>
    );
  }
  const max = Math.max(...rows.map((row) => row.actual), 0.0001);
  return (
    <div className="space-y-3">
      {rows.map((row) => {
        const share = total > 0 ? (row.actual / total) * 100 : 0;
        return (
          <StatBar
            key={row.key}
            testId="ade-stat-row"
            label={row.key}
            value={row.actual}
            max={max}
            tone={tone}
            right={`${usd(row.actual)} · ${share.toFixed(0)}%`}
            sub={`${row.turns} turn${row.turns === 1 ? "" : "s"} · ${compactCount(
              row.tokensIn,
            )} in / ${compactCount(row.tokensOut)} out${
              row.openReserved > 0 ? ` · ${usd(row.openReserved)} still reserved` : ""
            }`}
          />
        );
      })}
    </div>
  );
}

function DeltaTable({ rows }: { rows: Attribution[] }) {
  return (
    <div className="thin-scrollbar overflow-x-auto">
      <table className="w-full min-w-[30rem] text-left text-[11px]">
        <thead className="text-[10px] uppercase tracking-wider text-slate-600">
          <tr>
            <th className="pb-1.5 pr-3 font-medium">Model</th>
            <th className="pb-1.5 pr-3 text-right font-medium">Turns</th>
            <th className="pb-1.5 pr-3 text-right font-medium">Reserved</th>
            <th className="pb-1.5 pr-3 text-right font-medium">Actual</th>
            <th className="pb-1.5 pr-3 text-right font-medium">Δ</th>
            <th className="pb-1.5 font-medium">Bias</th>
          </tr>
        </thead>
        <tbody className="text-slate-300">
          {rows.map((row) => {
            const delta = row.reserved - row.actual;
            const ratio = row.actual > 0 ? delta / row.actual : delta > 0 ? 1 : 0;
            const clamped = Math.max(-1, Math.min(1, ratio));
            return (
              <tr key={row.key} className="border-t border-line">
                <td className="py-1.5 pr-3">
                  <span className="flex items-center gap-1.5">
                    <span className="min-w-0 truncate">{row.key}</span>
                    {row.zeroActualPriced > 0 ? (
                      <span
                        className="rounded bg-red-500/15 px-1 text-[9px] font-semibold text-red-200"
                        title={`${row.zeroActualPriced} committed turn(s) reported $0 despite a reserve`}
                      >
                        $0
                      </span>
                    ) : null}
                  </span>
                </td>
                <td className="py-1.5 pr-3 text-right tabular-nums">
                  {row.committedTurns}
                </td>
                <td className="py-1.5 pr-3 text-right tabular-nums text-amber-300/80">
                  {usd(row.reserved)}
                </td>
                <td className="py-1.5 pr-3 text-right tabular-nums text-blue-200">
                  {usd(row.actual)}
                </td>
                <td
                  className={`py-1.5 pr-3 text-right tabular-nums ${
                    Math.abs(delta) < 0.005
                      ? "text-emerald-300"
                      : delta > 0
                        ? "text-amber-300"
                        : "text-red-300"
                  }`}
                >
                  {signedUsd(delta)}
                </td>
                <td className="py-1.5">
                  <div className="relative h-1.5 w-24 rounded-full bg-white/6">
                    <span className="absolute inset-y-0 left-1/2 w-px bg-white/20" />
                    <span
                      className="absolute inset-y-0 rounded-full"
                      style={{
                        left: clamped >= 0 ? "50%" : `${50 + clamped * 50}%`,
                        width: `${Math.abs(clamped) * 50}%`,
                        background: clamped >= 0 ? TONE_FILL.warn : TONE_FILL.danger,
                      }}
                    />
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function LedgerTable({ rows }: { rows: LedgerEntry[] }) {
  if (rows.length === 0) {
    return (
      <p className="py-3 text-center text-[11px] text-slate-600">
        No ledger rows in this window.
      </p>
    );
  }
  return (
    <div className="thin-scrollbar max-h-[42vh] overflow-auto">
      <table className="w-full min-w-[34rem] text-left text-[11px]">
        <thead className="sticky top-0 bg-surface-2 text-[10px] uppercase tracking-wider text-slate-600">
          <tr>
            <th className="py-1.5 pr-3 font-medium">When</th>
            <th className="py-1.5 pr-3 font-medium">Status</th>
            <th className="py-1.5 pr-3 font-medium">Model</th>
            <th className="py-1.5 pr-3 text-right font-medium">Reserved</th>
            <th className="py-1.5 pr-3 text-right font-medium">Actual</th>
            <th className="py-1.5 pr-3 text-right font-medium">Δ</th>
            <th className="py-1.5 text-right font-medium">Tokens</th>
          </tr>
        </thead>
        <tbody className="text-slate-300">
          {rows.map((row) => {
            const settled = row.status === "committed";
            const delta = settled ? row.reserved_usd - row.actual_usd : null;
            return (
              <tr key={row.id} className="border-t border-line">
                <td className="py-1.5 pr-3 whitespace-nowrap text-slate-500">
                  {new Date(row.created_at).toLocaleString(undefined, {
                    month: "numeric",
                    day: "numeric",
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                </td>
                <td className="py-1.5 pr-3">
                  <span
                    className={`rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase ${
                      settled
                        ? "bg-blue-500/15 text-blue-200"
                        : "bg-amber-500/15 text-amber-200"
                    }`}
                  >
                    {row.status}
                  </span>
                </td>
                <td className="py-1.5 pr-3">
                  <span className="min-w-0 truncate">{row.model ?? "—"}</span>
                  {row.provider ? (
                    <span className="ml-1 text-slate-600">· {row.provider}</span>
                  ) : null}
                </td>
                <td className="py-1.5 pr-3 text-right tabular-nums text-amber-300/80">
                  {usd(row.reserved_usd)}
                </td>
                <td className="py-1.5 pr-3 text-right tabular-nums">
                  {settled ? usd(row.actual_usd) : "—"}
                </td>
                <td
                  className={`py-1.5 pr-3 text-right tabular-nums ${
                    delta === null
                      ? "text-slate-600"
                      : Math.abs(delta) < 0.005
                        ? "text-emerald-300"
                        : delta > 0
                          ? "text-amber-300"
                          : "text-red-300"
                  }`}
                >
                  {delta === null ? "open" : signedUsd(delta)}
                </td>
                <td className="py-1.5 text-right tabular-nums text-slate-500">
                  {compactCount(row.input_tokens)}/{compactCount(row.output_tokens)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
