import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { invoke } from "../ipc";
import { GraphCanvas } from "./GraphCanvas";
import { Chip, Legend, TONE_FILL, type Tone } from "./ui";

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

type VerifyGate = { gate: string; passed: boolean; status?: string };

type HandoffHistoryItem = {
  id: string;
  turn_status: string | null;
};

type LayerMode = "authority" | "work" | "both";
type ScopeMode = "all" | "global" | "workspace";
type ViewMode = "focus" | "map";

type NodeKind =
  | "hub"
  | "contract"
  | "profile"
  | "rule"
  | "skill"
  | "finding"
  | "phase"
  | "gate"
  | "handoff";

type EdgeKind =
  | "contains"
  | "activates"
  | "denies_write"
  | "derived_from"
  | "depends_on"
  | "verified_by";

type AtlasNode = {
  id: string;
  label: string;
  kind: NodeKind;
  layer: "authority" | "work";
  scope?: "global" | "workspace";
  preview: string;
  /** Second line inside the node — severity, gate result, path count. */
  meta?: string;
  tone: Tone;
  jump?: "guidance" | "plan";
  phaseId?: string;
};

type AtlasEdge = { from: string; to: string; kind: EdgeKind };

type PlacedNode = AtlasNode & { x: number; y: number; w: number; h: number };

const KIND_GLYPH: Record<NodeKind, string> = {
  hub: "◉",
  contract: "▤",
  profile: "⚑",
  rule: "☰",
  skill: "✦",
  finding: "⚠",
  phase: "◇",
  gate: "✓",
  handoff: "⇄",
};

const KIND_LABEL: Record<NodeKind, string> = {
  hub: "Hub",
  contract: "Contract",
  profile: "Profile",
  rule: "Rule",
  skill: "Skill",
  finding: "Finding",
  phase: "Phase",
  gate: "Gate",
  handoff: "Handoff",
};

const KIND_ORDER: NodeKind[] = [
  "hub",
  "contract",
  "profile",
  "rule",
  "skill",
  "finding",
  "phase",
  "gate",
  "handoff",
];

const EDGE_STYLE: Record<EdgeKind, { stroke: string; dash?: string; label: string }> = {
  contains: { stroke: "rgba(100,116,139,0.4)", label: "contains" },
  activates: { stroke: "rgba(167,139,250,0.5)", label: "activates" },
  denies_write: { stroke: TONE_FILL.warn, dash: "4 3", label: "denies write" },
  derived_from: { stroke: "rgba(148,163,184,0.4)", dash: "2 3", label: "derived from" },
  depends_on: { stroke: TONE_FILL.accent, label: "depends on" },
  verified_by: { stroke: TONE_FILL.ready, label: "verified by" },
};

const NODE_H = 34;
const NODE_MIN_W = 96;
const NODE_MAX_W = 216;
const PAD = 40;

function nodeWidth(node: AtlasNode): number {
  const label = node.label.length * 6.4 + 30;
  const meta = (node.meta?.length ?? 0) * 5.2 + 30;
  return Math.round(Math.min(NODE_MAX_W, Math.max(NODE_MIN_W, Math.max(label, meta))));
}

function severityTone(severity: string): Tone {
  const value = severity.toLowerCase();
  if (value.includes("block") || value.includes("critical")) return "danger";
  if (value.includes("high")) return "danger";
  if (value.includes("medium") || value.includes("warn")) return "warn";
  return "neutral";
}

function scopeFill(node: AtlasNode): string {
  if (node.layer === "work") return "rgba(56,189,248,0.10)";
  return node.scope === "global"
    ? "rgba(167,139,250,0.14)"
    : "rgba(96,165,250,0.10)";
}

/** Boundary point on a node rect facing (tx, ty) — keeps edges out from under labels. */
function edgePoint(node: PlacedNode, tx: number, ty: number) {
  const cx = node.x + node.w / 2;
  const cy = node.y + node.h / 2;
  const dx = tx - cx;
  const dy = ty - cy;
  if (dx === 0 && dy === 0) return { x: cx, y: cy };
  const halfW = node.w / 2 + 2;
  const halfH = node.h / 2 + 2;
  const factor = Math.min(
    dx === 0 ? Number.POSITIVE_INFINITY : halfW / Math.abs(dx),
    dy === 0 ? Number.POSITIVE_INFINITY : halfH / Math.abs(dy),
  );
  return { x: cx + dx * factor, y: cy + dy * factor };
}

