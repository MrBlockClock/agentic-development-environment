import { useMemo, useState } from "react";

export type PlanPhase = {
  id: string;
  title: string;
  owned_paths: string[];
  gates: string[];
  depends_on?: string[];
};

export type PlanReport = {
  phases: PlanPhase[];
  requires_human: string[];
  score_before?: number;
  score_max?: number;
  audit_root?: string;
};

type VerifyResult = {
  gate: string;
  passed: boolean;
};

type Station = "AUDIT" | "PLAN" | "EXECUTE" | "VERIFY";

function layoutPhases(phases: PlanPhase[]): Map<string, { x: number; y: number }> {
  const positions = new Map<string, { x: number; y: number }>();
  const depth = new Map<string, number>();
  const byId = new Map(phases.map((p) => [p.id, p]));

  const depthOf = (id: string, stack: Set<string>): number => {
    if (depth.has(id)) return depth.get(id)!;
    if (stack.has(id)) return 0;
    stack.add(id);
    const phase = byId.get(id);
    const deps = phase?.depends_on ?? [];
    const d =
      deps.length === 0 ? 0 : Math.max(...deps.map((dep) => depthOf(dep, stack))) + 1;
    stack.delete(id);
    depth.set(id, d);
    return d;
  };

  for (const phase of phases) {
    depthOf(phase.id, new Set());
  }

  const columns = new Map<number, string[]>();
  for (const phase of phases) {
    const d = depth.get(phase.id) ?? 0;
    const list = columns.get(d) ?? [];
    list.push(phase.id);
    columns.set(d, list);
  }

  const colWidth = 200;
  const rowHeight = 88;
  for (const [d, ids] of columns) {
    ids.forEach((id, index) => {
      positions.set(id, {
        x: 24 + d * colWidth,
        y: 24 + index * rowHeight,
      });
    });
  }
  return positions;
}

