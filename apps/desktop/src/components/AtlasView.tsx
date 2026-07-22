import { useEffect, useMemo, useState } from "react";
import { invoke } from "../ipc";
import { GraphCanvas } from "./GraphCanvas";

type RuleFile = {
  source: string;
  description: string;
  globs: string[];
  deny_writes: boolean;
  content: string;
  scope?: string;
};

type SkillFile = {
  name: string;
  description: string;
  always_apply: boolean;
  body: string;
  source: string;
  scope?: string;
};

type Finding = {
  layer: string;
  severity: string;
  detail: string;
  points: number;
  points_max: number;
};

type PlanPhase = {
  id: string;
  title: string;
  owned_paths: string[];
  gates: string[];
  depends_on?: string[];
};

type HandoffHistoryItem = {
  id: string;
  turn_status: string | null;
};

type LayerMode = "authority" | "work" | "both";

type AtlasNode = {
  id: string;
  label: string;
  kind: string;
  layer: "authority" | "work";
  scope?: string;
  x: number;
  y: number;
  preview: string;
  jump?: "guidance" | "plan";
  phaseId?: string;
};

type AtlasEdge = {
  from: string;
  to: string;
  kind: string;
};

/** Obsidian-like ADE Atlas — Authority + Work layers with pan/zoom (N5). */
export function AtlasView({
  auditFindings,
  planPhases,
  verifyGates,
  handoffs,
  focusNodeId = null,
  onOpenGuidance,
  onOpenPlan,
}: {
  auditFindings: Finding[];
  planPhases: PlanPhase[];
  verifyGates: string[];
  handoffs: HandoffHistoryItem[];
  focusNodeId?: string | null;
  onOpenGuidance?: () => void;
  onOpenPlan?: (phaseId?: string) => void;
}) {
  const [rules, setRules] = useState<RuleFile[]>([]);
  const [skills, setSkills] = useState<SkillFile[]>([]);
  const [activeProfile, setActiveProfile] = useState<string | null>(null);
  const [layer, setLayer] = useState<LayerMode>("both");
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>("hub-workspace");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [nextRules, nextSkills, nextActive] = await Promise.all([
          invoke<RuleFile[]>("list_rules"),
          invoke<SkillFile[]>("list_skills"),
          invoke<string | null>("get_active_guidance_profile").catch(() => null),
        ]);
        setRules(nextRules);
        setSkills(nextSkills);
        setActiveProfile(nextActive);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    })();
  }, []);

  useEffect(() => {
    if (focusNodeId) setSelectedId(focusNodeId);
  }, [focusNodeId]);

  const { nodes, edges } = useMemo(() => {
    const nodes: AtlasNode[] = [];
    const edges: AtlasEdge[] = [];

    nodes.push({
      id: "hub-global",
      label: "Global",
      kind: "hub",
      layer: "authority",
      scope: "global",
      x: 40,
      y: 40,
      preview: "Machine ADE guidance home",
      jump: "guidance",
    });
    nodes.push({
      id: "hub-workspace",
      label: "Workspace",
      kind: "hub",
      layer: "authority",
      scope: "workspace",
      x: 280,
      y: 40,
      preview: "Checkout .ade + AGENTS.md",
      jump: "guidance",
    });
    nodes.push({
      id: "agents-md",
      label: "AGENTS.md",
      kind: "contract",
      layer: "authority",
      scope: "workspace",
      x: 280,
      y: 110,
      preview: "Canonical workspace contract",
      jump: "guidance",
    });
    edges.push({ from: "hub-workspace", to: "agents-md", kind: "contains" });

    nodes.push({
      id: "profile",
      label: activeProfile ? `Profile · ${activeProfile}` : "Profile · all packs",
      kind: "profile",
      layer: "authority",
      scope: "global",
      x: 40,
      y: 110,
      preview: activeProfile
        ? `Active profile "${activeProfile}" filters pack-tagged guidance.`
        : "No active profile — all packs load. Set one in Guidance.",
      jump: "guidance",
    });
    edges.push({ from: "hub-global", to: "profile", kind: "activates" });

    rules.slice(0, 12).forEach((rule, index) => {
      const id = `rule:${rule.source}`;
      const global = rule.scope === "global";
      nodes.push({
        id,
        label: rule.source.split(/[\\/]/).pop() ?? rule.source,
        kind: "rule",
        layer: "authority",
        scope: global ? "global" : "workspace",
        x: global ? 40 : 280,
        y: 180 + index * 36,
        preview: `${rule.description || "(no description)"}\n\n${rule.content.slice(0, 800)}`,
        jump: "guidance",
      });
      edges.push({
        from: global ? "hub-global" : "hub-workspace",
        to: id,
        kind: rule.deny_writes ? "denies_write" : "contains",
      });
    });

    skills.slice(0, 12).forEach((skill, index) => {
      const id = `skill:${skill.name}`;
      const global = skill.scope === "global";
      nodes.push({
        id,
        label: skill.name,
        kind: "skill",
        layer: "authority",
        scope: global ? "global" : "workspace",
        x: global ? 140 : 420,
        y: 180 + index * 36,
        preview: `${skill.description}\n\n${skill.body.slice(0, 800)}`,
        jump: "guidance",
      });
      edges.push({
        from: global ? "hub-global" : "hub-workspace",
        to: id,
        kind: "contains",
      });
    });

    nodes.push({
      id: "hub-audit",
      label: "Audit",
      kind: "hub",
      layer: "work",
      x: 40,
      y: 520,
      preview: "L0–L11 workspace findings",
    });
    nodes.push({
      id: "hub-plan",
      label: "Plan",
      kind: "hub",
      layer: "work",
      x: 280,
      y: 520,
      preview: "Plan Map phases",
      jump: "plan",
    });
    edges.push({ from: "hub-audit", to: "hub-plan", kind: "derived_from" });

    auditFindings.slice(0, 8).forEach((finding, index) => {
      const id = `finding:${finding.layer}`;
      nodes.push({
        id,
        label: finding.layer,
        kind: "finding",
        layer: "work",
        x: 40,
        y: 580 + index * 32,
        preview: `${finding.severity} · ${finding.points}/${finding.points_max}\n${finding.detail}`,
      });
      edges.push({ from: "hub-audit", to: id, kind: "contains" });
    });

    planPhases.slice(0, 8).forEach((phase, index) => {
      const id = `phase:${phase.id}`;
      nodes.push({
        id,
        label: phase.title.slice(0, 28),
        kind: "phase",
        layer: "work",
        x: 280,
        y: 580 + index * 36,
        preview: `Paths: ${phase.owned_paths.join(", ") || "—"}\nGates: ${phase.gates.join(", ")}`,
        jump: "plan",
        phaseId: phase.id,
      });
      edges.push({ from: "hub-plan", to: id, kind: "contains" });
      for (const dep of phase.depends_on ?? []) {
        edges.push({ from: `phase:${dep}`, to: id, kind: "depends_on" });
      }
      for (const path of phase.owned_paths.slice(0, 2)) {
        const denyRule = rules.find(
          (r) => r.deny_writes && r.globs.some((g) => path.includes(g.replace("*", ""))),
        );
        if (denyRule) {
          edges.push({
            from: `rule:${denyRule.source}`,
            to: id,
            kind: "denies_write",
          });
        }
      }
    });

    verifyGates.slice(0, 6).forEach((gate, index) => {
      const id = `gate:${gate}`;
      nodes.push({
        id,
        label: gate,
        kind: "gate",
        layer: "work",
        x: 520,
        y: 580 + index * 32,
        preview: `Verify gate ${gate}`,
        jump: "plan",
      });
      edges.push({ from: "hub-plan", to: id, kind: "verified_by" });
    });

    handoffs.slice(0, 4).forEach((item, index) => {
      const id = `handoff:${item.id}`;
      nodes.push({
        id,
        label: item.id.slice(0, 12),
        kind: "handoff",
        layer: "work",
        x: 520,
        y: 520 + index * 28,
        preview: `Handoff ${item.id}\n${item.turn_status ?? ""}`,
      });
      edges.push({ from: "hub-plan", to: id, kind: "contains" });
    });

    return { nodes, edges };
  }, [rules, skills, activeProfile, auditFindings, planPhases, verifyGates, handoffs]);

  const q = query.trim().toLowerCase();
  const visibleNodes = nodes.filter((node) => {
    if (layer === "authority" && node.layer !== "authority") return false;
    if (layer === "work" && node.layer !== "work") return false;
    if (!q) return true;
    return (
      node.label.toLowerCase().includes(q) ||
      node.kind.includes(q) ||
      node.preview.toLowerCase().includes(q)
    );
  });
  const visibleIds = new Set(visibleNodes.map((n) => n.id));
  const visibleEdges = edges.filter(
    (e) => visibleIds.has(e.from) && visibleIds.has(e.to),
  );
  const selected = nodes.find((n) => n.id === selectedId);

  const width = Math.max(640, ...visibleNodes.map((n) => n.x + 140), 140);
  const height = Math.max(400, ...visibleNodes.map((n) => n.y + 40), 40);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="text-sm font-semibold text-slate-100">Atlas</h2>
        <nav className="flex items-center gap-1 text-[11px] text-slate-500">
          <button
            type="button"
            onClick={onOpenGuidance}
            className="text-blue-300/90 hover:text-blue-200"
          >
            Guidance
          </button>
          <span aria-hidden>→</span>
          <span className="text-slate-300">Atlas</span>
          <span aria-hidden>→</span>
          <button
            type="button"
            onClick={() => onOpenPlan?.()}
            className="text-blue-300/90 hover:text-blue-200"
          >
            Plan Map
          </button>
        </nav>
        <span
          className="rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-[10px] text-slate-400"
          title="Change in Guidance"
        >
          {activeProfile ? `profile:${activeProfile}` : "profile:all"}
        </span>
        <div className="flex rounded-lg border border-white/10 p-0.5 text-[11px]">
          {(["both", "authority", "work"] as LayerMode[]).map((mode) => (
            <button
              key={mode}
              type="button"
              onClick={() => setLayer(mode)}
              className={`rounded-md px-2.5 py-1 capitalize ${
                layer === mode
                  ? "bg-blue-500/20 text-blue-100"
                  : "text-slate-500 hover:text-slate-300"
              }`}
            >
              {mode}
            </button>
          ))}
        </div>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Filter nodes…"
          className="rounded-lg border border-white/10 bg-[#101620] px-2.5 py-1 text-[11px] text-slate-200"
        />
      </div>

      {error && <p className="text-xs text-red-400">{error}</p>}

      <div className="grid min-h-0 flex-1 gap-3 lg:grid-cols-[1fr_300px]">
        <GraphCanvas
          contentWidth={width}
          contentHeight={height}
          className="min-h-[360px] rounded-2xl border border-white/7 bg-[#080b11]"
        >
          {visibleEdges.map((edge) => {
            const from = nodes.find((n) => n.id === edge.from);
            const to = nodes.find((n) => n.id === edge.to);
            if (!from || !to) return null;
            return (
              <line
                key={`${edge.from}-${edge.to}-${edge.kind}`}
                x1={from.x + 50}
                y1={from.y + 12}
                x2={to.x + 50}
                y2={to.y + 12}
                stroke={
                  edge.kind === "denies_write"
                    ? "rgba(251,191,36,0.45)"
                    : "rgba(100,116,139,0.35)"
                }
                strokeWidth={1}
              />
            );
          })}
          {visibleNodes.map((node) => {
            const active = node.id === selectedId;
            const fill =
              node.scope === "global"
                ? "rgba(167,139,250,0.15)"
                : node.layer === "work"
                  ? "rgba(56,189,248,0.12)"
                  : "rgba(255,255,255,0.05)";
            return (
              <g
                key={node.id}
                data-graph-node=""
                transform={`translate(${node.x}, ${node.y})`}
                onClick={() => setSelectedId(node.id)}
                style={{ cursor: "pointer" }}
              >
                <rect
                  width={110}
                  height={26}
                  rx={6}
                  fill={active ? "rgba(59,130,246,0.25)" : fill}
                  stroke={active ? "rgba(96,165,250,0.8)" : "rgba(255,255,255,0.12)"}
                />
                <text
                  x={8}
                  y={17}
                  style={{ fontSize: 10, fill: "#e2e8f0", fontWeight: 500 }}
                >
                  {node.label.length > 14 ? `${node.label.slice(0, 14)}…` : node.label}
                </text>
              </g>
            );
          })}
        </GraphCanvas>

        <aside className="rounded-2xl border border-white/7 bg-[#0d121a]/85 p-4">
          {selected ? (
            <div className="space-y-2">
              <div className="text-[10px] uppercase tracking-wider text-slate-600">
                {selected.kind}
                {selected.scope ? ` · ${selected.scope}` : ""}
              </div>
              <div className="text-sm font-semibold text-slate-100">{selected.label}</div>
              <pre className="thin-scrollbar max-h-[40vh] overflow-auto whitespace-pre-wrap rounded-lg bg-black/30 p-3 text-[11px] leading-5 text-slate-300">
                {selected.preview}
              </pre>
              {selected.jump === "guidance" && (
                <button
                  type="button"
                  onClick={onOpenGuidance}
                  className="w-full rounded-lg border border-blue-400/25 bg-blue-500/10 py-2 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/20"
                >
                  Open in Guidance →
                </button>
              )}
              {selected.jump === "plan" && (
                <button
                  type="button"
                  onClick={() => onOpenPlan?.(selected.phaseId)}
                  className="w-full rounded-lg border border-blue-400/25 bg-blue-500/10 py-2 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/20"
                >
                  Open in Plan Map →
                </button>
              )}
            </div>
          ) : (
            <p className="text-xs text-slate-500">Select a node.</p>
          )}
        </aside>
      </div>
    </div>
  );
}