/**
 * ADE Atlas — Authority + Work as a *local* graph.
 *
 * Research (Obsidian graph-view teardown, Excalibrain, Sourcegraph): a global
 * force graph is decorative past a couple hundred nodes, layout that moves on
 * every reload cannot be learned, and the useful pattern is focus node + depth
 * + neighbour traversal + a side panel that keeps the graph in view. So: focus
 * mode by default, deterministic layout, keyboard traversal, and Map reserved
 * for structural review (including orphans).
 */
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
  verifyGates: VerifyGate[];
  handoffs: HandoffHistoryItem[];
  focusNodeId?: string | null;
  onOpenGuidance?: () => void;
  onOpenPlan?: (phaseId?: string) => void;
}) {
  const [rules, setRules] = useState<RuleFile[]>([]);
  const [skills, setSkills] = useState<SkillFile[]>([]);
  const [activeProfile, setActiveProfile] = useState<string | null>(null);
  const [layer, setLayer] = useState<LayerMode>("both");
  const [scopeMode, setScopeMode] = useState<ScopeMode>("all");
  const [mutedKinds, setMutedKinds] = useState<Set<NodeKind>>(new Set());
  const [query, setQuery] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>("focus");
  const [depth, setDepth] = useState(1);
  const [orphansOnly, setOrphansOnly] = useState(false);
  const [focusId, setFocusId] = useState("hub-workspace");
  const [selectedId, setSelectedId] = useState("hub-workspace");
  const [fitToken, setFitToken] = useState(1);
  const [error, setError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);

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
    if (!focusNodeId) return;
    setFocusId(focusNodeId);
    setSelectedId(focusNodeId);
    setViewMode("focus");
  }, [focusNodeId]);

  // ── Graph model: everything, no silent slicing ────────────────────────────
  const { nodes, edges } = useMemo(() => {
    const nodes: AtlasNode[] = [];
    const edges: AtlasEdge[] = [];

    nodes.push({
      id: "hub-global",
      label: "Global",
      kind: "hub",
      layer: "authority",
      scope: "global",
      meta: "machine guidance",
      tone: "authority",
      preview:
        "Machine ADE guidance home. Personal denies and skills that follow you across every workspace.",
      jump: "guidance",
    });
    nodes.push({
      id: "hub-workspace",
      label: "Workspace",
      kind: "hub",
      layer: "authority",
      scope: "workspace",
      meta: "this checkout",
      tone: "accent",
      preview:
        "This checkout's .ade/ plus AGENTS.md. Workspace bodies replace global on conflict; deny-writes union across both.",
      jump: "guidance",
    });
    nodes.push({
      id: "agents-md",
      label: "AGENTS.md",
      kind: "contract",
      layer: "authority",
      scope: "workspace",
      tone: "accent",
      preview: "Canonical workspace contract — authority order, golden path, security.",
      jump: "guidance",
    });
    edges.push({ from: "hub-workspace", to: "agents-md", kind: "contains" });

    nodes.push({
      id: "profile",
      label: activeProfile ?? "all packs",
      kind: "profile",
      layer: "authority",
      scope: "global",
      meta: activeProfile ? "active profile" : "no filter",
      tone: activeProfile ? "authority" : "neutral",
      preview: activeProfile
        ? `Active profile "${activeProfile}" keeps items whose pack is listed, plus every untagged item. Untagged deny rules always load.`
        : "No active profile — all packs load. Set one in Guidance.",
      jump: "guidance",
    });
    edges.push({ from: "hub-global", to: "profile", kind: "activates" });

    for (const rule of rules) {
      const id = `rule:${rule.source}`;
      const global = rule.scope === "global";
      nodes.push({
        id,
        label: rule.source.split(/[\\/]/).pop() ?? rule.source,
        kind: "rule",
        layer: "authority",
        scope: global ? "global" : "workspace",
        meta: rule.deny_writes
          ? `deny · ${rule.globs.length} glob${rule.globs.length === 1 ? "" : "s"}`
          : rule.globs.length > 0
            ? `${rule.globs.length} glob${rule.globs.length === 1 ? "" : "s"}`
            : undefined,
        tone: rule.deny_writes ? "warn" : global ? "authority" : "accent",
        preview: `${rule.description || "(no description)"}\n\nGlobs: ${
          rule.globs.join(", ") || "—"
        }\n\n${rule.content.slice(0, 1200)}`,
        jump: "guidance",
      });
      edges.push({
        from: global ? "hub-global" : "hub-workspace",
        to: id,
        kind: rule.deny_writes ? "denies_write" : "contains",
      });
    }

    for (const skill of skills) {
      const id = `skill:${skill.name}`;
      const global = skill.scope === "global";
      nodes.push({
        id,
        label: skill.name,
        kind: "skill",
        layer: "authority",
        scope: global ? "global" : "workspace",
        meta: skill.always_apply ? "always" : "on demand",
        tone: global ? "authority" : "accent",
        preview: `${skill.description}\n\n${skill.body.slice(0, 1200)}`,
        jump: "guidance",
      });
      edges.push({
        from: global ? "hub-global" : "hub-workspace",
        to: id,
        kind: "contains",
      });
    }

    const blockers = auditFindings.filter(
      (finding) => severityTone(finding.severity) === "danger",
    ).length;
    nodes.push({
      id: "hub-audit",
      label: "Audit",
      kind: "hub",
      layer: "work",
      meta: blockers > 0 ? `${blockers} blocking` : `${auditFindings.length} findings`,
      tone: blockers > 0 ? "danger" : "info",
      preview: "L0–L11 workspace findings from the live audit.",
    });
    nodes.push({
      id: "hub-plan",
      label: "Plan",
      kind: "hub",
      layer: "work",
      meta: `${planPhases.length} phase${planPhases.length === 1 ? "" : "s"}`,
      tone: "info",
      preview: "Plan Map phases derived from the audit.",
      jump: "plan",
    });
    edges.push({ from: "hub-audit", to: "hub-plan", kind: "derived_from" });

    for (const finding of auditFindings) {
      const id = `finding:${finding.layer}`;
      if (nodes.some((node) => node.id === id)) continue;
      nodes.push({
        id,
        label: finding.layer,
        kind: "finding",
        layer: "work",
        meta: `${finding.severity} · ${finding.points}/${finding.points_max}`,
        tone: severityTone(finding.severity),
        preview: `${finding.severity} · ${finding.points}/${finding.points_max}\n\n${finding.detail}`,
      });
      edges.push({ from: "hub-audit", to: id, kind: "contains" });
    }

    for (const phase of planPhases) {
      const id = `phase:${phase.id}`;
      nodes.push({
        id,
        label: phase.title,
        kind: "phase",
        layer: "work",
        meta: `${phase.owned_paths.length} path${
          phase.owned_paths.length === 1 ? "" : "s"
        }${phase.gates.length > 0 ? ` · ${phase.gates.join("/")}` : ""}`,
        tone: "info",
        preview: `Owned paths:\n${
          phase.owned_paths.map((path) => `  ${path}`).join("\n") || "  —"
        }\n\nGates: ${phase.gates.join(", ") || "—"}\nDepends on: ${
          (phase.depends_on ?? []).join(", ") || "—"
        }`,
        jump: "plan",
        phaseId: phase.id,
      });
      edges.push({ from: "hub-plan", to: id, kind: "contains" });
      for (const dep of phase.depends_on ?? []) {
        edges.push({ from: `phase:${dep}`, to: id, kind: "depends_on" });
      }
      for (const path of phase.owned_paths) {
        const denyRule = rules.find(
          (rule) =>
            rule.deny_writes &&
            rule.globs.some((glob) => path.includes(glob.replace("*", ""))),
        );
        if (denyRule) {
          edges.push({
            from: `rule:${denyRule.source}`,
            to: id,
            kind: "denies_write",
          });
        }
      }
    }

    for (const gate of verifyGates) {
      const id = `gate:${gate.gate}`;
      if (nodes.some((node) => node.id === id)) continue;
      const unavailable = gate.status === "unavailable" || gate.status === "skipped";
      nodes.push({
        id,
        label: gate.gate,
        kind: "gate",
        layer: "work",
        meta: unavailable ? (gate.status ?? "skipped") : gate.passed ? "pass" : "fail",
        tone: unavailable ? "neutral" : gate.passed ? "ready" : "danger",
        preview: `Verify gate ${gate.gate}\nResult: ${
          unavailable ? (gate.status ?? "skipped") : gate.passed ? "pass" : "fail"
        }`,
        jump: "plan",
      });
      edges.push({ from: "hub-plan", to: id, kind: "verified_by" });
    }

    for (const item of handoffs) {
      const id = `handoff:${item.id}`;
      const failed = item.turn_status === "failed" || item.turn_status === "cancelled";
      nodes.push({
        id,
        label: item.id.slice(0, 14),
        kind: "handoff",
        layer: "work",
        meta: item.turn_status ?? "unknown",
        tone: failed ? "danger" : item.turn_status === "completed" ? "ready" : "neutral",
        preview: `Handoff ${item.id}\nTurn status: ${item.turn_status ?? "unknown"}`,
      });
      edges.push({ from: "hub-plan", to: id, kind: "contains" });
    }

    const known = new Set(nodes.map((node) => node.id));
    return {
      nodes,
      edges: edges.filter((edge) => known.has(edge.from) && known.has(edge.to)),
    };
  }, [rules, skills, activeProfile, auditFindings, planPhases, verifyGates, handoffs]);

  const nodeById = useMemo(
    () => new Map(nodes.map((node) => [node.id, node])),
    [nodes],
  );

  const adjacency = useMemo(() => {
    const map = new Map<string, { id: string; kind: EdgeKind; out: boolean }[]>();
    for (const edge of edges) {
      if (!map.has(edge.from)) map.set(edge.from, []);
      if (!map.has(edge.to)) map.set(edge.to, []);
      map.get(edge.from)!.push({ id: edge.to, kind: edge.kind, out: true });
      map.get(edge.to)!.push({ id: edge.from, kind: edge.kind, out: false });
    }
    for (const list of map.values()) {
      list.sort((a, b) => a.id.localeCompare(b.id));
    }
    return map;
  }, [edges]);

  const availableKinds = useMemo(() => {
    const present = new Set(nodes.map((node) => node.kind));
    return KIND_ORDER.filter((kind) => present.has(kind));
  }, [nodes]);

  // ── Filters ───────────────────────────────────────────────────────────────
  const q = query.trim().toLowerCase();
  const candidates = useMemo(() => {
    return nodes.filter((node) => {
      if (layer !== "both" && node.layer !== layer) return false;
      if (scopeMode !== "all" && node.scope !== scopeMode) return false;
      if (mutedKinds.has(node.kind)) return false;
      if (orphansOnly && (adjacency.get(node.id)?.length ?? 0) > 0) return false;
      if (!q) return true;
      return (
        node.label.toLowerCase().includes(q) ||
        node.kind.includes(q) ||
        (node.meta?.toLowerCase().includes(q) ?? false) ||
        node.preview.toLowerCase().includes(q)
      );
    });
  }, [nodes, layer, scopeMode, mutedKinds, orphansOnly, adjacency, q]);

  const candidateIds = useMemo(
    () => new Set(candidates.map((node) => node.id)),
    [candidates],
  );

  /** Focus mode: BFS over the *filtered* subgraph so filters really prune. */
  const visibleIds = useMemo(() => {
    if (viewMode === "map") return candidateIds;
    const root = candidateIds.has(focusId)
      ? focusId
      : (candidates[0]?.id ?? null);
    if (!root) return new Set<string>();
    const seen = new Set<string>([root]);
    let frontier = [root];
    for (let step = 0; step < depth; step += 1) {
      const next: string[] = [];
      for (const id of frontier) {
        for (const link of adjacency.get(id) ?? []) {
          if (!candidateIds.has(link.id) || seen.has(link.id)) continue;
          seen.add(link.id);
          next.push(link.id);
        }
      }
      frontier = next;
      if (frontier.length === 0) break;
    }
    return seen;
  }, [viewMode, candidateIds, candidates, focusId, depth, adjacency]);

  const effectiveFocusId = candidateIds.has(focusId)
    ? focusId
    : (candidates[0]?.id ?? focusId);

  // ── Deterministic layout ──────────────────────────────────────────────────
  const placed = useMemo<PlacedNode[]>(() => {
    const visible = nodes
      .filter((node) => visibleIds.has(node.id))
      .sort(
        (a, b) =>
          KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) ||
          a.label.localeCompare(b.label) ||
          a.id.localeCompare(b.id),
      );
    if (visible.length === 0) return [];

    if (viewMode === "map") {
      // Layered columns: global authority · workspace authority · audit · plan · verify.
      const columnOf = (node: AtlasNode): number => {
        if (node.layer === "authority") return node.scope === "global" ? 0 : 1;
        if (node.kind === "finding" || node.id === "hub-audit") return 2;
        if (node.kind === "gate" || node.kind === "handoff") return 4;
        return 3;
      };
      const columns = new Map<number, AtlasNode[]>();
      for (const node of visible) {
        const column = columnOf(node);
        if (!columns.has(column)) columns.set(column, []);
        columns.get(column)!.push(node);
      }
      const columnWidth = NODE_MAX_W + 76;
      const rowHeight = NODE_H + 14;
      const out: PlacedNode[] = [];
      for (const [column, members] of columns) {
        members.forEach((node, index) => {
          out.push({
            ...node,
            w: nodeWidth(node),
            h: NODE_H,
            x: PAD + column * columnWidth,
            y: PAD + index * rowHeight,
          });
        });
      }
      return out;
    }

    // Focus mode: concentric rings by BFS distance from the focus node.
    const distance = new Map<string, number>([[effectiveFocusId, 0]]);
    let frontier = [effectiveFocusId];
    let step = 0;
    while (frontier.length > 0 && step < depth) {
      step += 1;
      const next: string[] = [];
      for (const id of frontier) {
        for (const link of adjacency.get(id) ?? []) {
          if (!visibleIds.has(link.id) || distance.has(link.id)) continue;
          distance.set(link.id, step);
          next.push(link.id);
        }
      }
      frontier = next;
    }

    const rings = new Map<number, AtlasNode[]>();
    for (const node of visible) {
      const ring = distance.get(node.id) ?? depth + 1;
      if (!rings.has(ring)) rings.set(ring, []);
      rings.get(ring)!.push(node);
    }

    const out: PlacedNode[] = [];
    for (const [ring, members] of [...rings.entries()].sort(([a], [b]) => a - b)) {
      if (ring === 0) {
        const node = members[0];
        if (node) {
          const w = nodeWidth(node);
          out.push({ ...node, w, h: NODE_H, x: -w / 2, y: -NODE_H / 2 });
        }
        continue;
      }
      // Crowded rings split into concentric bands: a single huge ring would
      // force a fit-zoom small enough to make every label unreadable.
      const perBand = 12;
      const bands = Math.ceil(members.length / perBand);
      const spacing = NODE_MAX_W * 0.62;
      for (let band = 0; band < bands; band += 1) {
        const slice = members.slice(band * perBand, (band + 1) * perBand);
        const radius = Math.max(
          210 * ring + band * 150,
          (slice.length * spacing) / (2 * Math.PI),
        );
        slice.forEach((node, index) => {
          // Offset alternate bands by half a step so nodes never line up radially.
          const angle =
            ((index + (band % 2) * 0.5) / slice.length) * Math.PI * 2 - Math.PI / 2;
          const w = nodeWidth(node);
          out.push({
            ...node,
            w,
            h: NODE_H,
            x: Math.cos(angle) * radius - w / 2,
            y: Math.sin(angle) * radius * 0.72 - NODE_H / 2,
          });
        });
      }
    }

    const minX = Math.min(...out.map((node) => node.x));
    const minY = Math.min(...out.map((node) => node.y));
    return out.map((node) => ({
      ...node,
      x: node.x - minX + PAD,
      y: node.y - minY + PAD,
    }));
  }, [nodes, visibleIds, viewMode, effectiveFocusId, depth, adjacency]);

  const placedById = useMemo(
    () => new Map(placed.map((node) => [node.id, node])),
    [placed],
  );

  const visibleEdges = useMemo(
    () => edges.filter((edge) => placedById.has(edge.from) && placedById.has(edge.to)),
    [edges, placedById],
  );

  const contentWidth = Math.max(
    640,
    ...placed.map((node) => node.x + node.w + PAD),
  );
  const contentHeight = Math.max(
    360,
    ...placed.map((node) => node.y + node.h + PAD),
  );

  // Re-fit whenever the layout changes shape.
  useEffect(() => {
    setFitToken((token) => token + 1);
  }, [viewMode, depth, effectiveFocusId, placed.length]);

  const selected = nodeById.get(selectedId) ?? nodeById.get(effectiveFocusId) ?? null;

  const neighbours = useMemo(() => {
    const list = adjacency.get(selected?.id ?? "") ?? [];
    return list
      .filter((link) => nodeById.has(link.id))
      .map((link) => ({ link, node: nodeById.get(link.id)! }));
  }, [adjacency, selected, nodeById]);

  /** Arrow keys cycle the focus node's neighbourhood; Enter promotes to focus. */
  const cycle = useCallback(
    (direction: 1 | -1) => {
      const ring = [
        effectiveFocusId,
        ...(adjacency.get(effectiveFocusId) ?? [])
          .filter((link) => visibleIds.has(link.id))
          .map((link) => link.id),
      ];
      if (ring.length === 0) return;
      const current = ring.indexOf(selectedId);
      const next = (current + direction + ring.length) % ring.length;
      setSelectedId(ring[next]);
    },
    [effectiveFocusId, adjacency, visibleIds, selectedId],
  );

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === "/") {
      event.preventDefault();
      searchRef.current?.focus();
      return;
    }
    if (event.key === "f") {
      event.preventDefault();
      setFitToken((token) => token + 1);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      setQuery("");
      setSelectedId(effectiveFocusId);
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      setFocusId(selectedId);
      setViewMode("focus");
      return;
    }
    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      event.preventDefault();
      cycle(1);
      return;
    }
    if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      event.preventDefault();
      cycle(-1);
    }
  };

  const focusNode = (id: string) => {
    setFocusId(id);
    setSelectedId(id);
    setViewMode("focus");
  };

  const orphanCount = useMemo(
    () => nodes.filter((node) => (adjacency.get(node.id)?.length ?? 0) === 0).length,
    [nodes, adjacency],
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <nav className="flex items-center gap-1 text-[11px] text-slate-500">
          <button
            type="button"
            onClick={onOpenGuidance}
            className="text-blue-300/90 hover:text-blue-200"
          >
            Guidance
          </button>
          <span aria-hidden>→</span>
          <span className="font-semibold text-slate-200">Atlas</span>
          <span aria-hidden>→</span>
          <button
            type="button"
            onClick={() => onOpenPlan?.()}
            className="text-blue-300/90 hover:text-blue-200"
          >
            Plan Map
          </button>
        </nav>

        <div className="flex rounded-lg border border-line-strong p-0.5 text-[11px]">
          {(["focus", "map"] as ViewMode[]).map((mode) => (
            <button
              key={mode}
              type="button"
              onClick={() => setViewMode(mode)}
              title={
                mode === "focus"
                  ? "Neighbourhood of one node — the view that stays useful as the graph grows"
                  : "Whole filtered graph in layered columns — structural review, not navigation"
              }
              className={`rounded-md px-2.5 py-1 capitalize ${
                viewMode === mode
                  ? "bg-blue-500/20 text-blue-100"
                  : "text-slate-500 hover:text-slate-300"
              }`}
            >
              {mode}
            </button>
          ))}
        </div>

        {viewMode === "focus" && (
          <div className="flex items-center gap-1 text-[11px] text-slate-500">
            <span className="text-[10px] uppercase tracking-wider text-slate-600">
              Depth
            </span>
            {[1, 2].map((value) => (
              <button
                key={value}
                type="button"
                onClick={() => setDepth(value)}
                className={`rounded-md px-2 py-1 ${
                  depth === value
                    ? "bg-blue-500/20 text-blue-100"
                    : "hover:text-slate-300"
                }`}
              >
                {value}
              </button>
            ))}
          </div>
        )}

        <input
          ref={searchRef}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Filter nodes…  (/)"
          className="w-40 rounded-lg border border-line-strong bg-surface-3 px-2.5 py-1 text-[11px] text-slate-200 placeholder:text-slate-600"
        />

        <span
          className="rounded-md border border-line-strong bg-white/5 px-2 py-0.5 text-[10px] text-slate-400"
          title="Change in Guidance"
        >
          {activeProfile ? `profile:${activeProfile}` : "profile:all"}
        </span>

        <span className="ml-auto text-[10px] text-slate-600">
          {placed.length} of {nodes.length} nodes · {visibleEdges.length} edges
        </span>
      </div>

      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <div className="flex rounded-lg border border-line-strong p-0.5 text-[11px]">
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
        <div className="flex rounded-lg border border-line-strong p-0.5 text-[11px]">
          {(["all", "global", "workspace"] as ScopeMode[]).map((mode) => (
            <button
              key={mode}
              type="button"
              onClick={() => setScopeMode(mode)}
              className={`rounded-md px-2.5 py-1 capitalize ${
                scopeMode === mode
                  ? "bg-violet-500/20 text-violet-100"
                  : "text-slate-500 hover:text-slate-300"
              }`}
            >
              {mode}
            </button>
          ))}
        </div>
        <div className="flex flex-wrap gap-1.5">
          {availableKinds.map((kind) => (
            <Chip
              key={kind}
              active={!mutedKinds.has(kind)}
              title={`Toggle ${KIND_LABEL[kind]} nodes`}
              onClick={() =>
                setMutedKinds((current) => {
                  const next = new Set(current);
                  if (next.has(kind)) next.delete(kind);
                  else next.add(kind);
                  return next;
                })
              }
            >
              {KIND_GLYPH[kind]} {KIND_LABEL[kind]}
            </Chip>
          ))}
          <Chip
            active={orphansOnly}
            title="Nodes with no edges — the one job a whole-graph view is genuinely good at"
            onClick={() => {
              setOrphansOnly((current) => {
                if (!current) setViewMode("map");
                return !current;
              });
            }}
          >
            Orphans {orphanCount > 0 ? `(${orphanCount})` : ""}
          </Chip>
        </div>
      </div>

      {error && <p className="text-xs text-red-400">{error}</p>}

      <div className="grid min-h-0 flex-1 gap-3 lg:grid-cols-[1fr_320px]">
        <div
          tabIndex={0}
          onKeyDown={onKeyDown}
          aria-label="Atlas graph. Arrow keys walk neighbours, Enter re-centres, slash searches, f fits."
          className="flex min-h-0 flex-col rounded-2xl outline-hidden focus-visible:ring-2 focus-visible:ring-blue-400/50"
        >
          <GraphCanvas
            contentWidth={contentWidth}
            contentHeight={contentHeight}
            fitToken={fitToken}
            fitMinScale={0.72}
            className="min-h-[320px] flex-1 rounded-2xl border border-line bg-surface-0"
            toolbarExtra={
              <span className="hidden text-[10px] text-slate-600 lg:inline">
                ←→ neighbours · Enter re-centre · / search · f fit
              </span>
            }
          >
            <defs>
              {(Object.keys(EDGE_STYLE) as EdgeKind[]).map((kind) => (
                <marker
                  key={kind}
                  id={`atlas-arrow-${kind}`}
                  viewBox="0 0 8 8"
                  refX={7}
                  refY={4}
                  markerWidth={5}
                  markerHeight={5}
                  orient="auto-start-reverse"
                >
                  <path d="M0,0 L8,4 L0,8 z" fill={EDGE_STYLE[kind].stroke} />
                </marker>
              ))}
            </defs>

            {visibleEdges.map((edge) => {
              const from = placedById.get(edge.from)!;
              const to = placedById.get(edge.to)!;
              const style = EDGE_STYLE[edge.kind];
              const start = edgePoint(
                from,
                to.x + to.w / 2,
                to.y + to.h / 2,
              );
              const end = edgePoint(to, from.x + from.w / 2, from.y + from.h / 2);
              const touchesSelection =
                edge.from === selectedId || edge.to === selectedId;
              return (
                <line
                  key={`${edge.from}-${edge.to}-${edge.kind}`}
                  x1={start.x}
                  y1={start.y}
                  x2={end.x}
                  y2={end.y}
                  stroke={style.stroke}
                  strokeWidth={touchesSelection ? 1.6 : 1}
                  strokeOpacity={touchesSelection ? 1 : 0.6}
                  strokeDasharray={style.dash}
                  markerEnd={`url(#atlas-arrow-${edge.kind})`}
                />
              );
            })}

            {placed.map((node) => {
              const active = node.id === selectedId;
              const isFocus = node.id === effectiveFocusId;
              const stroke =
                node.tone === "neutral"
                  ? "rgba(255,255,255,0.14)"
                  : TONE_FILL[node.tone];
              return (
                <g
                  key={node.id}
                  data-graph-node=""
                  transform={`translate(${node.x}, ${node.y})`}
                  onClick={() => setSelectedId(node.id)}
                  onDoubleClick={() => focusNode(node.id)}
                  style={{ cursor: "pointer" }}
                >
                  <rect
                    width={node.w}
                    height={node.h}
                    rx={8}
                    fill={active ? "rgba(59,130,246,0.28)" : scopeFill(node)}
                    stroke={active ? "rgba(147,197,253,0.95)" : stroke}
                    strokeWidth={active ? 1.8 : isFocus ? 1.5 : 1}
                    strokeOpacity={active ? 1 : node.tone === "neutral" ? 0.9 : 0.75}
                  />
                  <text
                    x={9}
                    y={node.meta ? 14 : 21}
                    style={{ fontSize: 10.5, fill: "#e2e8f0", fontWeight: 500 }}
                  >
                    <tspan style={{ fill: stroke, fontSize: 9 }}>
                      {KIND_GLYPH[node.kind]}
                    </tspan>
                    <tspan dx={5}>
                      {node.label.length > 26
                        ? `${node.label.slice(0, 25)}…`
                        : node.label}
                    </tspan>
                  </text>
                  {node.meta ? (
                    <text
                      x={9}
                      y={26}
                      style={{ fontSize: 8.5, fill: "#7c8ba1", fontWeight: 400 }}
                    >
                      {node.meta.length > 30 ? `${node.meta.slice(0, 29)}…` : node.meta}
                    </text>
                  ) : null}
                  {isFocus && !active ? (
                    <rect
                      width={node.w}
                      height={node.h}
                      rx={8}
                      fill="none"
                      stroke="rgba(147,197,253,0.35)"
                      strokeDasharray="3 3"
                    />
                  ) : null}
                </g>
              );
            })}
          </GraphCanvas>

          <Legend
            className="mt-2 shrink-0"
            items={[
              { label: "Global authority", color: "rgba(167,139,250,0.6)" },
              { label: "Workspace authority", color: "rgba(96,165,250,0.5)" },
              { label: "Work", color: "rgba(56,189,248,0.45)" },
              { label: "Blocking", color: TONE_FILL.danger },
              { label: "Attention", color: TONE_FILL.warn },
              { label: "Passing", color: TONE_FILL.ready },
              {
                label: "denies write",
                color: TONE_FILL.warn,
                shape: "line",
                dashed: true,
              },
              { label: "depends on", color: TONE_FILL.accent, shape: "line" },
              { label: "verified by", color: TONE_FILL.ready, shape: "line" },
            ]}
          />
        </div>

        <aside className="thin-scrollbar min-h-0 overflow-y-auto rounded-2xl border border-line bg-surface-2/85 p-4">
          {selected ? (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-[10px] uppercase tracking-wider text-slate-600">
                <span style={{ color: TONE_FILL[selected.tone] }}>
                  {KIND_GLYPH[selected.kind]}
                </span>
                <span>{KIND_LABEL[selected.kind]}</span>
                {selected.scope ? <span>· {selected.scope}</span> : null}
                <span>· {selected.layer}</span>
              </div>
              <div>
                <div className="text-sm font-semibold text-slate-100">
                  {selected.label}
                </div>
                {selected.meta ? (
                  <div
                    className="text-[11px]"
                    style={{ color: TONE_FILL[selected.tone] }}
                  >
                    {selected.meta}
                  </div>
                ) : null}
              </div>

              {selected.id !== effectiveFocusId && (
                <button
                  type="button"
                  onClick={() => focusNode(selected.id)}
                  className="w-full rounded-lg border border-line-strong bg-white/4 py-1.5 text-[11px] font-medium text-slate-200 hover:bg-white/8"
                >
                  Centre here (Enter)
                </button>
              )}

              <div>
                <div className="mb-1.5 text-[10px] uppercase tracking-wider text-slate-600">
                  Connections ({neighbours.length})
                </div>
                {neighbours.length === 0 ? (
                  <p className="text-[11px] text-slate-600">
                    Orphan — nothing links here.
                  </p>
                ) : (
                  <ul className="space-y-0.5">
                    {neighbours.map(({ link, node }) => (
                      <li key={`${link.id}-${link.kind}-${link.out}`}>
                        <button
                          type="button"
                          onClick={() => setSelectedId(node.id)}
                          onDoubleClick={() => focusNode(node.id)}
                          className="flex w-full items-center gap-1.5 rounded-md px-1.5 py-1 text-left text-[11px] text-slate-400 hover:bg-white/5 hover:text-slate-200"
                        >
                          <span
                            className="shrink-0"
                            style={{ color: TONE_FILL[node.tone] }}
                          >
                            {KIND_GLYPH[node.kind]}
                          </span>
                          <span className="min-w-0 flex-1 truncate">{node.label}</span>
                          <span
                            className="shrink-0 text-[9px]"
                            style={{ color: EDGE_STYLE[link.kind].stroke }}
                          >
                            {link.out ? "→" : "←"} {EDGE_STYLE[link.kind].label}
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              <pre className="thin-scrollbar max-h-[32vh] overflow-auto whitespace-pre-wrap rounded-lg bg-black/30 p-3 text-[11px] leading-5 text-slate-300">
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
            <p className="text-xs text-slate-500">
              No node matches these filters. Clear the search or re-enable a kind.
            </p>
          )}
        </aside>
      </div>
    </div>
  );
}