/** ADE Trust Route — AUDIT→PLAN→EXECUTE→VERIFY with phase DAG. */
export function PlanMap({
  plan,
  scorePercent,
  verifyResults,
  executing,
  onExecute,
  onRunAudit,
  onRunVerify,
}: {
  plan: PlanReport;
  scorePercent: number;
  verifyResults: VerifyResult[];
  executing: boolean;
  onExecute: () => void;
  onRunAudit?: () => void;
  onRunVerify?: () => void;
}) {
  const [selectedId, setSelectedId] = useState<string | null>(plan.phases[0]?.id ?? null);

  const station: Station = useMemo(() => {
    if (verifyResults.length > 0) return "VERIFY";
    if (executing) return "EXECUTE";
    if (plan.phases.length > 0) return "PLAN";
    return "AUDIT";
  }, [plan.phases.length, verifyResults.length, executing]);

  const positions = useMemo(() => layoutPhases(plan.phases), [plan.phases]);
  const selected = plan.phases.find((p) => p.id === selectedId);

  const width = Math.max(
    480,
    ...[...positions.values()].map((p) => p.x + 180),
    24,
  );
  const height = Math.max(
    200,
    ...[...positions.values()].map((p) => p.y + 72),
    24,
  );

  const stations: Station[] = ["AUDIT", "PLAN", "EXECUTE", "VERIFY"];
  const stationIndex = stations.indexOf(station);

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-slate-100">Trust Route</h2>
          <p className="text-[11px] text-slate-500">
            AUDIT → PLAN → EXECUTE → VERIFY · owned paths stay visible
          </p>
        </div>
        <div className="text-[11px] text-slate-400">
          Score{" "}
          <span className="font-semibold text-slate-200">
            {plan.score_before != null && plan.score_max != null
              ? `${plan.score_before}/${plan.score_max}`
              : `${scorePercent}%`}
          </span>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-1.5">
        {stations.map((name, index) => {
          const active = index <= stationIndex;
          const current = name === station;
          return (
            <div key={name} className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => {
                  if (name === "AUDIT") onRunAudit?.();
                  if (name === "VERIFY") onRunVerify?.();
                }}
                className={`rounded-md px-3 py-1.5 text-[11px] font-semibold tracking-wide ${
                  current
                    ? "bg-blue-500/25 text-blue-100 ring-1 ring-blue-400/40"
                    : active
                      ? "bg-white/8 text-slate-200"
                      : "bg-white/3 text-slate-600"
                }`}
              >
                {name}
              </button>
              {index < stations.length - 1 && (
                <span className="text-slate-700">→</span>
              )}
            </div>
          );
        })}
      </div>

      {plan.phases.length === 0 ? (
        <div className="rounded-2xl border border-white/7 bg-[#0d121a]/85 py-16 text-center text-sm text-slate-500">
          No remediation phases. Run Audit to refresh, or the workspace is clear.
        </div>
      ) : (
        <div className="grid gap-4 lg:grid-cols-[1fr_280px]">
          <div className="thin-scrollbar overflow-auto rounded-2xl border border-white/7 bg-[#0a0e14] p-2">
            <svg width={width} height={height} className="block min-w-full">
              {plan.phases.flatMap((phase) =>
                (phase.depends_on ?? []).map((dep) => {
                  const from = positions.get(dep);
                  const to = positions.get(phase.id);
                  if (!from || !to) return null;
                  return (
                    <line
                      key={`${dep}->${phase.id}`}
                      x1={from.x + 160}
                      y1={from.y + 28}
                      x2={to.x}
                      y2={to.y + 28}
                      stroke="rgba(148,163,184,0.35)"
                      strokeWidth={1.5}
                      markerEnd="url(#arrow)"
                    />
                  );
                }),
              )}
              <defs>
                <marker
                  id="arrow"
                  markerWidth="6"
                  markerHeight="6"
                  refX="5"
                  refY="3"
                  orient="auto"
                >
                  <path d="M0,0 L6,3 L0,6 Z" fill="rgba(148,163,184,0.5)" />
                </marker>
              </defs>
              {plan.phases.map((phase) => {
                const pos = positions.get(phase.id) ?? { x: 0, y: 0 };
                const isBlocker = phase.id.includes("blocker");
                const selected = phase.id === selectedId;
                return (
                  <g
                    key={phase.id}
                    transform={`translate(${pos.x}, ${pos.y})`}
                    onClick={() => setSelectedId(phase.id)}
                    style={{ cursor: "pointer" }}
                  >
                    <rect
                      width={168}
                      height={64}
                      rx={10}
                      fill={
                        selected
                          ? "rgba(59,130,246,0.2)"
                          : isBlocker
                            ? "rgba(251,191,36,0.12)"
                            : "rgba(255,255,255,0.04)"
                      }
                      stroke={
                        selected
                          ? "rgba(96,165,250,0.7)"
                          : isBlocker
                            ? "rgba(251,191,36,0.45)"
                            : "rgba(255,255,255,0.1)"
                      }
                    />
                    <text
                      x={10}
                      y={22}
                      className="fill-slate-100"
                      style={{ fontSize: 11, fontWeight: 600 }}
                    >
                      {phase.title.length > 22
                        ? `${phase.title.slice(0, 22)}…`
                        : phase.title}
                    </text>
                    <text x={10} y={40} style={{ fontSize: 9, fill: "#94a3b8" }}>
                      {phase.owned_paths.length} path
                      {phase.owned_paths.length === 1 ? "" : "s"} · {phase.gates.length}{" "}
                      gate
                      {phase.gates.length === 1 ? "" : "s"}
                    </text>
                  </g>
                );
              })}
            </svg>
          </div>

          <aside className="rounded-2xl border border-white/7 bg-[#0d121a]/85 p-4 text-sm">
            {selected ? (
              <div className="space-y-3">
                <div>
                  <div className="text-[10px] uppercase tracking-wider text-slate-600">
                    Phase
                  </div>
                  <div className="mt-1 font-semibold text-slate-100">{selected.title}</div>
                  <div className="mt-0.5 font-mono text-[10px] text-slate-500">
                    {selected.id}
                  </div>
                </div>
                <div>
                  <div className="text-[10px] uppercase tracking-wider text-slate-600">
                    Owned paths
                  </div>
                  <ul className="mt-1 space-y-1">
                    {selected.owned_paths.length === 0 && (
                      <li className="text-[11px] text-slate-500">None listed</li>
                    )}
                    {selected.owned_paths.map((path) => (
                      <li
                        key={path}
                        className="rounded bg-white/5 px-2 py-1 font-mono text-[10px] text-slate-300"
                      >
                        {path}
                      </li>
                    ))}
                  </ul>
                </div>
                <div>
                  <div className="text-[10px] uppercase tracking-wider text-slate-600">
                    Gates
                  </div>
                  <div className="mt-1 flex flex-wrap gap-1">
                    {selected.gates.map((gate) => (
                      <span
                        key={gate}
                        className="rounded bg-blue-500/10 px-1.5 py-0.5 text-[10px] text-blue-200"
                      >
                        {gate}
                      </span>
                    ))}
                  </div>
                </div>
                {plan.requires_human.length > 0 && (
                  <div>
                    <div className="text-[10px] uppercase tracking-wider text-amber-600/80">
                      Human
                    </div>
                    <ul className="mt-1 space-y-1 text-[11px] text-amber-100/80">
                      {plan.requires_human.slice(0, 4).map((item) => (
                        <li key={item}>{item}</li>
                      ))}
                    </ul>
                  </div>
                )}
                <button
                  type="button"
                  onClick={onExecute}
                  disabled={executing}
                  className="w-full rounded-lg bg-violet-500/90 py-2 text-xs font-semibold hover:bg-violet-400 disabled:opacity-50"
                >
                  {executing ? "Executing…" : "Approve and execute"}
                </button>
              </div>
            ) : (
              <p className="text-xs text-slate-500">Select a phase on the map.</p>
            )}
          </aside>
        </div>
      )}
    </div>
  );
}
