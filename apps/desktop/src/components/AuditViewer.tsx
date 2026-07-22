import { useEffect, useState, type ReactNode } from "react";
import { invoke, isTauri } from "../ipc";
import { Disclosure } from "./ui";

type HandoffHistoryItem = {
  id: string;
  created_at: string | null;
  turn_status: string | null;
  score_before: number | null;
  score_after: number | null;
  score_max: number | null;
  score_delta: number | null;
  context_status: string | null;
  context_tokens: number | null;
};

type LedgerRow = {
  id: string;
  created_at: string;
  status: string;
  scope: string;
  period_key: string;
  provider: string | null;
  model: string | null;
  actual_usd: number;
  reserved_usd: number;
  input_tokens: number;
  output_tokens: number;
};

type AuditViewerProps = {
  handoffs: HandoffHistoryItem[];
};

/** Recent Trust activity: handoff capsules + spend ledger rows, exportable JSON. */
export function AuditViewer({ handoffs }: AuditViewerProps): ReactNode {
  const [rows, setRows] = useState<LedgerRow[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke<LedgerRow[]>("spend_ledger_recent", { limit: 40 })
      .then(setRows)
      .catch((reason) => setError(String(reason)));
  }, [handoffs.length]);

  const exportJson = () => {
    const payload = {
      exported_at: new Date().toISOString(),
      handoffs,
      ledger: rows,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `ade-trust-audit-${Date.now()}.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="rounded-xl border border-white/8 bg-[#0d121a]/85 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold text-slate-100">Audit log</h3>
          <p className="mt-0.5 text-[12px] text-slate-500">
            Recent handoffs and spend ledger entries for this workspace
          </p>
        </div>
        <button
          type="button"
          onClick={exportJson}
          className="rounded-md border border-white/10 px-2.5 py-1.5 text-[11px] font-semibold text-slate-300 hover:bg-white/5"
        >
          Export JSON
        </button>
      </div>

      {error && <div className="mt-2 text-[11px] text-amber-200/90">{error}</div>}

      <Disclosure
        title="Handoff capsules"
        summary={`${handoffs.length}`}
        storageKey="ade-trust-handoff-log"
        defaultOpen
      >
        <div className="mt-2 space-y-1.5">
          {handoffs.length === 0 && (
            <div className="text-[11px] text-slate-600">No capsules yet.</div>
          )}
          {handoffs.slice(0, 20).map((item) => (
            <div
              key={item.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-white/6 bg-black/20 px-3 py-2"
            >
              <div className="min-w-0">
                <div className="font-mono text-[11px] text-slate-200">{item.id}</div>
                <div className="text-[10px] text-slate-500">
                  {item.created_at ?? "—"} · {item.turn_status ?? "unknown"}
                </div>
              </div>
              <div className="text-right text-[10px] text-slate-400">
                {item.score_before != null && item.score_after != null
                  ? `${item.score_before} → ${item.score_after}`
                  : "—"}
                {item.score_delta != null && (
                  <span className="ml-1 text-slate-500">
                    ({item.score_delta >= 0 ? "+" : ""}
                    {item.score_delta})
                  </span>
                )}
              </div>
            </div>
          ))}
        </div>
      </Disclosure>

      <Disclosure
        title="Spend ledger"
        summary={`${rows.length}`}
        storageKey="ade-trust-spend-log"
        className="mt-3"
      >
        <div className="mt-2 space-y-1.5">
          {rows.length === 0 && (
            <div className="text-[11px] text-slate-600">No ledger rows yet.</div>
          )}
          {rows.map((row) => (
            <div
              key={row.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-white/6 bg-black/20 px-3 py-2"
            >
              <div className="min-w-0">
                <div className="font-mono text-[11px] text-slate-200">
                  {row.provider ?? "—"}/{row.model ?? "—"}
                </div>
                <div className="text-[10px] text-slate-500">
                  {row.created_at} · {row.status} · {row.scope}/{row.period_key}
                </div>
              </div>
              <div className="text-right font-mono text-[10px] text-slate-400">
                ${row.actual_usd.toFixed(4)}
                <div className="text-slate-600">
                  {row.input_tokens} in / {row.output_tokens} out
                </div>
              </div>
            </div>
          ))}
        </div>
      </Disclosure>
    </div>
  );
}
