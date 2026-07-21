import { Channel } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke, isTauri } from "./ipc";
import {
  RecipeWizard,
  type ScaffoldResult,
  type ScaffoldFilePlan,
  type StackRecipe,
} from "./components/RecipeWizard";
import { RulesEditor } from "./components/RulesEditor";
import { PlanMap } from "./components/PlanMap";
import { AtlasView } from "./components/AtlasView";
import { Chip, ChipRow, Disclosure, Hint } from "./components/ui";

const DEV_MODE_KEY = "ade_dev_mode";
const AUTONOMY_KEY = "ade_autonomy_level";
const SURFACE_MODE_KEY = "ade_surface_mode";
const AGENT_PROVIDER_KEY = "ade_agent_provider";
const AGENT_BASE_URL_KEY = "ade_agent_base_url";
const AGENT_MODEL_KEY = "ade_agent_model";
const NAV_OPEN_KEY = "ade_nav_open";

type AutonomyLevel = "observe" | "propose" | "act" | "automate";
type SurfaceMode = "guided" | "power" | "dev";

const SURFACE_MODES: { id: SurfaceMode; label: string; hint: string }[] = [
  {
    id: "guided",
    label: "Simple",
    hint: "Do work: Home, Agent, Keys, Verify, Recipes. Suggest or Apply only.",
  },
  {
    id: "power",
    label: "Full",
    hint: "All views + Observe→Automate. Turn on Debug for budgets and traces.",
  },
  {
    id: "dev",
    label: "Debug",
    hint: "Same as Full, with traces, leases, and harness open.",
  },
];

const PROVIDER_PRESETS: {
  id: string;
  baseUrl: string;
  models: string[];
}[] = [
  {
    id: "openai",
    baseUrl: "https://api.openai.com/v1",
    models: ["gpt-4.1-mini", "gpt-4.1", "o4-mini"],
  },
  {
    id: "anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    models: ["claude-sonnet-4-5", "claude-opus-4"],
  },
  {
    id: "openrouter",
    baseUrl: "https://openrouter.ai/api/v1",
    models: ["openai/gpt-4.1-mini", "anthropic/claude-sonnet-4"],
  },
];

const DEFAULT_MODEL = "gpt-4.1-mini";

const PROMPT_PRESETS: {
  label: string;
  prompt: string;
  autonomy: "propose" | "act";
}[] = [
  {
    label: "Explain repo",
    prompt: "Summarize what this workspace is for and the safest next change.",
    autonomy: "propose",
  },
  {
    label: "Find bug risk",
    prompt: "Scan for the top 3 likely bugs or missing tests. Suggest-only; no edits.",
    autonomy: "propose",
  },
  {
    label: "Small fix",
    prompt:
      "Propose one small, verify-gated fix inside PLAN owned paths, then apply if approved.",
    autonomy: "act",
  },
];

const AUTONOMY_LEVELS: { id: AutonomyLevel; label: string; hint: string }[] = [
  { id: "observe", label: "Observe", hint: "Read-only; explain and point" },
  { id: "propose", label: "Propose", hint: "Plan + diffs; no apply" },
  { id: "act", label: "Act", hint: "Execute approved owned paths" },
  { id: "automate", label: "Automate", hint: "Caps + verify gates required" },
];

const GUIDED_NAV_IDS = new Set(["Home", "Agent", "Keys", "Verify", "Recipes"]);

const VERIFY_GATE_LABELS: Record<string, string> = {
  G0: "Quick sanity check",
  G1: "Project setup",
  G2: "Code quality",
  G3: "Automated tests",
  G4: "Integration",
  G5: "End-to-end / browser",
};

function verifyGateLabel(gate: string): string {
  return VERIFY_GATE_LABELS[gate] ?? gate;
}

function workspaceLeaf(path: string | undefined | null): string {
  if (!path) return "Locating workspace…";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function readAutonomy(): AutonomyLevel {
  if (typeof window === "undefined") return "propose";
  const raw = window.localStorage.getItem(AUTONOMY_KEY);
  if (raw === "observe" || raw === "propose" || raw === "act" || raw === "automate") {
    return raw;
  }
  return "propose";
}

function readSurfaceMode(): SurfaceMode {
  if (typeof window === "undefined") return "guided";
  const raw = window.localStorage.getItem(SURFACE_MODE_KEY);
  if (raw === "power" || raw === "guided" || raw === "dev") return raw;
  return "guided";
}

type NavItem = { id: string; label: string; icon: string; desktopOnly?: boolean };

type NavGroup = { title?: string; items: NavItem[] };

const navGroups: NavGroup[] = [
  {
    items: [
      { id: "Home", label: "Home", icon: "⌂" },
      { id: "Agent", label: "Agent", icon: "✦" },
    ],
  },
  {
    title: "Setup",
    items: [
      { id: "Recipes", label: "Recipes", icon: "▦" },
      { id: "Rules", label: "Guidance", icon: "☰" },
      { id: "Atlas", label: "Atlas", icon: "◈" },
      { id: "Plan", label: "Plan Map", icon: "◇" },
    ],
  },
  {
    title: "Trust",
    items: [
      { id: "Health", label: "Health", icon: "◎" },
      { id: "Audit", label: "Audit", icon: "◉" },
      { id: "Verify", label: "Verify", icon: "✓" },
    ],
  },
  {
    title: "Integrations",
    items: [
      { id: "Keys", label: "Keys", icon: "◈", desktopOnly: true },
      { id: "MCP", label: "MCP", icon: "⬡", desktopOnly: true },
    ],
  },
];

function navGroupsForSurface(mode: SurfaceMode): NavGroup[] {
  if (mode !== "guided") return navGroups;
  // Flat work rail — no Setup/Trust grouping noise in Simple.
  const order = ["Home", "Agent", "Keys", "Verify", "Recipes"];
  const byId = new Map(
    navGroups.flatMap((group) => group.items).map((item) => [item.id, item]),
  );
  return [
    {
      items: order
        .map((id) => byId.get(id))
        .filter((item): item is NavItem => Boolean(item)),
    },
  ];
}

function readDevMode(): boolean {
  if (typeof window === "undefined") return false;
  const env = (import.meta as { env?: { VITE_ADE_DEV_MODE?: string } }).env
    ?.VITE_ADE_DEV_MODE;
  if (env === "1" || env === "true") return true;
  return window.localStorage.getItem(DEV_MODE_KEY) === "1";
}

type Finding = {
  layer: string;
  severity: string;
  detail: string;
  points: number;
  points_max: number;
};

type AuditReport = {
  score: number;
  score_max: number;
  findings: Finding[];
  blockers: string[];
};

type PlanPhase = {
  id: string;
  title: string;
  owned_paths: string[];
  gates: string[];
  depends_on?: string[];
};

type PlanReport = {
  phases: PlanPhase[];
  requires_human: string[];
  score_before?: number;
  score_max?: number;
  audit_root?: string;
};

type HandoffPromptSection = {
  name: string;
  tokens: number;
  truncated: boolean;
};

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

type HandoffMetrics = {
  capsule_count: number;
  invalid_capsule_count: number;
  total_bytes: number;
  latest_bytes: number;
  latest_summary_chars: number;
  latest_compaction_percent: number;
  latest_score_before: number | null;
  latest_score_after: number | null;
  latest_score_max: number | null;
  latest_score_delta: number | null;
  latest_status: string | null;
  latest_created_at: string | null;
  latest_context_status: string | null;
  latest_context_tokens: number | null;
  latest_context_sections: HandoffPromptSection[];
  recent: HandoffHistoryItem[];
};

type PathLease = {
  id: string;
  agent_id: string;
  path: string;
  mode: "observe" | "cooperative" | "strong" | "exclusive";
  created_at: string;
  expires_at: string;
  protected: boolean;
};

type AgentTask = {
  id: string;
  goal: string;
  owned_paths: string[];
  lease_mode: "observe" | "cooperative" | "strong" | "exclusive";
  depends_on: string[];
  status: "queued" | "claimed" | "running" | "completed" | "failed" | "cancelled";
  agent_id: string | null;
  created_at: string;
  heartbeat_at: string | null;
  expires_at: string | null;
  failure: string | null;
};

type GuidedWinsState = {
  understand: boolean;
  verify: boolean;
  improve_ade: boolean;
  understand_artifact?: string | null;
};

type UnderstandResult = {
  path: string;
  summary: string;
  wins: GuidedWinsState;
};

type DashboardSnapshot = {
  workspace_root: string;
  is_dogfood?: boolean;
  ade_source_root?: string | null;
  audit: AuditReport;
  plan: PlanReport;
  handoff: HandoffMetrics;
  leases: PathLease[];
  tasks: AgentTask[];
  rebuild_lock_warnings?: string[];
};

type VerifyResult = {
  gate: string;
  command: string;
  exit_code: number | null;
  stdout: string | null;
  stderr: string | null;
  passed: boolean;
  status?: "pass" | "fail" | "unavailable" | "skipped";
};

type McpToolInfo = {
  server: string;
  name: string;
  description: string;
  input_schema: {
    properties?: Record<string, { type?: string; description?: string; default?: unknown }>;
    required?: string[];
  };
};

type McpToolCallResult = {
  server: string;
  tool: string;
  is_error: boolean;
  text: string;
  content: unknown;
};

type AgentTurnResult = {
  session_id: string;
  provider: string;
  model: string;
  text: string;
  tool_calls: number;
  usage: { input_tokens: number; output_tokens: number };
  cost_micros: number;
};

type AgentEvent =
  | { type: "started"; session_id: string; provider: string; model: string }
  | { type: "text_delta"; text: string }
  | {
      type: "tool_call";
      server: string;
      tool: string;
      arguments: unknown;
      effect?: string;
    }
  | { type: "tool_result"; server: string; tool: string; is_error: boolean; text: string }
  | {
      type: "usage";
      input_tokens: number;
      output_tokens: number;
      cost_micros: number;
    }
  | {
      type: "spend_warning";
      scope: string;
      period_key: string;
      projected_micros: number;
      soft_cap_micros: number;
    }
  | { type: "verify_complete"; gate: string; passed: boolean; summary: string }
  | { type: "completed"; result: AgentTurnResult }
  | { type: "failed"; error: string }
  | { type: "cancelled"; reason: string };

type ProviderKeyStatus = {
  profile: string;
  provider: string;
  configured: boolean;
};

type ProviderKeyDeleteResult = {
  profile: string;
  provider: string;
  deleted: boolean;
};

type ProviderKeySmokeResult = {
  profile: string;
  provider: string;
  status: "ready" | "passed" | "failed" | "skipped";
  detail: string;
};

function App() {
  const [dashboard, setDashboard] = useState<DashboardSnapshot | null>(null);
  const [activeView, setActiveView] = useState("Home");
  const [surfaceMode, setSurfaceMode] = useState<SurfaceMode>(readSurfaceMode);
  const [devMode, setDevMode] = useState(() => readDevMode() || readSurfaceMode() === "dev");
  const [navOpen, setNavOpen] = useState(() => {
    if (typeof window === "undefined") return false;
    // Simple: full-width canvas by default; Full keeps last preference.
    if (readSurfaceMode() === "guided") return false;
    return window.localStorage.getItem(NAV_OPEN_KEY) === "1";
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [gate, setGate] = useState("G3");
  const [verifyResults, setVerifyResults] = useState<VerifyResult[]>([]);
  const [verifying, setVerifying] = useState(false);
  const [executing, setExecuting] = useState(false);
  const [mcpServers, setMcpServers] = useState<string[]>([]);
  const [mcpTools, setMcpTools] = useState<McpToolInfo[]>([]);
  const [mcpBusy, setMcpBusy] = useState(false);
  const [agentEvents, setAgentEvents] = useState<AgentEvent[]>([]);
  const [agentBusy, setAgentBusy] = useState(false);
  const [recipes, setRecipes] = useState<StackRecipe[]>([]);
  const [recipeBusy, setRecipeBusy] = useState(false);
  const [recipePlan, setRecipePlan] = useState<ScaffoldFilePlan[] | null>(null);
  const [recipePlanError, setRecipePlanError] = useState<string | null>(null);
  const [recipeResult, setRecipeResult] = useState<ScaffoldResult | null>(null);
  const [homePrompt, setHomePrompt] = useState("");
  const [agentAutoSubmit, setAgentAutoSubmit] = useState(false);
  const [pendingImproveWin, setPendingImproveWin] = useState(false);
  const [guidedWins, setGuidedWins] = useState<GuidedWinsState>({
    understand: false,
    verify: false,
    improve_ade: false,
  });
  const [understandBusy, setUnderstandBusy] = useState(false);
  const [lastUnderstandPath, setLastUnderstandPath] = useState<string | null>(null);

  const toggleDevMode = () => {
    setDevMode((prev) => {
      const next = !prev;
      window.localStorage.setItem(DEV_MODE_KEY, next ? "1" : "0");
      return next;
    });
  };

  const setNavOpenPersisted = (open: boolean) => {
    setNavOpen(open);
    window.localStorage.setItem(NAV_OPEN_KEY, open ? "1" : "0");
  };

  const setSurfaceModePersisted = (mode: SurfaceMode) => {
    setSurfaceMode(mode);
    window.localStorage.setItem(SURFACE_MODE_KEY, mode);
    if (mode === "dev") {
      setDevMode(true);
      window.localStorage.setItem(DEV_MODE_KEY, "1");
    }
    if (mode === "power") {
      setDevMode(false);
      window.localStorage.setItem(DEV_MODE_KEY, "0");
    }
    if (mode === "guided") {
      setDevMode(false);
      window.localStorage.setItem(DEV_MODE_KEY, "0");
      setNavOpenPersisted(false);
      const allowed = GUIDED_NAV_IDS;
      setActiveView((current) => (allowed.has(current) ? current : "Home"));
    }
  };

  useEffect(() => {
    if (surfaceMode === "guided" && devMode) {
      setDevMode(false);
      window.localStorage.setItem(DEV_MODE_KEY, "0");
    }
  }, [surfaceMode, devMode]);

  const visibleNav = useMemo(() => navGroupsForSurface(surfaceMode), [surfaceMode]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [snapshot, wins] = await Promise.all([
        invoke<DashboardSnapshot>("get_dashboard"),
        invoke<GuidedWinsState>("guided_wins_status").catch(
          (): GuidedWinsState => ({
            understand: false,
            verify: false,
            improve_ade: false,
            understand_artifact: null,
          }),
        ),
      ]);
      setDashboard({ ...snapshot, tasks: snapshot.tasks ?? [] });
      setGuidedWins(wins);
      if (wins.understand_artifact) {
        setLastUnderstandPath(wins.understand_artifact);
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshMcp = useCallback(async () => {
    try {
      const [servers, tools] = await Promise.all([
        invoke<string[]>("mcp_list_servers"),
        invoke<McpToolInfo[]>("mcp_list_tools"),
      ]);
      setMcpServers(servers);
      setMcpTools(tools);
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (activeView === "MCP") {
      void refreshMcp();
    }
  }, [activeView, refreshMcp]);

  useEffect(() => {
    if (activeView === "Recipes" && recipes.length === 0) {
      void invoke<StackRecipe[]>("list_recipes")
        .then(setRecipes)
        .catch((reason) => setError(String(reason)));
    }
  }, [activeView, recipes.length]);

  const scorePercent = useMemo(() => {
    if (!dashboard || dashboard.audit.score_max === 0) return 0;
    return Math.round((dashboard.audit.score / dashboard.audit.score_max) * 100);
  }, [dashboard]);

  const runVerify = async (options?: { stayOnHome?: boolean }) => {
    setVerifying(true);
    setError(null);
    try {
      const results = await invoke<VerifyResult[]>("run_verify", {
        gate,
        through: true,
      });
      setVerifyResults(results);
      const passed = results.every(
        (result) => result.passed || result.status === "unavailable" || result.status === "skipped",
      );
      if (passed) {
        try {
          const wins = await invoke<GuidedWinsState>("guided_mark_win", { win: "verify" });
          setGuidedWins(wins);
        } catch {
          // non-fatal: verify still succeeded
        }
      }
      if (
        !options?.stayOnHome &&
        activeView !== "Verify" &&
        activeView !== "Home" &&
        activeView !== "Agent"
      ) {
        setActiveView("Verify");
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setVerifying(false);
    }
  };

  const runAudit = async () => {
    setLoading(true);
    setError(null);
    try {
      await invoke("run_audit");
      await invoke("run_plan");
      await refresh();
      setActiveView("Plan");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  };

  const runUnderstandProject = async () => {
    setUnderstandBusy(true);
    setError(null);
    try {
      const result = await invoke<UnderstandResult>("guided_understand_project");
      setGuidedWins(result.wins);
      setLastUnderstandPath(result.path);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setUnderstandBusy(false);
    }
  };

  const startImproveAde = () => {
    const prompt =
      "Improve ADE itself in this workspace: propose a small, verify-gated change that advances Ideal ADE I4 activation (guided wins / self-build). Stay inside owned_paths from PLAN. Run verify after.";
    window.localStorage.setItem(AUTONOMY_KEY, "act");
    setHomePrompt(prompt);
    setPendingImproveWin(true);
    setAgentAutoSubmit(true);
    setActiveView("Agent");
  };

  const openAdeOnItself = async () => {
    setError(null);
    try {
      const result = await invoke<{ workspace_root: string; already_dogfood: boolean }>(
        "open_ade_on_itself",
      );
      await refresh();
      if (!result.already_dogfood) {
        setHomePrompt(
          "ADE is now pointed at its own repo. Run Understand / Verify / Improve ADE from Home.",
        );
      }
    } catch (reason) {
      setError(String(reason));
    }
  };

  const executePlan = async () => {
    if (
      !window.confirm(
        "Apply the current approved plan? ADE will only write to its owned paths.",
      )
    ) {
      return;
    }
    setExecuting(true);
    setError(null);
    try {
      await invoke("run_execute", {
        approved: true,
        recipe: "rust-api-turso",
      });
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setExecuting(false);
    }
  };

  const connectMcp = async (input: {
    name: string;
    command: string;
    args: string[];
    approved: boolean;
  }) => {
    setMcpBusy(true);
    setError(null);
    try {
      await invoke("mcp_connect", input);
      await refreshMcp();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setMcpBusy(false);
    }
  };

  const disconnectMcp = async (name: string) => {
    setMcpBusy(true);
    setError(null);
    try {
      await invoke("mcp_disconnect", { name });
      await refreshMcp();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setMcpBusy(false);
    }
  };

  const callMcpTool = async (input: {
    server: string;
    tool: string;
    arguments: unknown;
  }) => {
    setMcpBusy(true);
    setError(null);
    try {
      return await invoke<McpToolCallResult>("mcp_call_tool", input);
    } catch (reason) {
      setError(String(reason));
      return null;
    } finally {
      setMcpBusy(false);
    }
  };

  const runAgentTurn = async (input: {
    prompt: string;
    provider: string;
    baseUrl: string;
    model: string;
    inputCostPerMtok: number;
    outputCostPerMtok: number;
    sessionCapUsd: number;
    dailyCapUsd: number;
    autonomy: AutonomyLevel;
    maxSteps: number;
    maxTokens: number | null;
    verifyOnComplete: boolean;
    verifyGate: string;
    approveOwnedPaths: boolean;
    ownedPaths: string[];
  }) => {
    if (!isTauri()) {
      setError(
        "Agent turns require the ADE desktop app; the browser preview is read-only.",
      );
      return;
    }
    setAgentBusy(true);
    setAgentEvents([]);
    setError(null);
    const onEvent = new Channel<AgentEvent>();
    onEvent.onmessage = (event) => {
      setAgentEvents((current) => [...current, event]);
    };
    try {
      await invoke("run_agent_turn", {
        prompt: input.prompt,
        provider: input.provider,
        baseUrl: input.baseUrl,
        model: input.model,
        inputCostPerMtok: input.inputCostPerMtok,
        outputCostPerMtok: input.outputCostPerMtok,
        sessionCapUsd: input.sessionCapUsd,
        dailyCapUsd: input.dailyCapUsd,
        leaseAgentId: null,
        autonomy: input.autonomy,
        maxSteps: input.maxSteps,
        maxTokens: input.maxTokens,
        verifyOnComplete: input.verifyOnComplete,
        verifyGate: input.verifyGate,
        approveOwnedPaths: input.approveOwnedPaths,
        ownedPaths: input.ownedPaths,
        onEvent,
      });
      if (pendingImproveWin) {
        try {
          const wins = await invoke<GuidedWinsState>("guided_mark_win", {
            win: "improve_ade",
          });
          setGuidedWins(wins);
        } catch {
          // non-fatal
        }
        setPendingImproveWin(false);
      }
    } catch (reason) {
      setError(String(reason));
      setPendingImproveWin(false);
    } finally {
      setAgentBusy(false);
    }
  };

  const previewRecipe = useCallback(
    (input: { recipe: string; projectName: string; force: boolean }) => {
      void invoke<ScaffoldFilePlan[]>("preview_recipe_scaffold", {
        recipe: input.recipe,
        projectName: input.projectName || null,
        force: input.force,
      })
        .then((plan) => {
          setRecipePlan(plan);
          setRecipePlanError(null);
        })
        .catch((reason) => {
          setRecipePlan(null);
          setRecipePlanError(String(reason));
        });
    },
    [],
  );

  const initializeRecipe = async (input: {
    recipe: string;
    projectName: string;
    force: boolean;
  }) => {
    setRecipeBusy(true);
    setError(null);
    try {
      const result = await invoke<ScaffoldResult>("initialize_recipe", {
        recipe: input.recipe,
        projectName: input.projectName || null,
        force: input.force,
      });
      setRecipeResult(result);
      setRecipePlan(result.files);
      setRecipePlanError(null);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setRecipeBusy(false);
    }
  };

  return (
    <div className="flex h-screen overflow-hidden text-slate-100">
      {navOpen && (
        <button
          type="button"
          aria-label="Close navigation"
          className={`fixed inset-0 z-20 bg-black/50 ${
            surfaceMode === "guided" ? "" : "md:hidden"
          }`}
          onClick={() => setNavOpenPersisted(false)}
        />
      )}
      <aside
        className={`fixed inset-y-0 left-0 z-30 flex w-[15.5rem] shrink-0 flex-col border-r border-white/7 bg-[#0b0f16] px-3 py-4 transition-transform ${
          surfaceMode === "guided"
            ? navOpen
              ? "translate-x-0"
              : "-translate-x-full"
            : `md:static md:translate-x-0 ${navOpen ? "translate-x-0" : "-translate-x-full md:translate-x-0"}`
        }`}
      >
        <div className="flex items-center gap-2.5 px-2 pb-3">
          <div className="grid size-8 place-items-center rounded-lg border border-blue-400/35 bg-gradient-to-br from-blue-500/25 to-cyan-500/10 text-xs font-black tracking-tight text-blue-200">
            ADE
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-sm font-semibold tracking-wide text-slate-50">ADE</div>
          </div>
          <button
            type="button"
            className={`grid size-8 place-items-center rounded-lg border border-white/10 text-slate-400 ${
              surfaceMode === "guided" ? "" : "md:hidden"
            }`}
            aria-label="Close menu"
            onClick={() => setNavOpenPersisted(false)}
          >
            ✕
          </button>
        </div>

        {surfaceMode !== "guided" ? (
          <div
            className="mb-4 grid grid-cols-3 gap-1 rounded-lg border border-white/8 bg-black/20 p-1"
            title="Simple = do work. Full = all tools. Debug = Full + traces."
          >
            {SURFACE_MODES.map((mode) => (
              <button
                key={mode.id}
                type="button"
                title={mode.hint}
                onClick={() => {
                  setSurfaceModePersisted(mode.id);
                  setNavOpenPersisted(false);
                }}
                className={`rounded-md px-1 py-1.5 text-[11px] font-medium tracking-tight transition ${
                  surfaceMode === mode.id
                    ? "bg-blue-500/25 text-blue-100"
                    : "text-slate-500 hover:text-slate-300"
                }`}
              >
                {mode.label}
              </button>
            ))}
          </div>
        ) : (
          <p className="mb-3 px-2 text-[10px] uppercase tracking-wider text-slate-600">
            Simple
          </p>
        )}

        <nav className="thin-scrollbar min-h-0 flex-1 space-y-2 overflow-y-auto">
          {visibleNav.map((group) => (
            <div key={group.title ?? "primary"}>
              {group.title && surfaceMode !== "guided" && (
                <div className="mb-0.5 px-3 text-[9px] font-semibold uppercase tracking-[0.14em] text-slate-600">
                  {group.title}
                </div>
              )}
              <div className="space-y-px">
                {group.items.map((item) => {
                  const needsDesktop = Boolean(item.desktopOnly) && !isTauri();
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => {
                        setActiveView(item.id);
                        setNavOpenPersisted(false);
                      }}
                      className={`flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-[13px] transition ${
                        activeView === item.id
                          ? "bg-blue-500/12 text-blue-200"
                          : "text-slate-400 hover:bg-white/4 hover:text-slate-200"
                      }`}
                    >
                      <span className="w-4 text-center text-sm opacity-80">{item.icon}</span>
                      <span className="flex-1">{item.label}</span>
                      {needsDesktop && (
                        <span className="rounded bg-amber-400/10 px-1.5 py-0.5 text-[9px] uppercase tracking-wide text-amber-200/90">
                          Desktop
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </nav>

        <div className="mt-3 border-t border-white/6 pt-3">
          {surfaceMode === "guided" && (
            <button
              type="button"
              onClick={() => setSurfaceModePersisted("power")}
              className="mb-1 w-full rounded-lg px-3 py-2 text-left text-[11px] text-slate-500 hover:bg-white/4 hover:text-slate-300"
            >
              Full mode…
            </button>
          )}
          {surfaceMode === "power" && (
            <button
              type="button"
              onClick={toggleDevMode}
              title="Opens harness budgets, leases, and turn traces"
              className={`mb-1 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-xs transition ${
                devMode
                  ? "bg-amber-400/10 text-amber-200"
                  : "text-slate-500 hover:bg-white/4 hover:text-slate-300"
              }`}
            >
              <span
                className={`size-1.5 rounded-full ${devMode ? "bg-amber-400" : "bg-slate-600"}`}
              />
              Debug panels {devMode ? "on" : "off"}
            </button>
          )}
          {surfaceMode === "dev" && (
            <div className="mb-1 flex items-center gap-2 px-3 py-1.5 text-xs text-amber-200/80">
              <span className="size-1.5 rounded-full bg-amber-400" />
              Debug surface
            </div>
          )}
          {surfaceMode !== "guided" && (
            <div className="px-3 py-1.5 text-[10px] leading-4 text-slate-600">
              {isTauri() ? "Desktop" : "Browser preview"}
              {mcpServers.length > 0 ? ` · ${mcpServers.length} MCP` : ""}
            </div>
          )}
        </div>
      </aside>

      <main className="thin-scrollbar min-w-0 flex-1 overflow-y-auto">
        <header className="sticky top-0 z-10 flex h-12 items-center justify-between gap-2 border-b border-white/7 bg-[#080b11]/90 px-3 backdrop-blur-xl sm:h-14 sm:px-5">
          <div className="flex min-w-0 items-center gap-2">
            <button
              type="button"
              className={`grid size-8 shrink-0 place-items-center rounded-lg border border-white/10 bg-white/2.5 text-slate-300 ${
                surfaceMode === "guided" ? "" : "md:hidden"
              }`}
              aria-label="Open menu"
              onClick={() => setNavOpenPersisted(true)}
            >
              ☰
            </button>
            <div className="min-w-0">
              <h1 className="text-sm font-semibold leading-tight">
                {surfaceMode === "guided" && activeView === "Home" ? "ADE" : activeView}
                {devMode && surfaceMode !== "guided" && (
                  <span className="ml-2 rounded bg-amber-400/15 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-amber-200">
                    Debug
                  </span>
                )}
              </h1>
              {surfaceMode !== "guided" && (
                <p
                  className="mt-0.5 max-w-[36vw] truncate text-[10px] text-slate-500 sm:max-w-[52vw]"
                  title={dashboard?.workspace_root ?? undefined}
                >
                  {workspaceLeaf(dashboard?.workspace_root)}
                  {dashboard?.is_dogfood ? " · dogfood" : ""}
                  {!isTauri() && " · browser"}
                </p>
              )}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            {surfaceMode !== "guided" && (
              <div className="flex items-center overflow-hidden rounded-lg border border-white/10">
                <button
                  onClick={() => void runVerify()}
                  disabled={verifying}
                  className="bg-blue-500 px-3 py-1.5 text-xs font-semibold text-white transition hover:bg-blue-400 disabled:opacity-50"
                >
                  {verifying ? "…" : "Check"}
                </button>
                <select
                  value={gate}
                  onChange={(event) => setGate(event.target.value)}
                  title="Verify through gate"
                  aria-label="Verify through gate"
                  className="border-l border-white/10 bg-[#101620] px-1.5 py-1.5 text-[11px] text-slate-300"
                >
                  {["G0", "G1", "G2", "G3", "G4", "G5"].map((item) => (
                    <option key={item} value={item}>
                      {item}
                    </option>
                  ))}
                </select>
              </div>
            )}
            <button
              onClick={() => void refresh()}
              disabled={loading}
              aria-label="Refresh dashboard"
              className="grid size-8 place-items-center rounded-lg border border-white/10 bg-white/2.5 text-slate-400 hover:text-white disabled:opacity-50"
            >
              ↻
            </button>
          </div>
        </header>

        <div
          className={`mx-auto max-w-[1400px] p-4 ${
            surfaceMode === "guided" ? "sm:p-7" : "sm:p-5"
          }`}
        >
          {!isTauri() && (
            <div className="mb-5 rounded-xl border border-amber-400/25 bg-amber-400/8 px-4 py-3 text-[12px] leading-5 text-amber-100/90">
              Chat and keys need the Desktop app. This browser view is for status and checks.
            </div>
          )}
          {error && (
            <div className="mb-5 rounded-xl border border-red-400/20 bg-red-400/7 px-4 py-3 text-xs text-red-200">
              {error}
            </div>
          )}

          {loading && !dashboard ? (
            <LoadingState />
          ) : dashboard ? (
            <>
              {activeView === "Home" && (
                <HomeView
                  dashboard={dashboard}
                  scorePercent={scorePercent}
                  prompt={homePrompt}
                  onPromptChange={setHomePrompt}
                  agentBusy={agentBusy}
                  understandBusy={understandBusy}
                  verifying={verifying}
                  guidedWins={guidedWins}
                  lastUnderstandPath={lastUnderstandPath}
                  devMode={devMode}
                  simpleMode={surfaceMode === "guided"}
                  onOpenAgent={() => setActiveView("Agent")}
                  onOpenHealth={() => setActiveView("Health")}
                  onOpenRecipes={() => setActiveView("Recipes")}
                  onOpenKeys={() => setActiveView("Keys")}
                  onUnderstand={() => void runUnderstandProject()}
                  onVerifyHome={() => void runVerify({ stayOnHome: true })}
                  onImproveAde={startImproveAde}
                  onOpenAdeOnItself={() => void openAdeOnItself()}
                  onRunAgent={() => {
                    if (!homePrompt.trim()) return;
                    setAgentAutoSubmit(true);
                    setActiveView("Agent");
                  }}
                  onApplyPreset={(preset) => {
                    window.localStorage.setItem(AUTONOMY_KEY, preset.autonomy);
                    setHomePrompt(preset.prompt);
                    setAgentAutoSubmit(true);
                    setActiveView("Agent");
                  }}
                />
              )}
              {activeView === "Health" && (
                <Overview
                  dashboard={dashboard}
                  scorePercent={scorePercent}
                  verifyResults={verifyResults}
                  executing={executing}
                  onExecute={() => void executePlan()}
                  devMode={devMode}
                />
              )}
              {activeView === "Agent" && (
                <AgentView
                  events={agentEvents}
                  busy={agentBusy}
                  connectedTools={mcpTools.length}
                  initialPrompt={homePrompt}
                  autoSubmit={agentAutoSubmit}
                  onAutoSubmitHandled={() => setAgentAutoSubmit(false)}
                  sharedVerifyGate={gate}
                  devMode={devMode}
                  simpleMode={surfaceMode === "guided"}
                  leases={dashboard.leases}
                  planOwnedPaths={[
                    ...new Set(
                      dashboard.plan.phases.flatMap((phase) => phase.owned_paths),
                    ),
                  ]}
                  rebuildLockWarnings={dashboard.rebuild_lock_warnings ?? []}
                  onRun={(input) => void runAgentTurn(input)}
                />
              )}
              {activeView === "Keys" && (
                <KeysView
                  simpleMode={surfaceMode === "guided"}
                  onContinueToAgent={() => setActiveView("Agent")}
                />
              )}
              {activeView === "Audit" && <AuditView audit={dashboard.audit} />}
              {activeView === "Plan" && (
                <PlanMap
                  plan={dashboard.plan}
                  scorePercent={scorePercent}
                  verifyResults={verifyResults}
                  executing={executing}
                  onExecute={() => void executePlan()}
                  onRunAudit={() => void runAudit()}
                  onRunVerify={() => void runVerify()}
                />
              )}
              {activeView === "Atlas" && (
                <AtlasView
                  auditFindings={dashboard.audit.findings}
                  planPhases={dashboard.plan.phases}
                  verifyGates={verifyResults.map((r) => r.gate)}
                  handoffs={dashboard.handoff.recent}
                  onOpenGuidance={() => setActiveView("Rules")}
                  onOpenPlan={() => setActiveView("Plan")}
                />
              )}
              {activeView === "Verify" && (
                <VerifyView
                  results={verifyResults}
                  simpleMode={surfaceMode === "guided"}
                  onRun={() => void runVerify()}
                />
              )}
              {activeView === "Rules" && <RulesEditor />}
              {activeView === "MCP" && (
                <McpView
                  servers={mcpServers}
                  tools={mcpTools}
                  busy={mcpBusy}
                  workspaceRoot={dashboard.workspace_root}
                  onConnect={(input) => void connectMcp(input)}
                  onDisconnect={(name) => void disconnectMcp(name)}
                  onRefresh={() => void refreshMcp()}
                  onCallTool={callMcpTool}
                />
              )}
              {activeView === "Recipes" && (
                <RecipeWizard
                  recipes={recipes}
                  busy={recipeBusy}
                  plan={recipePlan}
                  planError={recipePlanError}
                  lastResult={recipeResult}
                  simpleMode={surfaceMode === "guided"}
                  onPreview={previewRecipe}
                  onInitialize={(input) => void initializeRecipe(input)}
                />
              )}
            </>
          ) : null}
        </div>
      </main>
    </div>
  );
}

function HomeView({
  dashboard,
  scorePercent,
  prompt,
  onPromptChange,
  agentBusy,
  understandBusy,
  verifying,
  guidedWins,
  lastUnderstandPath,
  devMode,
  simpleMode = false,
  onOpenAgent,
  onOpenHealth,
  onOpenRecipes,
  onOpenKeys,
  onUnderstand,
  onVerifyHome,
  onImproveAde,
  onOpenAdeOnItself,
  onRunAgent,
  onApplyPreset,
}: {
  dashboard: DashboardSnapshot;
  scorePercent: number;
  prompt: string;
  onPromptChange: (value: string) => void;
  agentBusy: boolean;
  understandBusy: boolean;
  verifying: boolean;
  guidedWins: GuidedWinsState;
  lastUnderstandPath: string | null;
  devMode: boolean;
  simpleMode?: boolean;
  onOpenAgent: () => void;
  onOpenHealth: () => void;
  onOpenRecipes: () => void;
  onOpenKeys: () => void;
  onUnderstand: () => void;
  onVerifyHome: () => void;
  onImproveAde: () => void;
  onOpenAdeOnItself: () => void;
  onRunAgent: () => void;
  onApplyPreset: (preset: (typeof PROMPT_PRESETS)[number]) => void;
}) {
  const latestHandoff = dashboard.handoff.recent[0];
  const winsDone =
    Number(guidedWins.understand) + Number(guidedWins.verify) + Number(guidedWins.improve_ade);
  const nextWin = !guidedWins.understand
    ? "Learn this project"
    : !guidedWins.verify
      ? "Check that things still work"
      : !guidedWins.improve_ade
        ? "Try a small safe change"
        : null;

  const starters = [
    {
      id: "understand" as const,
      title: "Learn this project",
      detail: "Write a short project snapshot you can reuse",
      done: guidedWins.understand,
      busy: understandBusy,
      onClick: onUnderstand,
    },
    {
      id: "verify" as const,
      title: "Check that things still work",
      detail: "Run ADE’s built-in checks on this workspace",
      done: guidedWins.verify,
      busy: verifying,
      onClick: onVerifyHome,
    },
    {
      id: "improve" as const,
      title: "Try a small safe change",
      detail: "Open Agent with a careful, check-after change",
      done: guidedWins.improve_ade,
      busy: agentBusy,
      onClick: onImproveAde,
    },
  ];

  return (
    <div className="mx-auto max-w-3xl space-y-5">
      <section className="rounded-2xl border border-white/8 bg-[#0c121c] px-5 py-5 sm:px-6 sm:py-6">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h2 className="text-2xl font-semibold tracking-tight text-slate-50 sm:text-3xl">ADE</h2>
            <p className="mt-1 text-sm text-slate-400">
              {simpleMode ? "Tell ADE what to do." : "Composer first. Sidebar for the rest."}
            </p>
          </div>
          <p className="text-[11px] text-slate-500">
            {simpleMode
              ? `${winsDone}/3 ready`
              : `${scorePercent}% ready`}
            {dashboard.is_dogfood ? " · dogfood" : ""}
          </p>
        </div>

        <div className="mt-4 flex flex-col gap-3 sm:flex-row sm:items-stretch">
          <textarea
            value={prompt}
            onChange={(event) => onPromptChange(event.target.value)}
            rows={simpleMode ? 4 : 3}
            placeholder="Describe what you want help with…"
            className={`w-full flex-1 resize-y rounded-xl border border-white/10 bg-black/25 px-4 py-3 text-sm text-slate-200 outline-none ring-blue-400/30 placeholder:text-slate-600 focus:ring-2 ${
              simpleMode ? "min-h-[110px]" : "min-h-[88px]"
            }`}
          />
          <button
            type="button"
            onClick={onRunAgent}
            disabled={!prompt.trim() || agentBusy || !isTauri()}
            className="shrink-0 rounded-xl bg-blue-500 px-5 py-3 text-sm font-semibold text-white hover:bg-blue-400 disabled:opacity-50 sm:min-w-[7.5rem]"
          >
            {agentBusy ? "…" : isTauri() ? "Go" : "Desktop"}
          </button>
        </div>

        <div className="mt-3 flex flex-wrap gap-1.5">
          {PROMPT_PRESETS.map((item) => (
            <Chip
              key={item.label}
              onClick={() => onApplyPreset(item)}
              title={`${item.autonomy === "act" ? "Apply" : "Suggest"} · ${item.prompt}`}
            >
              {item.label}
            </Chip>
          ))}
        </div>

        {simpleMode && (
          <div className="mt-5 flex flex-wrap items-center gap-2 border-t border-white/6 pt-4">
            <button
              type="button"
              onClick={onOpenKeys}
              className="rounded-lg border border-white/10 px-3 py-2 text-[11px] font-semibold text-slate-300 hover:bg-white/5"
            >
              {isTauri() ? "Add API key" : "Add API key (Desktop)"}
            </button>
            {nextWin ? (
              <button
                type="button"
                disabled={understandBusy || verifying || agentBusy}
                onClick={() => {
                  if (!guidedWins.understand) onUnderstand();
                  else if (!guidedWins.verify) onVerifyHome();
                  else onImproveAde();
                }}
                className="rounded-lg bg-blue-500/90 px-3 py-2 text-[11px] font-semibold text-white hover:bg-blue-400 disabled:opacity-50"
              >
                Next: {nextWin}
              </button>
            ) : (
              <button
                type="button"
                onClick={onOpenAgent}
                className="rounded-lg border border-blue-400/30 px-3 py-2 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/10"
              >
                Open Agent
              </button>
            )}
          </div>
        )}

        {simpleMode && (
          <Disclosure
            title="Guided steps"
            summary={`${winsDone}/3`}
            subtitle="Optional path — Learn, Check, then a small safe change"
            defaultOpen={winsDone < 3}
            storageKey="ade_home_guided_steps"
            className="mt-4"
          >
            <div className="space-y-2">
              {starters.map((starter) => (
                <button
                  key={starter.id}
                  type="button"
                  disabled={starter.busy}
                  onClick={starter.onClick}
                  className={`flex w-full items-center justify-between gap-3 rounded-lg border px-3 py-2.5 text-left transition disabled:opacity-60 ${
                    starter.done
                      ? "border-emerald-400/25 bg-emerald-500/8"
                      : "border-white/8 bg-white/3 hover:border-blue-400/30"
                  }`}
                >
                  <div>
                    <div className="text-[12px] font-medium text-slate-200">{starter.title}</div>
                    <div className="text-[10px] text-slate-500">{starter.detail}</div>
                  </div>
                  <span className="text-[10px] uppercase tracking-wide text-slate-500">
                    {starter.done ? "done" : starter.busy ? "…" : "run"}
                  </span>
                </button>
              ))}
              {guidedWins.understand && lastUnderstandPath && (
                <p className="pt-1 font-mono text-[10px] text-emerald-200/70">
                  {lastUnderstandPath}
                </p>
              )}
            </div>
          </Disclosure>
        )}

        {!dashboard.is_dogfood && dashboard.ade_source_root && (
          <button
            type="button"
            onClick={onOpenAdeOnItself}
            className="mt-4 text-[11px] font-semibold text-blue-200/80 hover:text-blue-100"
          >
            Open ADE on itself →
          </button>
        )}

        {!simpleMode && (
          <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-white/6 pt-3 text-[11px] text-slate-500">
            <button
              type="button"
              onClick={onOpenHealth}
              className="font-medium text-slate-300 hover:text-blue-200"
            >
              {scorePercent}% ready
            </button>
            <span className="text-slate-700">·</span>
            <button
              type="button"
              onClick={onOpenHealth}
              className="hover:text-slate-300"
              title={
                latestHandoff
                  ? `${latestHandoff.turn_status ?? "capsule"} · ${latestHandoff.id}`
                  : undefined
              }
            >
              {dashboard.handoff.capsule_count} handoff
              {dashboard.handoff.capsule_count === 1 ? "" : "s"}
            </button>
            <span className="text-slate-700">·</span>
            <button type="button" onClick={onOpenRecipes} className="hover:text-slate-300">
              Recipes
            </button>
          </div>
        )}
      </section>

      {devMode && (
        <section className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-100/80">
          <span className="font-mono text-amber-100/70">{dashboard.workspace_root}</span>
          <span className="text-amber-100/50">
            {" "}
            · leases {dashboard.leases.length} · guided {winsDone}/3
          </span>
        </section>
      )}
    </div>
  );
}

function Overview({
  dashboard,
  scorePercent,
  verifyResults,
  executing,
  onExecute,
  devMode = false,
}: {
  dashboard: DashboardSnapshot;
  scorePercent: number;
  verifyResults: VerifyResult[];
  executing: boolean;
  onExecute: () => void;
  devMode?: boolean;
}) {
  const passed = verifyResults.filter((result) => result.passed).length;
  const openTasks = dashboard.tasks.filter(
    (task) => !["completed", "failed", "cancelled"].includes(task.status),
  ).length;
  const [globalAudit, setGlobalAudit] = useState<{
    ok: boolean;
    checks: { id: string; label: string; passed: boolean; detail: string }[];
  } | null>(null);

  useEffect(() => {
    void invoke<{
      ok: boolean;
      checks: { id: string; label: string; passed: boolean; detail: string }[];
    }>("run_global_audit")
      .then(setGlobalAudit)
      .catch(() => setGlobalAudit(null));
  }, [dashboard.workspace_root]);

  return (
    <div className="space-y-4">
      {globalAudit && (
        <div
          className={`rounded-lg border px-3 py-2 text-[11px] ${
            globalAudit.ok
              ? "border-emerald-400/20 bg-emerald-400/5 text-emerald-100/85"
              : "border-amber-400/25 bg-amber-400/8 text-amber-100/90"
          }`}
        >
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="font-semibold">
              Machine · {globalAudit.ok ? "ADE ready" : "ADE attention"}
            </span>
            <button
              type="button"
              className="text-[10px] text-slate-400 hover:text-slate-200"
              onClick={() =>
                void invoke<typeof globalAudit>("run_global_audit").then(setGlobalAudit)
              }
            >
              Re-check
            </button>
          </div>
          <div className="mt-1.5 flex flex-wrap gap-x-3 gap-y-1 text-slate-400">
            {globalAudit.checks.map((check) => (
              <span key={check.id} title={check.detail}>
                {check.passed ? "✓" : "!"} {check.label}
              </span>
            ))}
          </div>
        </div>
      )}
      {devMode && (
        <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-1.5 text-[11px] text-amber-100/80">
          Health · score {dashboard.audit.score}/{dashboard.audit.score_max} · leases{" "}
          {dashboard.leases.length}
        </div>
      )}
      <section
        className={`grid gap-2 ${
          devMode
            ? "grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-7"
            : "grid-cols-2 sm:grid-cols-4"
        }`}
      >
        <MetricCard dense label="Ready" value={`${scorePercent}%`} accent="blue" />
        <MetricCard
          dense
          label="Blockers"
          value={String(dashboard.audit.blockers.length)}
          accent={dashboard.audit.blockers.length ? "red" : "green"}
        />
        <MetricCard
          dense
          label="Verify"
          value={verifyResults.length ? `${passed}/${verifyResults.length}` : "—"}
          accent={verifyResults.length && passed === verifyResults.length ? "green" : "slate"}
        />
        <MetricCard
          dense
          label="Handoffs"
          value={String(dashboard.handoff.capsule_count)}
          accent={dashboard.handoff.invalid_capsule_count ? "red" : "green"}
        />
        {devMode && (
          <>
            <MetricCard
              dense
              label="Phases"
              value={String(dashboard.plan.phases.length)}
              accent="violet"
            />
            <MetricCard
              dense
              label="Leases"
              value={String(dashboard.leases.length)}
              accent={dashboard.leases.length ? "violet" : "slate"}
            />
            <MetricCard
              dense
              label="Tasks"
              value={String(openTasks)}
              accent={dashboard.tasks.some((task) => task.status === "running") ? "blue" : "slate"}
            />
          </>
        )}
      </section>

      <section className="grid grid-cols-1 gap-4 lg:grid-cols-[1.4fr_1fr]">
        <Panel title="Environment" subtitle="L0–L11 audit" dense>
          <div className="flex gap-5">
            <div
              className="score-ring grid size-24 shrink-0 place-items-center rounded-full p-1.5 sm:size-28"
              style={
                {
                  "--score-angle": `${scorePercent * 3.6}deg`,
                } as React.CSSProperties
              }
            >
              <div className="grid size-full place-items-center rounded-full bg-[#0d121a] text-center">
                <div>
                  <div className="text-xl font-semibold sm:text-2xl">{scorePercent}%</div>
                  <div className="text-[10px] text-slate-500">
                    {dashboard.audit.score}/{dashboard.audit.score_max}
                  </div>
                </div>
              </div>
            </div>
            <div className="min-w-0 flex-1 space-y-2.5">
              {dashboard.audit.findings.slice(0, 6).map((finding) => (
                <FindingBar key={finding.layer} finding={finding} />
              ))}
            </div>
          </div>
        </Panel>

        <Panel title="Plan" subtitle="Execution scope" dense>
          {dashboard.plan.phases.length === 0 ? (
            <div className="flex h-44 flex-col items-center justify-center text-center">
              <div className="grid size-10 place-items-center rounded-full bg-emerald-400/10 text-emerald-300">
                ✓
              </div>
              <div className="mt-3 text-sm font-medium">No remediation needed</div>
              <p className="mt-1 max-w-60 text-xs leading-5 text-slate-500">
                The current audit has no actionable gaps. ADE is ready for focused work.
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {dashboard.plan.phases.slice(0, 3).map((phase) => (
                <div key={phase.id} className="rounded-lg border border-white/7 bg-white/2 p-3">
                  <div className="text-xs font-medium">{phase.title}</div>
                  <div className="mt-1 text-[10px] text-slate-600">{phase.id}</div>
                </div>
              ))}
              <button
                onClick={onExecute}
                disabled={executing}
                className="w-full rounded-lg bg-violet-500/90 py-2 text-xs font-semibold hover:bg-violet-400 disabled:opacity-50"
              >
                {executing ? "Executing…" : "Review and execute"}
              </button>
            </div>
          )}
        </Panel>
      </section>

      {devMode && (
        <Panel title="Leases" subtitle="Durable path ownership" dense>
          {dashboard.leases.length === 0 ? (
            <p className="text-xs text-slate-500">
              No active leases. Writes need PLAN owned paths or a bound lease.
            </p>
          ) : (
            <div className="space-y-2">
              {dashboard.leases.map((lease) => (
                <div
                  key={lease.id}
                  className="grid grid-cols-[1fr_110px_1fr_130px] items-center gap-3 border-b border-white/5 py-2 text-[11px]"
                >
                  <span className="font-mono text-slate-300">{lease.path}</span>
                  <span
                    className={
                      lease.mode === "exclusive" || lease.mode === "strong"
                        ? "text-violet-300"
                        : lease.mode === "cooperative"
                          ? "text-blue-300"
                          : "text-slate-500"
                    }
                  >
                    {lease.mode}
                    {lease.protected ? " · protected" : ""}
                  </span>
                  <span className="truncate font-mono text-slate-600">{lease.agent_id}</span>
                  <span className="text-right text-slate-600">
                    {new Date(lease.expires_at).toLocaleTimeString()}
                  </span>
                </div>
              ))}
            </div>
          )}
        </Panel>
      )}

      {devMode && (
        <Panel title="Task queue" subtitle="Lease-backed claims" dense>
          {dashboard.tasks.length === 0 ? (
            <p className="text-xs text-slate-500">
              No queued tasks. Use <span className="font-mono">ade task enqueue --approve</span>.
            </p>
          ) : (
            <div className="space-y-2">
              {dashboard.tasks.slice(-8).map((task) => (
                <div
                  key={task.id}
                  className="grid grid-cols-[90px_1fr_150px_120px] items-center gap-3 border-b border-white/5 py-2 text-[11px]"
                >
                  <span
                    className={
                      task.status === "running"
                        ? "text-blue-300"
                      : task.status === "completed"
                        ? "text-emerald-300"
                        : task.status === "failed"
                          ? "text-red-300"
                          : "text-slate-400"
                  }
                >
                  {task.status}
                </span>
                <span className="truncate text-slate-300">{task.goal}</span>
                <span className="truncate font-mono text-slate-600">
                  {task.agent_id ?? "unassigned"}
                </span>
                <span className="text-right text-slate-600">
                  {task.owned_paths.length} owned path
                  {task.owned_paths.length === 1 ? "" : "s"}
                </span>
              </div>
            ))}
          </div>
        )}
      </Panel>
      )}

      <Panel title="Continuity" subtitle="Handoff metadata only" dense>
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
          <div>
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Score
            </div>
            <div className="mt-1 text-base font-semibold text-slate-200">
              {dashboard.handoff.latest_score_before !== null
                ? dashboard.handoff.latest_score_after !== null
                  ? `${dashboard.handoff.latest_score_before} → ${dashboard.handoff.latest_score_after}`
                  : String(dashboard.handoff.latest_score_before)
                : "—"}
              {dashboard.handoff.latest_score_max !== null
                ? ` / ${dashboard.handoff.latest_score_max}`
                : ""}
            </div>
            {dashboard.handoff.latest_score_delta !== null && (
              <div
                className={`mt-0.5 text-[10px] ${
                  dashboard.handoff.latest_score_delta >= 0
                    ? "text-emerald-400"
                    : "text-red-400"
                }`}
              >
                {dashboard.handoff.latest_score_delta >= 0 ? "+" : ""}
                {dashboard.handoff.latest_score_delta} pts
              </div>
            )}
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Context
            </div>
            <div className="mt-1 text-base font-semibold text-slate-200">
              {dashboard.handoff.latest_context_status
                ? dashboard.handoff.latest_context_status
                : dashboard.handoff.latest_bytes
                  ? `${dashboard.handoff.latest_compaction_percent}%`
                  : "—"}
            </div>
            <div className="mt-0.5 text-[10px] text-slate-600">
              {dashboard.handoff.latest_context_tokens !== null
                ? `${dashboard.handoff.latest_context_tokens} tok · `
                : ""}
              {dashboard.handoff.latest_summary_chars} / {dashboard.handoff.latest_bytes} B
            </div>
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              State
            </div>
            <div className="mt-1 text-base font-semibold capitalize text-slate-200">
              {dashboard.handoff.latest_status?.replaceAll("_", " ") ?? "None"}
            </div>
            <div className="mt-0.5 truncate text-[10px] text-slate-600">
              {dashboard.handoff.latest_created_at ?? "—"}
            </div>
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Archive
            </div>
            <div className="mt-1 text-base font-semibold text-slate-200">
              {dashboard.handoff.invalid_capsule_count === 0 ? "Valid" : "Attention"}
            </div>
            <div className="mt-0.5 text-[10px] text-slate-600">
              {dashboard.handoff.invalid_capsule_count} invalid ·{" "}
              {dashboard.handoff.total_bytes.toLocaleString()} B
            </div>
          </div>
        </div>

        {devMode && dashboard.handoff.latest_context_sections.length > 0 && (
          <div className="mt-4 space-y-2">
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Prompt sections
            </div>
            {dashboard.handoff.latest_context_sections.map((section) => {
              const maxTokens = Math.max(
                ...dashboard.handoff.latest_context_sections.map((item) => item.tokens),
                1,
              );
              const width = Math.round((section.tokens / maxTokens) * 100);
              return (
                <div key={section.name} className="space-y-1">
                  <div className="flex items-center justify-between text-[11px] text-slate-400">
                    <span className="capitalize">{section.name}</span>
                    <span>
                      {section.tokens} tok
                      {section.truncated ? " · truncated" : ""}
                    </span>
                  </div>
                  <div className="h-1.5 overflow-hidden rounded-full bg-white/5">
                    <div
                      className={`h-full rounded-full ${
                        section.truncated ? "bg-amber-400/70" : "bg-blue-400/70"
                      }`}
                      style={{ width: `${width}%` }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {dashboard.handoff.recent.length > 0 && (
          <div className="mt-5 space-y-2">
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Recent score deltas
            </div>
            {dashboard.handoff.recent.slice(0, 5).map((item) => (
              <div
                key={item.id}
                className="flex items-center justify-between border-b border-white/5 py-2 text-[11px]"
              >
                <div className="min-w-0 text-slate-400">
                  <span className="capitalize text-slate-300">
                    {item.turn_status?.replaceAll("_", " ") ?? "unknown"}
                  </span>
                  <span className="ml-2 text-slate-600">
                    {item.score_before ?? "-"} → {item.score_after ?? "-"}
                    {item.score_max !== null ? ` / ${item.score_max}` : ""}
                  </span>
                </div>
                <div
                  className={
                    item.score_delta === null
                      ? "text-slate-600"
                      : item.score_delta >= 0
                        ? "text-emerald-400"
                        : "text-red-400"
                  }
                >
                  {item.score_delta === null
                    ? "n/a"
                    : `${item.score_delta >= 0 ? "+" : ""}${item.score_delta}`}
                </div>
              </div>
            ))}
          </div>
        )}
      </Panel>

      <Panel title="Audit findings" subtitle="All architecture layers, ordered by contract">
        <div className="grid grid-cols-2 gap-x-6 gap-y-1">
          {dashboard.audit.findings.map((finding) => (
            <div
              key={finding.layer}
              className="flex items-center justify-between border-b border-white/5 py-3"
            >
              <div className="min-w-0">
                <div className="text-xs font-medium text-slate-300">{finding.layer}</div>
                <div className="mt-1 truncate pr-4 text-[11px] text-slate-600">
                  {finding.detail}
                </div>
              </div>
              <StatusPill severity={finding.severity} />
            </div>
          ))}
        </div>
      </Panel>
    </div>
  );
}

function AgentView({
  events,
  busy,
  connectedTools,
  onRun,
  initialPrompt = "",
  autoSubmit = false,
  onAutoSubmitHandled,
  sharedVerifyGate = "G3",
  devMode = false,
  simpleMode = false,
  leases = [],
  planOwnedPaths = [],
  rebuildLockWarnings = [],
}: {
  events: AgentEvent[];
  busy: boolean;
  connectedTools: number;
  initialPrompt?: string;
  autoSubmit?: boolean;
  onAutoSubmitHandled?: () => void;
  sharedVerifyGate?: string;
  devMode?: boolean;
  simpleMode?: boolean;
  leases?: PathLease[];
  planOwnedPaths?: string[];
  rebuildLockWarnings?: string[];
  onRun: (input: {
    prompt: string;
    provider: string;
    baseUrl: string;
    model: string;
    inputCostPerMtok: number;
    outputCostPerMtok: number;
    sessionCapUsd: number;
    dailyCapUsd: number;
    autonomy: AutonomyLevel;
    maxSteps: number;
    maxTokens: number | null;
    verifyOnComplete: boolean;
    verifyGate: string;
    approveOwnedPaths: boolean;
    ownedPaths: string[];
  }) => void;
}) {
  const [prompt, setPrompt] = useState(initialPrompt);
  const [provider, setProvider] = useState(() => {
    if (typeof window === "undefined") return "openai";
    return window.localStorage.getItem(AGENT_PROVIDER_KEY) || "openai";
  });
  const [baseUrl, setBaseUrl] = useState(() => {
    if (typeof window === "undefined") return "https://api.openai.com/v1";
    return window.localStorage.getItem(AGENT_BASE_URL_KEY) || "https://api.openai.com/v1";
  });
  const [model, setModel] = useState(() => {
    if (typeof window === "undefined") return DEFAULT_MODEL;
    return window.localStorage.getItem(AGENT_MODEL_KEY) || DEFAULT_MODEL;
  });
  const [inputCost, setInputCost] = useState("0");
  const [outputCost, setOutputCost] = useState("0");
  const [sessionCap, setSessionCap] = useState("1");
  const [dailyCap, setDailyCap] = useState("10");
  const [autonomy, setAutonomy] = useState<AutonomyLevel>(readAutonomy);
  const [maxSteps, setMaxSteps] = useState("8");
  const [maxTokens, setMaxTokens] = useState("");
  const [verifyOnComplete, setVerifyOnComplete] = useState(false);
  const [verifyGate, setVerifyGate] = useState(sharedVerifyGate);
  const [approveOwnedPaths, setApproveOwnedPaths] = useState(false);

  const mutating = autonomy === "act" || autonomy === "automate";
  // Budgets/leases/traces stay behind Debug panels (Full) or Debug surface.
  const showDebugHarness = !simpleMode && devMode;

  useEffect(() => {
    if (sharedVerifyGate.trim()) {
      setVerifyGate(sharedVerifyGate);
    }
  }, [sharedVerifyGate]);

  useEffect(() => {
    if (initialPrompt.trim()) {
      setPrompt(initialPrompt);
    }
  }, [initialPrompt]);

  const setAutonomyPersisted = (level: AutonomyLevel) => {
    setAutonomy(level);
    window.localStorage.setItem(AUTONOMY_KEY, level);
    if (level === "act" || level === "automate") {
      setApproveOwnedPaths(true);
    } else {
      setApproveOwnedPaths(false);
    }
  };

  useEffect(() => {
    window.localStorage.setItem(AGENT_PROVIDER_KEY, provider);
  }, [provider]);
  useEffect(() => {
    window.localStorage.setItem(AGENT_BASE_URL_KEY, baseUrl);
  }, [baseUrl]);
  useEffect(() => {
    window.localStorage.setItem(AGENT_MODEL_KEY, model);
  }, [model]);

  useEffect(() => {
    if (mutating) {
      setApproveOwnedPaths(true);
    }
  }, [mutating, planOwnedPaths.length]);

  useEffect(() => {
    if (simpleMode && (autonomy === "observe" || autonomy === "automate")) {
      setAutonomyPersisted(autonomy === "automate" ? "act" : "propose");
    } else if (simpleMode && autonomy !== "propose" && autonomy !== "act") {
      setAutonomyPersisted("propose");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- one-shot remap when entering Simple
  }, [simpleMode]);

  useEffect(() => {
    if (autonomy === "automate") {
      setVerifyOnComplete(true);
    }
    if (simpleMode && mutating) {
      setVerifyOnComplete(true);
    }
    if (autonomy === "observe" || autonomy === "propose") {
      setApproveOwnedPaths(false);
      if (simpleMode) {
        setVerifyOnComplete(false);
      }
    }
  }, [autonomy, simpleMode, mutating]);

  const buildTurnInput = () => ({
    prompt: prompt.trim(),
    provider: provider.trim(),
    baseUrl: baseUrl.trim(),
    model: model.trim(),
    inputCostPerMtok: Number(inputCost) || 0,
    outputCostPerMtok: Number(outputCost) || 0,
    sessionCapUsd: Number(sessionCap),
    dailyCapUsd: Number(dailyCap),
    autonomy,
    maxSteps: Math.max(1, Number(maxSteps) || 8),
    maxTokens: maxTokens.trim() ? Number(maxTokens) || null : null,
    verifyOnComplete: autonomy === "automate" ? true : verifyOnComplete,
    verifyGate: verifyGate.trim() || "G3",
    approveOwnedPaths: mutating ? approveOwnedPaths : false,
    ownedPaths: mutating && approveOwnedPaths ? planOwnedPaths : [],
  });

  const submit = () => {
    onRun(buildTurnInput());
  };

  useEffect(() => {
    if (!autoSubmit) return;
    onAutoSubmitHandled?.();
    const nextPrompt = (initialPrompt.trim() || prompt).trim();
    if (!nextPrompt || !provider.trim() || !model.trim() || busy || !isTauri()) {
      return;
    }
    if (initialPrompt.trim()) {
      setPrompt(initialPrompt);
    }
    onRun({
      ...buildTurnInput(),
      prompt: nextPrompt,
    });
    // Intentionally one-shot when autoSubmit flips true
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoSubmit]);

  const text = events
    .filter((event): event is Extract<AgentEvent, { type: "text_delta" }> =>
      event.type === "text_delta",
    )
    .map((event) => event.text)
    .join("");
  const completed = [...events].reverse().find(
    (event): event is Extract<AgentEvent, { type: "completed" }> =>
      event.type === "completed",
  );
  const failed = [...events].reverse().find(
    (event): event is Extract<AgentEvent, { type: "failed" }> => event.type === "failed",
  );
  const verifyEvent = [...events].reverse().find(
    (event): event is Extract<AgentEvent, { type: "verify_complete" }> =>
      event.type === "verify_complete",
  );
  const activity = events.filter(
    (event) => event.type === "tool_call" || event.type === "tool_result",
  );
  const usageEvents = events.filter(
    (event): event is Extract<AgentEvent, { type: "usage" }> => event.type === "usage",
  );
  const spendWarnings = events.filter(
    (event): event is Extract<AgentEvent, { type: "spend_warning" }> =>
      event.type === "spend_warning",
  );

  return (
    <div className="space-y-5">
      {rebuildLockWarnings.length > 0 && !simpleMode && (
        <div className="rounded-xl border border-amber-400/25 bg-amber-400/8 px-4 py-3 text-[11px] leading-5 text-amber-100/85">
          <div className="font-semibold uppercase tracking-wider text-amber-200/80">
            Rebuild lock
          </div>
          <ul className="mt-1 space-y-1">
            {rebuildLockWarnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        </div>
      )}

      <div className={`grid grid-cols-1 gap-4 ${simpleMode ? "" : "lg:grid-cols-[1fr_280px]"}`}>
        <Panel
          title={simpleMode ? "Agent session" : "Agent"}
          subtitle={
            simpleMode
              ? undefined
              : connectedTools > 0
                ? `${connectedTools} MCP · ${autonomy}`
                : autonomy
          }
          dense={!simpleMode}
        >
          <div className="mb-4">
            {simpleMode ? (
              <>
                <div className="mb-2 flex items-center gap-2 text-[10px] uppercase tracking-wider text-slate-500">
                  How should ADE help?
                  <Hint text="Suggest only never writes. Apply changes edits PLAN owned paths automatically." />
                </div>
                <div className="grid grid-cols-2 gap-1.5">
                  <button
                    type="button"
                    onClick={() => setAutonomyPersisted("propose")}
                    className={`rounded-lg border px-3 py-2.5 text-left transition ${
                      autonomy === "propose" || autonomy === "observe"
                        ? "border-blue-400/40 bg-blue-500/20 text-blue-100"
                        : "border-white/8 bg-white/2 text-slate-400 hover:bg-white/4"
                    }`}
                  >
                    <div className="text-[12px] font-semibold">Suggest only</div>
                    <div className="mt-0.5 text-[10px] text-slate-500">No file edits</div>
                  </button>
                  <button
                    type="button"
                    onClick={() => setAutonomyPersisted("act")}
                    className={`rounded-lg border px-3 py-2.5 text-left transition ${
                      mutating
                        ? "border-blue-400/40 bg-blue-500/20 text-blue-100"
                        : "border-white/8 bg-white/2 text-slate-400 hover:bg-white/4"
                    }`}
                  >
                    <div className="text-[12px] font-semibold">Apply changes</div>
                    <div className="mt-0.5 text-[10px] text-slate-500">Edit planned files</div>
                  </button>
                </div>
              </>
            ) : (
              <>
                <div className="mb-1.5 flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-slate-500">
                  Autonomy
                  <Hint text="Observe=read-only. Propose=plan only. Act=owned paths. Automate=caps + verify." />
                </div>
                <div className="grid grid-cols-4 gap-1">
                  {AUTONOMY_LEVELS.map((level) => {
                    const active = autonomy === level.id;
                    return (
                      <button
                        key={level.id}
                        type="button"
                        title={level.hint}
                        onClick={() => setAutonomyPersisted(level.id)}
                        className={`rounded-md border px-1.5 py-1.5 text-center text-[11px] font-semibold transition ${
                          active
                            ? "border-blue-400/40 bg-blue-500/20 text-blue-100"
                            : "border-white/8 bg-white/2 text-slate-400 hover:bg-white/4"
                        }`}
                      >
                        {level.label}
                      </button>
                    );
                  })}
                </div>
                {mutating && (
                  <label className="mt-2 flex items-start gap-2 rounded-md border border-white/8 bg-white/2 px-2.5 py-1.5 text-[11px] text-slate-300">
                    <input
                      type="checkbox"
                      className="mt-0.5"
                      checked={approveOwnedPaths}
                      onChange={(event) => setApproveOwnedPaths(event.target.checked)}
                    />
                    <span>
                      Approve PLAN writes
                      <span className="mt-0.5 block text-[10px] text-slate-500">
                        {planOwnedPaths.length > 0
                          ? `${planOwnedPaths.length}: ${planOwnedPaths.slice(0, 3).join(", ")}${planOwnedPaths.length > 3 ? "…" : ""}`
                          : "Builds PLAN owned_paths if missing"}
                      </span>
                    </span>
                  </label>
                )}
                <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1.5 rounded-md border border-white/7 bg-black/15 px-2.5 py-1.5 text-[11px] text-slate-400">
                  <label className="flex items-center gap-1.5">
                    <input
                      type="checkbox"
                      checked={autonomy === "automate" ? true : verifyOnComplete}
                      disabled={autonomy === "automate"}
                      onChange={(event) => setVerifyOnComplete(event.target.checked)}
                      className="accent-blue-500"
                    />
                    Verify after
                    {autonomy === "automate" ? " (req)" : ""}
                  </label>
                  <select
                    value={verifyGate}
                    onChange={(event) => setVerifyGate(event.target.value)}
                    title="Verify gate"
                    className="rounded border border-white/10 bg-[#101620] px-1.5 py-0.5 text-[11px] text-slate-300"
                  >
                    {["G0", "G1", "G2", "G3", "G4", "G5"].map((item) => (
                      <option key={item} value={item}>
                        {item}
                      </option>
                    ))}
                  </select>
                </div>
              </>
            )}
          </div>

          <div className="mb-2 flex flex-wrap gap-1.5">
            {PROMPT_PRESETS.map((item) => (
              <Chip
                key={item.label}
                onClick={() => {
                  setAutonomyPersisted(item.autonomy);
                  setPrompt(item.prompt);
                }}
                title={`${item.autonomy === "act" ? "Apply" : "Suggest"} · ${item.prompt}`}
              >
                {item.label}
              </Chip>
            ))}
          </div>
          <textarea
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            rows={simpleMode ? 4 : 3}
            className="thin-scrollbar w-full rounded-xl border border-white/10 bg-[#101620] px-3 py-2.5 text-sm leading-6 text-slate-200"
            placeholder="What should ADE help you accomplish?"
          />
          <div className="mb-4 mt-2.5 flex items-center justify-between gap-3">
            <span className="text-[10px] text-slate-600">
              {simpleMode
                ? autonomy === "propose" || autonomy === "observe"
                  ? "Suggest only"
                  : "Apply changes"
                : `${provider} · ${model || DEFAULT_MODEL}`}
            </span>
            <button
              onClick={submit}
              disabled={
                busy || !prompt.trim() || !provider.trim() || !model.trim() || !isTauri()
              }
              className="rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
            >
              {busy
                ? "…"
                : isTauri()
                  ? "Go"
                  : "Desktop"}
            </button>
          </div>

          {simpleMode && (
            <Disclosure
              title="Model"
              summary={`${provider} · ${model || DEFAULT_MODEL}`}
              hint="Key stays in Keys. Expand to change provider or model."
              defaultOpen={false}
              storageKey="ade_agent_model_open"
              className="mb-5"
            >
              <div className="space-y-3">
                <ChipRow label="Provider">
                  {PROVIDER_PRESETS.map((preset) => (
                    <Chip
                      key={preset.id}
                      active={provider === preset.id}
                      onClick={() => {
                        setProvider(preset.id);
                        setBaseUrl(preset.baseUrl);
                        if (
                          !model.trim() ||
                          !PROVIDER_PRESETS.some((p) => p.models.includes(model))
                        ) {
                          setModel(preset.models[0] ?? DEFAULT_MODEL);
                        }
                      }}
                    >
                      {preset.id}
                    </Chip>
                  ))}
                </ChipRow>
                <ChipRow label="Model">
                  {(
                    PROVIDER_PRESETS.find((preset) => preset.id === provider)?.models ?? [
                      DEFAULT_MODEL,
                    ]
                  ).map((id) => (
                    <Chip key={id} active={model === id} onClick={() => setModel(id)}>
                      {id}
                    </Chip>
                  ))}
                </ChipRow>
              </div>
            </Disclosure>
          )}

          <div className="min-h-40 rounded-xl border border-white/7 bg-black/20 p-4">
            {!text && !busy ? (
              <div className="grid min-h-32 place-items-center text-center">
                <p className="max-w-sm text-xs leading-5 text-slate-600">
                  Responses appear here after you Go.
                </p>
              </div>
            ) : (
              <div className="whitespace-pre-wrap text-sm leading-7 text-slate-300">
                {text}
                {busy && <span className="ml-1 inline-block size-1.5 animate-pulse bg-blue-300" />}
              </div>
            )}
          </div>


          {activity.length > 0 && (
            <div className="mt-3 space-y-2">
              {activity.map((event, index) => (
                <div
                  key={`${event.type}-${index}`}
                  className="rounded-lg border border-white/7 bg-white/2 px-3 py-2 text-[11px]"
                >
                  {event.type === "tool_call" ? (
                    <span className="text-amber-200">
                      {simpleMode
                        ? event.effect === "ReadOnly"
                          ? "Looking up…"
                          : event.effect === "WorkspaceWrite"
                            ? "Writing…"
                            : "Working…"
                        : `→ ${event.server}/${event.tool}`}
                      {!simpleMode && event.effect ? (
                        <span className="ml-2 text-slate-500">{event.effect}</span>
                      ) : null}
                      {simpleMode ? (
                        <span className="ml-2 text-slate-500">
                          {event.server}/{event.tool}
                        </span>
                      ) : null}
                    </span>
                  ) : (
                    <span className={event.is_error ? "text-red-300" : "text-emerald-300"}>
                      {simpleMode
                        ? event.is_error
                          ? "Something failed"
                          : "Done"
                        : `← ${event.is_error ? "error" : "ok"} ${event.server}/${event.tool}`}
                    </span>
                  )}
                </div>
              ))}
            </div>
          )}

          {verifyEvent && (
            <div
              className={`mt-3 rounded-lg border px-3 py-2 text-[11px] ${
                verifyEvent.passed
                  ? "border-emerald-400/20 bg-emerald-400/8 text-emerald-200"
                  : "border-red-400/20 bg-red-400/8 text-red-200"
              }`}
            >
              Verify-on-complete {verifyEvent.gate}
              {simpleMode ? ` (${verifyGateLabel(verifyEvent.gate)})` : ""}:{" "}
              {verifyEvent.summary}
            </div>
          )}

          {failed && (
            <div className="mt-3 rounded-lg border border-red-400/20 bg-red-400/8 px-3 py-2 text-[11px] text-red-200">
              {failed.error}
            </div>
          )}

          {completed && !simpleMode && (
            <div className="mt-3 flex flex-wrap gap-3 text-[10px] text-slate-500">
              <span>{completed.result.provider}</span>
              <span>{completed.result.model}</span>
              <span>
                {completed.result.usage.input_tokens} in /{" "}
                {completed.result.usage.output_tokens} out
              </span>
              <span>${(completed.result.cost_micros / 1_000_000).toFixed(6)}</span>
            </div>
          )}
        </Panel>

        {!simpleMode && (
          <div className="space-y-3">
            {showDebugHarness && (
              <Disclosure
                title="Budgets"
                subtitle="Step, token, and dollar caps"
                hint="Reserved before each provider round."
                summary={`${maxSteps} steps · $${sessionCap}/$${dailyCap}`}
                defaultOpen={false}
                storageKey="ade_harness_open"
              >
                <div className="space-y-3">
                  <div className="grid grid-cols-2 gap-2">
                    <Field label="Max steps" value={maxSteps} onChange={setMaxSteps} />
                    <Field
                      label="Max tokens"
                      value={maxTokens}
                      onChange={setMaxTokens}
                      placeholder="unlimited"
                    />
                  </div>
                  <div className="grid grid-cols-2 gap-2">
                    <Field label="Session cap $" value={sessionCap} onChange={setSessionCap} />
                    <Field label="Daily cap $" value={dailyCap} onChange={setDailyCap} />
                  </div>
                </div>
              </Disclosure>
            )}

            <Disclosure
              title="Model"
              subtitle="Key from local vault"
              hint="Presets set provider + base URL. Expand for exact ids."
              summary={`${provider} · ${model || DEFAULT_MODEL}`}
              defaultOpen={false}
              storageKey="ade_agent_model_open"
            >
              <div className="space-y-3">
                <ChipRow label="Provider">
                  {PROVIDER_PRESETS.map((preset) => (
                    <Chip
                      key={preset.id}
                      active={provider === preset.id}
                      onClick={() => {
                        setProvider(preset.id);
                        setBaseUrl(preset.baseUrl);
                        if (
                          !model.trim() ||
                          !PROVIDER_PRESETS.some((p) => p.models.includes(model))
                        ) {
                          setModel(preset.models[0] ?? DEFAULT_MODEL);
                        }
                      }}
                    >
                      {preset.id}
                    </Chip>
                  ))}
                </ChipRow>
                <ChipRow label="Model">
                  {(
                    PROVIDER_PRESETS.find((preset) => preset.id === provider)?.models ?? [
                      DEFAULT_MODEL,
                    ]
                  ).map((id) => (
                    <Chip key={id} active={model === id} onClick={() => setModel(id)}>
                      {id}
                    </Chip>
                  ))}
                </ChipRow>
                <Field label="Provider id" value={provider} onChange={setProvider} />
                <Disclosure
                  title="Base URL and pricing"
                  subtitle="Usually leave defaults"
                  defaultOpen={false}
                  storageKey="ade_agent_pricing_open"
                >
                  <div className="space-y-3">
                    <Field label="Base URL" value={baseUrl} onChange={setBaseUrl} mono />
                    <div className="grid grid-cols-2 gap-2">
                      <Field label="Input $/MTok" value={inputCost} onChange={setInputCost} />
                      <Field label="Output $/MTok" value={outputCost} onChange={setOutputCost} />
                    </div>
                  </div>
                </Disclosure>
                <Field label="Exact model id" value={model} onChange={setModel} mono />
              </div>
            </Disclosure>

            {showDebugHarness && (
              <Disclosure
                title="Leases"
                subtitle="Path ownership"
                hint="Empty means PLAN paths only."
                summary={leases.length ? String(leases.length) : "none"}
                defaultOpen={false}
                storageKey="ade_leases_open"
              >
                {leases.length === 0 ? (
                  <p className="text-[11px] leading-5 text-slate-500">
                    No active leases. Writes need PLAN owned paths or a bound lease.
                  </p>
                ) : (
                  <div className="space-y-2">
                    {leases.map((lease) => (
                      <div
                        key={lease.id}
                        className="rounded-lg border border-white/7 bg-white/2 px-3 py-2 text-[11px]"
                      >
                        <div className="font-mono text-slate-200">{lease.path}</div>
                        <div className="mt-1 text-[10px] text-slate-500">
                          {lease.mode} · agent {lease.agent_id.slice(0, 8)}
                          {lease.protected ? " · protected" : ""}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </Disclosure>
            )}
          </div>
        )}
      </div>

      {devMode && (
        <Disclosure
          title="Turn trace"
          subtitle="Timeline · ToolEffect · spend"
          hint="Debug event stream."
          defaultOpen={false}
          className="mt-4"
          storageKey="ade_turn_trace_open"
        >
          {events.length === 0 ? (
            <p className="text-[11px] text-slate-500">Run a turn to populate the trace.</p>
          ) : (
            <div className="thin-scrollbar max-h-80 space-y-1.5 overflow-y-auto">
              {events.map((event, index) => (
                <div
                  key={`trace-${index}`}
                  className="rounded-md border border-white/6 bg-black/25 px-3 py-1.5 font-mono text-[10px] text-slate-400"
                >
                  <span className="text-slate-500">{String(index + 1).padStart(2, "0")}</span>{" "}
                  <span className="text-amber-200/90">{event.type}</span>
                  {event.type === "started" && (
                    <span>
                      {" "}
                      {event.provider}/{event.model}
                    </span>
                  )}
                  {event.type === "tool_call" && (
                    <span>
                      {" "}
                      {event.server}/{event.tool}
                      {event.effect ? ` [${event.effect}]` : ""}
                    </span>
                  )}
                  {event.type === "tool_result" && (
                    <span>
                      {" "}
                      {event.is_error ? "error" : "ok"} {event.server}/{event.tool}
                    </span>
                  )}
                  {event.type === "usage" && (
                    <span>
                      {" "}
                      in={event.input_tokens} out={event.output_tokens} $
                      {(event.cost_micros / 1_000_000).toFixed(6)}
                    </span>
                  )}
                  {event.type === "spend_warning" && (
                    <span>
                      {" "}
                      {event.scope} projected=
                      {(event.projected_micros / 1_000_000).toFixed(4)}
                    </span>
                  )}
                  {event.type === "verify_complete" && (
                    <span>
                      {" "}
                      {event.gate} {event.passed ? "pass" : "fail"} · {event.summary}
                    </span>
                  )}
                  {event.type === "completed" && (
                    <span>
                      {" "}
                      tools={event.result.tool_calls} $
                      {(event.result.cost_micros / 1_000_000).toFixed(6)}
                    </span>
                  )}
                  {event.type === "failed" && <span> {event.error}</span>}
                  {event.type === "cancelled" && <span> {event.reason}</span>}
                  {event.type === "text_delta" && (
                    <span>
                      {" "}
                      {event.text.slice(0, 80)}
                      {event.text.length > 80 ? "…" : ""}
                    </span>
                  )}
                </div>
              ))}
            </div>
          )}
          {(usageEvents.length > 0 || spendWarnings.length > 0) && (
            <div className="mt-3 flex flex-wrap gap-3 text-[10px] text-slate-500">
              <span>{usageEvents.length} usage pulses</span>
              <span>{spendWarnings.length} spend warnings</span>
              <span>{activity.length} tool events</span>
            </div>
          )}
        </Disclosure>
      )}
    </div>
  );
}

function KeysView({
  simpleMode = false,
  onContinueToAgent,
}: {
  simpleMode?: boolean;
  onContinueToAgent?: () => void;
}) {
  const [provider, setProvider] = useState(() => {
    if (typeof window === "undefined") return "openai";
    return window.localStorage.getItem(AGENT_PROVIDER_KEY) || "openai";
  });
  const [profile, setProfile] = useState("local");
  const [secret, setSecret] = useState("");
  const [baseUrl, setBaseUrl] = useState(() => {
    if (typeof window === "undefined") return "https://api.openai.com/v1";
    return window.localStorage.getItem(AGENT_BASE_URL_KEY) || "https://api.openai.com/v1";
  });
  const [model, setModel] = useState(() => {
    if (typeof window === "undefined") return DEFAULT_MODEL;
    return window.localStorage.getItem(AGENT_MODEL_KEY) || DEFAULT_MODEL;
  });
  const [inputCostPerMtok, setInputCostPerMtok] = useState("");
  const [outputCostPerMtok, setOutputCostPerMtok] = useState("");
  const [maxCostUsd, setMaxCostUsd] = useState("0.05");
  const [approveLiveCost, setApproveLiveCost] = useState(false);
  const [status, setStatus] = useState<ProviderKeyStatus | null>(null);
  const [smoke, setSmoke] = useState<ProviderKeySmokeResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [showLiveValidation, setShowLiveValidation] = useState(false);

  const refreshStatus = useCallback(async () => {
    if (!provider.trim() || !profile.trim()) return;
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeyStatus>("key_status", {
        provider: provider.trim(),
        profile: profile.trim(),
      });
      setStatus(result);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }, [profile, provider]);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    window.localStorage.setItem(AGENT_PROVIDER_KEY, provider.trim() || "openai");
  }, [provider]);
  useEffect(() => {
    if (baseUrl.trim()) {
      window.localStorage.setItem(AGENT_BASE_URL_KEY, baseUrl.trim());
    }
  }, [baseUrl]);
  useEffect(() => {
    if (model.trim()) {
      window.localStorage.setItem(AGENT_MODEL_KEY, model.trim());
    }
  }, [model]);

  const save = async (andContinue: boolean) => {
    if (!secret.trim()) return;
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeyStatus>("key_set", {
        provider: provider.trim(),
        profile: profile.trim(),
        secret,
      });
      setSecret("");
      setStatus(result);
      setSmoke(null);
      window.localStorage.setItem(AGENT_PROVIDER_KEY, result.provider);
      setMessage(`${result.provider} credential saved to the OS vault.`);
      if (andContinue) {
        onContinueToAgent?.();
      }
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (
      !window.confirm(
        `Delete the ${provider.trim()} credential from profile ${profile.trim()}?`,
      )
    ) {
      return;
    }
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeyDeleteResult>("key_delete", {
        provider: provider.trim(),
        profile: profile.trim(),
      });
      setStatus({
        profile: result.profile,
        provider: result.provider,
        configured: false,
      });
      setSmoke(null);
      setMessage(result.deleted ? "Credential deleted." : "No credential was configured.");
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const runSmoke = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeySmokeResult>("key_smoke", {
        provider: provider.trim(),
        profile: profile.trim(),
      });
      setSmoke(result);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const runLiveSmoke = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<ProviderKeySmokeResult>("key_live_smoke", {
        provider: provider.trim(),
        profile: profile.trim(),
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        inputCostPerMtok: Number(inputCostPerMtok),
        outputCostPerMtok: Number(outputCostPerMtok),
        maxCostUsd: Number(maxCostUsd),
        approveCost: approveLiveCost,
      });
      setSmoke(result);
      setApproveLiveCost(false);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid grid-cols-1 gap-5 lg:grid-cols-[1fr_340px]">
      <Panel
        title="Provider credentials"
        subtitle="Stored only in the native OS credential vault"
      >
        <ChipRow
          label="Common providers"
          hint="Fills provider id and base URL for Agent + live smoke."
        >
          {PROVIDER_PRESETS.map((preset) => (
            <Chip
              key={preset.id}
              active={provider === preset.id}
              onClick={() => {
                setProvider(preset.id);
                setBaseUrl(preset.baseUrl);
                setModel(preset.models[0] ?? DEFAULT_MODEL);
                setStatus(null);
                setSmoke(null);
              }}
            >
              {preset.id}
            </Chip>
          ))}
        </ChipRow>
        <div className="mt-3 grid max-w-2xl grid-cols-1 gap-3 sm:grid-cols-2">
          <label className="block">
            <span className="mb-1.5 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-wider text-slate-600">
              Provider
              <Hint text="Must match the vault entry Agent will load." />
            </span>
            <input
              value={provider}
              onChange={(event) => {
                setProvider(event.target.value);
                setStatus(null);
                setSmoke(null);
              }}
              list="provider-key-options"
              autoComplete="off"
              className="w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 text-xs text-slate-200"
            />
            <datalist id="provider-key-options">
              <option value="openai" />
              <option value="anthropic" />
              <option value="azure-openai" />
              <option value="openrouter" />
            </datalist>
          </label>
          <Field
            label="Profile"
            value={profile}
            onChange={(value) => {
              setProfile(value);
              setStatus(null);
              setSmoke(null);
            }}
          />
        </div>

        <label className="mt-4 block max-w-2xl">
          <span className="mb-1.5 block text-[10px] font-semibold uppercase tracking-wider text-slate-600">
            API key
          </span>
          <input
            type="password"
            value={secret}
            onChange={(event) => setSecret(event.target.value)}
            autoComplete="new-password"
            spellCheck={false}
            placeholder={status?.configured ? "Enter a replacement key" : "Enter provider key"}
            className="w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs text-slate-200"
          />
        </label>

        <div className="mt-5 max-w-2xl rounded-xl border border-white/10 p-4">
          <button
            type="button"
            onClick={() => setShowLiveValidation((open) => !open)}
            className="flex w-full items-center justify-between text-left"
          >
            <div>
              <div className="text-xs font-semibold text-slate-300">Live credential validation</div>
              <p className="mt-1 text-[10px] leading-5 text-slate-500">
                Optional one-call smoke with a spend cap. Safe preflight below is free.
              </p>
            </div>
            <span className="text-[10px] font-semibold text-blue-200/80">
              {showLiveValidation ? "Hide" : "Show"}
            </span>
          </button>
          {showLiveValidation && (
            <>
          <p className="mt-3 text-[10px] leading-5 text-slate-500">
            Sends one 16-token-max agent turn. Current pricing is required so ADE can reject the
            request before network access if its worst-case estimate exceeds your cap.
          </p>
          <div className="mt-3 grid grid-cols-2 gap-3">
            <Field label="Exact model id" value={model} onChange={setModel} />
            <Field label="API base URL" value={baseUrl} onChange={setBaseUrl} />
            <Field
              label="Input USD / MTok"
              value={inputCostPerMtok}
              onChange={setInputCostPerMtok}
            />
            <Field
              label="Output USD / MTok"
              value={outputCostPerMtok}
              onChange={setOutputCostPerMtok}
            />
            <Field label="Maximum cost (USD)" value={maxCostUsd} onChange={setMaxCostUsd} />
          </div>
          <label className="mt-3 flex items-start gap-2 text-[10px] leading-5 text-slate-400">
            <input
              type="checkbox"
              checked={approveLiveCost}
              onChange={(event) => setApproveLiveCost(event.target.checked)}
              className="mt-1"
            />
            I approve one potentially billable provider request, bounded by the maximum above.
          </label>
          <button
            onClick={() => void runLiveSmoke()}
            disabled={
              busy ||
              !status?.configured ||
              !model.trim() ||
              !baseUrl.trim() ||
              !inputCostPerMtok.trim() ||
              !outputCostPerMtok.trim() ||
              !maxCostUsd.trim() ||
              !approveLiveCost
            }
            className="mt-3 rounded-lg border border-blue-400/30 px-4 py-2 text-xs font-semibold text-blue-300 hover:bg-blue-400/5 disabled:opacity-40"
          >
            Run capped live smoke
          </button>
            </>
          )}
        </div>

        <div className="mt-4 flex flex-wrap gap-2">
          <button
            onClick={() => void save(simpleMode)}
            disabled={busy || !provider.trim() || !profile.trim() || !secret.trim()}
            className="rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
          >
            {simpleMode
              ? status?.configured
                ? "Replace key & go to Agent"
                : "Save key & go to Agent"
              : status?.configured
                ? "Replace key"
                : "Save key"}
          </button>
          {status?.configured && (
            <button
              onClick={() => onContinueToAgent?.()}
              disabled={busy}
              className="rounded-lg border border-blue-400/30 px-4 py-2 text-xs font-semibold text-blue-200 hover:bg-blue-400/5 disabled:opacity-50"
            >
              Continue to Agent
            </button>
          )}
          {!simpleMode && (
            <button
              onClick={() => void refreshStatus()}
              disabled={busy || !provider.trim() || !profile.trim()}
              className="rounded-lg border border-white/10 px-4 py-2 text-xs text-slate-300 hover:bg-white/5 disabled:opacity-50"
            >
              Check status
            </button>
          )}
          <button
            onClick={() => void runSmoke()}
            disabled={busy || !provider.trim() || !profile.trim()}
            className="rounded-lg border border-white/10 px-4 py-2 text-xs text-slate-300 hover:bg-white/5 disabled:opacity-50"
          >
            Safe smoke preflight
          </button>
          <button
            onClick={() => void remove()}
            disabled={busy || !status?.configured}
            className="rounded-lg border border-red-400/20 px-4 py-2 text-xs text-red-300 hover:bg-red-400/5 disabled:opacity-40"
          >
            Delete key
          </button>
        </div>

        {message && (
          <div className="mt-4 rounded-lg border border-white/7 bg-white/2 p-3 text-xs text-slate-400">
            {message}
          </div>
        )}
      </Panel>

      <div className="space-y-5">
        <Panel title="Vault status" subtitle={`${profile || "—"} / ${provider || "—"}`}>
          <div
            className={`rounded-xl border p-4 ${
              status?.configured
                ? "border-emerald-400/20 bg-emerald-400/5"
                : "border-amber-400/20 bg-amber-400/5"
            }`}
          >
            <div
              className={`text-sm font-semibold ${
                status?.configured ? "text-emerald-300" : "text-amber-300"
              }`}
            >
              {busy
                ? "Checking…"
                : status?.configured
                  ? "Configured"
                  : status
                    ? "Not configured"
                    : "Status pending"}
            </div>
            <p className="mt-2 text-[10px] leading-5 text-slate-500">
              ADE reports presence only. The credential value is never returned to the frontend.
            </p>
          </div>
        </Panel>

        {smoke && (
          <Panel title="Smoke result" subtitle={smoke.status.toUpperCase()}>
            <p className="text-xs leading-5 text-slate-400">{smoke.detail}</p>
            {smoke.status === "ready" || smoke.status === "skipped" ? (
              <p className="mt-3 text-[10px] leading-5 text-slate-600">
                The safe preflight makes no network request and incurs no provider cost.
              </p>
            ) : null}
          </Panel>
        )}
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  mono = false,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  mono?: boolean;
  placeholder?: string;
}) {
  return (
    <label className="block text-[11px] text-slate-500">
      {label}
      <input
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        className={`mt-1.5 w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 text-xs text-slate-200 ${
          mono ? "font-mono" : ""
        }`}
      />
    </label>
  );
}

function AuditView({ audit }: { audit: AuditReport }) {
  return (
    <Panel title="AUDIT report" subtitle={`${audit.score}/${audit.score_max} total points`}>
      {audit.blockers.length > 0 && (
        <div className="mb-5 rounded-lg border border-red-400/20 bg-red-400/5 p-4">
          <div className="text-xs font-semibold text-red-300">Blocking findings</div>
          {audit.blockers.map((blocker) => (
            <div key={blocker} className="mt-2 text-xs text-red-200/70">
              • {blocker}
            </div>
          ))}
        </div>
      )}
      <div className="space-y-2">
        {audit.findings.map((finding) => (
          <div
            key={finding.layer}
            className="grid grid-cols-[170px_1fr_70px] items-center gap-4 rounded-lg border border-white/6 bg-white/2 px-4 py-3"
          >
            <div className="text-xs font-medium">{finding.layer}</div>
            <div className="text-xs text-slate-500">{finding.detail}</div>
            <div className="text-right text-xs text-slate-400">
              {finding.points}/{finding.points_max}
            </div>
          </div>
        ))}
      </div>
    </Panel>
  );
}

function VerifyView({
  results,
  onRun,
  simpleMode = false,
}: {
  results: VerifyResult[];
  onRun: () => void;
  simpleMode?: boolean;
}) {
  const [openDetails, setOpenDetails] = useState<Record<string, boolean>>({});

  return (
    <Panel
      title={simpleMode ? "Check my work" : "Verification evidence"}
      subtitle={
        simpleMode
          ? "Pass/fail checks that prove the workspace is still healthy"
          : "Commands, status, and captured output"
      }
    >
      {results.length === 0 ? (
        <div className="py-20 text-center">
          <div className="text-sm text-slate-400">
            {simpleMode ? "No checks run yet" : "No verification evidence yet"}
          </div>
          <button
            onClick={onRun}
            className="mt-4 rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold"
          >
            Check work
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          {results.map((result) => {
            const detailsOpen = openDetails[result.gate] ?? false;
            return (
              <div key={result.gate} className="rounded-xl border border-white/7 bg-white/2 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <span className="text-sm font-semibold">
                      {simpleMode ? verifyGateLabel(result.gate) : result.gate}
                    </span>
                    {!simpleMode && (
                      <span className="ml-3 font-mono text-[11px] text-slate-500">
                        {result.command}
                      </span>
                    )}
                    {simpleMode && (
                      <span className="ml-2 text-[10px] uppercase tracking-wide text-slate-600">
                        {result.gate}
                      </span>
                    )}
                  </div>
                  <span
                    className={
                      result.passed
                        ? "text-xs text-emerald-300"
                        : result.status === "unavailable"
                          ? "text-xs text-amber-300"
                          : "text-xs text-red-300"
                    }
                  >
                    {result.passed
                      ? simpleMode
                        ? "Pass"
                        : "● PASS"
                      : result.status === "unavailable"
                        ? simpleMode
                          ? "Skipped"
                          : "● UNAVAILABLE"
                        : simpleMode
                          ? "Needs attention"
                          : "● FAIL"}
                  </span>
                </div>
                {(result.stderr || result.stdout) && (
                  <>
                    {simpleMode ? (
                      <button
                        type="button"
                        className="mt-2 text-[10px] font-semibold text-blue-200/90 hover:text-blue-100"
                        onClick={() =>
                          setOpenDetails((current) => ({
                            ...current,
                            [result.gate]: !detailsOpen,
                          }))
                        }
                      >
                        {detailsOpen ? "Hide details" : "Show details"}
                      </button>
                    ) : null}
                    {(!simpleMode || detailsOpen) && (
                      <pre className="thin-scrollbar mt-3 max-h-44 overflow-auto whitespace-pre-wrap rounded-lg bg-black/25 p-3 text-[10px] leading-5 text-slate-500">
                        {result.stderr || result.stdout}
                      </pre>
                    )}
                  </>
                )}
              </div>
            );
          })}
          <button
            onClick={onRun}
            className="rounded-lg border border-white/10 px-3 py-2 text-xs font-semibold text-slate-300 hover:bg-white/5"
          >
            Check work again
          </button>
        </div>
      )}
    </Panel>
  );
}

function McpView({
  servers,
  tools,
  busy,
  workspaceRoot,
  onConnect,
  onDisconnect,
  onRefresh,
  onCallTool,
}: {
  servers: string[];
  tools: McpToolInfo[];
  busy: boolean;
  workspaceRoot: string;
  onConnect: (input: {
    name: string;
    command: string;
    args: string[];
    approved: boolean;
  }) => void;
  onDisconnect: (name: string) => void;
  onRefresh: () => void;
  onCallTool: (input: {
    server: string;
    tool: string;
    arguments: unknown;
  }) => Promise<McpToolCallResult | null>;
}) {
  const [name, setName] = useState("filesystem");
  const [command, setCommand] = useState("npx.cmd");
  const [argsText, setArgsText] = useState(
    `-y\n@modelcontextprotocol/server-filesystem\n${workspaceRoot}`,
  );
  const [approved, setApproved] = useState(false);
  const [selectedTool, setSelectedTool] = useState<McpToolInfo | null>(null);
  const [argsJson, setArgsJson] = useState("{}");
  const [callResult, setCallResult] = useState<McpToolCallResult | null>(null);

  const submit = () => {
    const args = argsText
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
    onConnect({ name: name.trim(), command: command.trim(), args, approved });
  };

  const selectTool = (tool: McpToolInfo) => {
    setSelectedTool(tool);
    setCallResult(null);
    setArgsJson(JSON.stringify(prefillArgs(tool, workspaceRoot), null, 2));
  };

  const runSelectedTool = async () => {
    if (!selectedTool) return;
    let argumentsPayload: unknown;
    try {
      argumentsPayload = JSON.parse(argsJson || "{}");
    } catch {
      setCallResult({
        server: selectedTool.server,
        tool: selectedTool.name,
        is_error: true,
        text: "Arguments must be valid JSON.",
        content: null,
      });
      return;
    }
    const result = await onCallTool({
      server: selectedTool.server,
      tool: selectedTool.name,
      arguments: argumentsPayload,
    });
    if (result) {
      setCallResult(result);
    }
  };

  return (
    <div className="space-y-5">
      <Panel
        title="Connect MCP server"
        subtitle="Spawns a reviewed command over stdio — approval required before launch"
      >
        <div className="grid grid-cols-2 gap-4">
          <label className="block text-xs text-slate-400">
            Server name
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              className="mt-1.5 w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 text-sm text-slate-200"
              placeholder="filesystem"
            />
          </label>
          <label className="block text-xs text-slate-400">
            Command
            <input
              value={command}
              onChange={(event) => setCommand(event.target.value)}
              className="mt-1.5 w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-sm text-slate-200"
              placeholder="npx.cmd"
            />
          </label>
        </div>
        <label className="mt-4 block text-xs text-slate-400">
          Arguments (one per line)
          <textarea
            value={argsText}
            onChange={(event) => setArgsText(event.target.value)}
            rows={4}
            className="thin-scrollbar mt-1.5 w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs leading-5 text-slate-200"
          />
        </label>
        <div className="mt-4 flex flex-wrap items-center gap-4">
          <label className="flex items-center gap-2 text-xs text-slate-400">
            <input
              type="checkbox"
              checked={approved}
              onChange={(event) => setApproved(event.target.checked)}
              className="size-3.5 accent-blue-500"
            />
            I reviewed this exact command and argument list
          </label>
          <button
            onClick={submit}
            disabled={busy || !name.trim() || !command.trim() || !approved}
            className="rounded-lg bg-violet-500 px-4 py-2 text-xs font-semibold hover:bg-violet-400 disabled:opacity-50"
          >
            {busy ? "Working…" : "Connect server"}
          </button>
          <button
            onClick={onRefresh}
            disabled={busy}
            className="rounded-lg border border-white/10 px-3 py-2 text-xs text-slate-400 hover:text-white disabled:opacity-50"
          >
            Refresh tools
          </button>
        </div>
      </Panel>

      <section className="grid grid-cols-[0.9fr_1.4fr] gap-5">
        <Panel title="Connected servers" subtitle={`${servers.length} active`}>
          {servers.length === 0 ? (
            <div className="py-12 text-center text-xs text-slate-500">
              {isTauri()
                ? "No MCP servers connected yet."
                : "Browser preview cannot host MCP connections — open ADE Desktop to connect servers."}
            </div>
          ) : (
            <div className="space-y-2">
              {servers.map((server) => (
                <div
                  key={server}
                  className="flex items-center justify-between rounded-lg border border-white/7 bg-white/2 px-3 py-2.5"
                >
                  <div className="flex items-center gap-2 text-xs">
                    <span className="size-1.5 rounded-full bg-emerald-400" />
                    {server}
                  </div>
                  <button
                    onClick={() => onDisconnect(server)}
                    disabled={busy}
                    className="text-[11px] text-red-300/80 hover:text-red-200 disabled:opacity-50"
                  >
                    Disconnect
                  </button>
                </div>
              ))}
            </div>
          )}
        </Panel>

        <Panel title="Exposed tools" subtitle={`${tools.length} tool(s) — click to call`}>
          {tools.length === 0 ? (
            <div className="py-12 text-center text-xs text-slate-500">
              Connect an approved server to inspect its tools.
            </div>
          ) : (
            <div className="thin-scrollbar max-h-112 space-y-2 overflow-y-auto">
              {tools.map((tool) => {
                const selected =
                  selectedTool?.server === tool.server && selectedTool?.name === tool.name;
                return (
                  <button
                    key={`${tool.server}:${tool.name}`}
                    onClick={() => selectTool(tool)}
                    className={`w-full rounded-lg border px-3 py-3 text-left transition ${
                      selected
                        ? "border-blue-400/40 bg-blue-500/10"
                        : "border-white/7 bg-white/2 hover:border-white/15"
                    }`}
                  >
                    <div className="flex items-baseline gap-2">
                      <span className="font-mono text-xs font-medium text-blue-200">{tool.name}</span>
                      <span className="text-[10px] uppercase tracking-wider text-slate-600">
                        {tool.server}
                      </span>
                      {isWriteCapable(tool.name) && (
                        <span className="rounded bg-amber-400/10 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wider text-amber-300">
                          writes
                        </span>
                      )}
                    </div>
                    <p className="mt-1.5 text-[11px] leading-5 text-slate-500">{tool.description}</p>
                  </button>
                );
              })}
            </div>
          )}
        </Panel>
      </section>

      <Panel
        title="Call tool"
        subtitle={
          selectedTool
            ? `${selectedTool.server} / ${selectedTool.name}`
            : "Select a tool from the list above"
        }
      >
        {!selectedTool ? (
          <div className="py-10 text-center text-xs text-slate-500">
            Choose a connected tool to edit arguments and invoke it.
          </div>
        ) : (
          <div className="space-y-4">
            {isWriteCapable(selectedTool.name) && (
              <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-200">
                This tool can modify files. Review the arguments before calling.
              </div>
            )}
            {Object.keys(selectedTool.input_schema.properties ?? {}).length > 0 && (
              <div className="rounded-lg border border-white/7 bg-white/2 px-3 py-2.5">
                <div className="text-[10px] uppercase tracking-wider text-slate-600">
                  Parameters
                </div>
                <div className="mt-1.5 space-y-1">
                  {Object.entries(selectedTool.input_schema.properties ?? {}).map(
                    ([key, spec]) => (
                      <div key={key} className="flex items-baseline gap-2 text-[11px]">
                        <span className="font-mono text-blue-200">{key}</span>
                        <span className="text-slate-600">{spec.type ?? "any"}</span>
                        {(selectedTool.input_schema.required ?? []).includes(key) && (
                          <span className="text-[9px] font-semibold uppercase text-red-300/80">
                            required
                          </span>
                        )}
                        {spec.description && (
                          <span className="truncate text-slate-500">{spec.description}</span>
                        )}
                      </div>
                    ),
                  )}
                </div>
              </div>
            )}
            <label className="block text-xs text-slate-400">
              Arguments (JSON object)
              <textarea
                value={argsJson}
                onChange={(event) => setArgsJson(event.target.value)}
                rows={8}
                className="thin-scrollbar mt-1.5 w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs leading-5 text-slate-200"
              />
            </label>
            <button
              onClick={() => void runSelectedTool()}
              disabled={busy}
              className="rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
            >
              {busy ? "Calling…" : `Call ${selectedTool.name}`}
            </button>
            {callResult && (
              <div
                className={`rounded-xl border p-4 ${
                  callResult.is_error
                    ? "border-red-400/20 bg-red-400/5"
                    : "border-emerald-400/20 bg-emerald-400/5"
                }`}
              >
                <div className="flex items-center justify-between text-xs">
                  <span className="font-medium">
                    {callResult.is_error ? "Tool reported an error" : "Tool result"}
                  </span>
                  <span className="font-mono text-[10px] text-slate-500">
                    {callResult.server}/{callResult.tool}
                  </span>
                </div>
                <pre className="thin-scrollbar mt-3 max-h-72 overflow-auto whitespace-pre-wrap rounded-lg bg-black/25 p-3 text-[11px] leading-5 text-slate-300">
                  {callResult.text || JSON.stringify(callResult.content, null, 2)}
                </pre>
              </div>
            )}
          </div>
        )}
      </Panel>
    </div>
  );
}

function isWriteCapable(toolName: string): boolean {
  return /write|edit|move|create|delete|remove|rename/i.test(toolName);
}

/// Builds an argument skeleton from the tool's JSON Schema, seeding known
/// path-like fields with the workspace root so read-only calls work as-is.
function prefillArgs(tool: McpToolInfo, workspaceRoot: string): Record<string, unknown> {
  const known = knownArgsForTool(tool.name, workspaceRoot);
  const properties = tool.input_schema.properties ?? {};
  const required = tool.input_schema.required ?? [];
  const result: Record<string, unknown> = {};
  for (const [key, spec] of Object.entries(properties)) {
    if (key in known) {
      result[key] = known[key];
      continue;
    }
    if (!required.includes(key)) continue;
    if (spec.default !== undefined) {
      result[key] = spec.default;
    } else if (key === "path") {
      result[key] = workspaceRoot;
    } else {
      result[key] = emptyValueForType(spec.type);
    }
  }
  return result;
}

function knownArgsForTool(toolName: string, workspaceRoot: string): Record<string, unknown> {
  switch (toolName) {
    case "list_directory":
    case "list_directory_with_sizes":
    case "directory_tree":
      return { path: workspaceRoot };
    case "read_text_file":
    case "read_file":
    case "get_file_info":
      return { path: `${workspaceRoot}\\AGENTS.md` };
    case "search_files":
      return { path: workspaceRoot, pattern: "*.rs" };
    default:
      return {};
  }
}

function emptyValueForType(type: string | undefined): unknown {
  switch (type) {
    case "number":
    case "integer":
      return 0;
    case "boolean":
      return false;
    case "array":
      return [];
    case "object":
      return {};
    default:
      return "";
  }
}

function Panel({
  title,
  subtitle,
  dense = false,
  children,
}: {
  title: string;
  subtitle?: string;
  dense?: boolean;
  children: React.ReactNode;
}) {
  return (
    <section
      className={`rounded-2xl border border-white/7 bg-[#0d121a]/85 shadow-[0_12px_45px_rgba(0,0,0,0.15)] ${
        dense ? "p-4" : "p-5"
      }`}
    >
      <div className={subtitle ? (dense ? "mb-3" : "mb-5") : dense ? "mb-3" : "mb-4"}>
        <h2 className="text-sm font-semibold">{title}</h2>
        {subtitle ? <p className="mt-0.5 text-[11px] text-slate-600">{subtitle}</p> : null}
      </div>
      {children}
    </section>
  );
}

function MetricCard({
  label,
  value,
  accent,
  dense = false,
}: {
  label: string;
  value: string;
  accent: "blue" | "green" | "red" | "violet" | "slate";
  dense?: boolean;
}) {
  const colors = {
    blue: "text-blue-300",
    green: "text-emerald-300",
    red: "text-red-300",
    violet: "text-violet-300",
    slate: "text-slate-300",
  };
  return (
    <div
      className={`rounded-xl border border-white/7 bg-[#0d121a]/80 ${
        dense ? "px-3 py-2.5" : "px-4 py-4"
      }`}
    >
      <div className="text-[10px] uppercase tracking-[0.12em] text-slate-600">{label}</div>
      <div className={`mt-1 font-semibold ${dense ? "text-lg" : "text-xl"} ${colors[accent]}`}>
        {value}
      </div>
    </div>
  );
}

function FindingBar({ finding }: { finding: Finding }) {
  const percent = Math.round((finding.points / finding.points_max) * 100);
  return (
    <div>
      <div className="mb-1 flex justify-between text-[10px]">
        <span className="max-w-[80%] truncate text-slate-400">{finding.layer}</span>
        <span className="text-slate-600">{finding.points}/{finding.points_max}</span>
      </div>
      <div className="h-1 overflow-hidden rounded-full bg-slate-800">
        <div className="h-full rounded-full bg-blue-400/80" style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}

function StatusPill({ severity }: { severity: string }) {
  const ok = severity === "ok" || severity === "info";
  return (
    <span
      className={`shrink-0 rounded-full px-2 py-1 text-[9px] font-semibold uppercase tracking-wider ${
        ok ? "bg-emerald-400/8 text-emerald-300" : "bg-amber-400/8 text-amber-300"
      }`}
    >
      {severity}
    </span>
  );
}

function LoadingState() {
  return (
    <div className="grid h-[65vh] place-items-center">
      <div className="text-center">
        <div className="mx-auto size-6 animate-spin rounded-full border-2 border-blue-400/20 border-t-blue-400" />
        <div className="mt-3 text-xs text-slate-500">Auditing workspace…</div>
      </div>
    </div>
  );
}

export default App;
