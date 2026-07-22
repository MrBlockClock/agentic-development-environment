import { Channel } from "@tauri-apps/api/core";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from "react";
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
import { AgentActivityFeed } from "./components/AgentActivityFeed";
import {
  evaluateTurnFailure,
  failureFingerprint,
  type TurnFailureAction,
} from "./components/turnFailure";
import { BrowserApiSetup } from "./components/BrowserApiSetup";
import { BrowserView } from "./components/BrowserView";
import { EditorView, ADE_EDITOR_INTENT_KEY } from "./components/EditorView";
import { TerminalView } from "./components/TerminalView";
import { SettingsView } from "./components/SettingsView";
import { WorkspacesView } from "./components/WorkspacesView";
import { DarkSelect, GearIcon } from "./components/DarkSelect";
import { AuditViewer } from "./components/AuditViewer";
import {
  AgentSessionStrip,
  honestLeaseError,
  readOrCreateAgentId,
  rotateAgentId,
  writableConflict,
  type PathLease as StripLease,
} from "./components/AgentSessionStrip";
import {
  CapabilityMatrix,
  DesktopRequired,
} from "./components/DesktopRequired";
import { ModelPicker, ProviderSelect } from "./components/ModelPicker";
import { ComposerModelSelect } from "./components/ComposerModelSelect";
import { Chip, Disclosure } from "./components/ui";
import { DESKTOP_REQUIRED_VIEWS } from "./capabilities";
import {
  DEFAULT_BASE_URL,
  DEFAULT_MODEL,
  DEFAULT_PROVIDER,
  PROVIDER_PRESETS,
  canonicalBaseUrl,
  presetById,
} from "./providers";

const DEV_MODE_KEY = "ade_dev_mode";
const AUTONOMY_KEY = "ade_autonomy_level";
const SHELL_SCOPE_KEY = "ade_agent_shell_scope";
const APPLY_ISOLATE_KEY = "ade_apply_isolate_worktree";
const SURFACE_MODE_KEY = "ade_surface_mode";
const AGENT_PROVIDER_KEY = "ade_agent_provider";
const AGENT_BASE_URL_KEY = "ade_agent_base_url";
const AGENT_MODEL_KEY = "ade_agent_model";
const AGENT_EFFORT_KEY = "ade_agent_effort";
const NAV_OPEN_KEY = "ade_nav_open";

type AutonomyLevel = "observe" | "propose" | "act" | "automate";
/** G1: shell default cwd — workspace root vs user Desktop/home. */
type ShellScope = "workspace" | "home";
type SurfaceMode = "guided" | "power" | "dev";
type EffortLevel = "low" | "medium" | "high";

type EngGoal = {
  schema: string;
  id: string;
  createdAt: string;
  updatedAt: string;
  statement: string;
  successCriteria: string[];
  shellScope: string;
  autonomy: string;
  verifyGate?: string | null;
  ownedPaths: string[];
  status: string;
  lastHandoffId?: string | null;
};

const EFFORT_OPTIONS: {
  id: EffortLevel;
  label: string;
  /** Cap on *output* tokens across the turn; null = no output budget (High). */
  maxTokens: number | null;
  maxSteps: number;
}[] = [
  { id: "low", label: "Low · 16", maxTokens: 4_096, maxSteps: 16 },
  { id: "medium", label: "Med · 24", maxTokens: 16_384, maxSteps: 24 },
  // High: no cumulative kill-switch — finish the task (maxSteps still applies).
  { id: "high", label: "High · 32", maxTokens: null, maxSteps: 32 },
];

/** Apply/Automate/Continuity floor at Med; Suggest may stay Low. */
function effectiveEffort(
  autonomy: AutonomyLevel,
  stored: EffortLevel,
  context: "normal" | "continuity" = "normal",
): EffortLevel {
  if (context === "continuity") {
    return stored === "high" ? "high" : "medium";
  }
  if (autonomy === "propose" || autonomy === "observe") {
    return stored;
  }
  return stored === "low" ? "medium" : stored;
}

function providerSupportsEffort(providerId: string): boolean {
  return ["opencode", "openai", "anthropic", "openrouter"].includes(providerId);
}

/** Simple (`guided`) is parked — Standard is the product; Debug adds harness. */
const SURFACE_MODES: { id: SurfaceMode; label: string; hint: string }[] = [
  {
    id: "power",
    label: "Standard",
    hint: "Default work rail: Home (composer) + maps. Checks live under Configure.",
  },
  {
    id: "dev",
    label: "Debug",
    hint: "Same as Standard, with traces, leases, and harness open.",
  },
];

const PROMPT_PRESETS: {
  label: string;
  prompt: string;
  autonomy: "propose" | "act";
}[] = [
  {
    label: "Explain environment",
    prompt:
      "Summarize what this environment (attached workspace folder) is for and the safest next change.",
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
  if (!path) return "No environment attached";
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

function readShellScope(): ShellScope {
  if (typeof window === "undefined") return "workspace";
  const raw = window.localStorage.getItem(SHELL_SCOPE_KEY);
  return raw === "home" ? "home" : "workspace";
}

/** Host default for shell__run_command when model omits cwd. */
function preferredShellCwd(scope: ShellScope): string | null {
  return scope === "home" ? "~/Desktop" : null;
}

function readApplyIsolate(): boolean {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem(APPLY_ISOLATE_KEY) === "1";
}

function readSurfaceMode(): SurfaceMode {
  if (typeof window === "undefined") return "power";
  const raw = window.localStorage.getItem(SURFACE_MODE_KEY);
  // Migrate parked Simple → Standard so the product matches one rail.
  if (raw === "guided") {
    window.localStorage.setItem(SURFACE_MODE_KEY, "power");
    return "power";
  }
  if (raw === "power" || raw === "dev") return raw;
  // Legacy: old “Debug panels” flag without surface mode.
  if (window.localStorage.getItem(DEV_MODE_KEY) === "1") return "dev";
  return "power";
}

type NavItem = { id: string; label: string; icon: string; desktopOnly?: boolean };

/** 0 = daily work, 1 = context peers, 2 = rare configure / debug density */
type NavTier = 0 | 1 | 2;

type NavGroup = { title?: string; tier: NavTier; items: NavItem[] };

/**
 * Usage-ranked rail:
 * Tier 0 — one work surface (Home = composer/agent).
 * Tier 1 — environment + workspaces + context maps.
 * Tier 2 — Configure (incl. Checks = gate evidence; run from Checks view).
 */
const navGroups: NavGroup[] = [
  {
    tier: 0,
    items: [{ id: "Home", label: "Home", icon: "⌂" }],
  },
  {
    tier: 1,
    items: [
      { id: "Environment", label: "Environment", icon: "◎" },
      { id: "Workspaces", label: "Workspaces", icon: "▤", desktopOnly: true },
      { id: "Atlas", label: "Atlas", icon: "◈" },
      { id: "Plan", label: "Plan Map", icon: "◇" },
      { id: "Audit", label: "Trust", icon: "◉" },
      { id: "Browser", label: "Browser", icon: "⬚", desktopOnly: true },
      { id: "Terminal", label: "Terminal", icon: "▸", desktopOnly: true },
      { id: "Editor", label: "Editor", icon: "✎", desktopOnly: true },
    ],
  },
  {
    tier: 2,
    title: "Configure",
    items: [
      { id: "Recipes", label: "Recipes", icon: "▦" },
      { id: "Rules", label: "Guidance", icon: "☰" },
      { id: "Keys", label: "Keys", icon: "◈", desktopOnly: true },
      { id: "MCP", label: "MCP", icon: "⬡", desktopOnly: true },
      { id: "Verify", label: "Checks", icon: "✓" },
    ],
  },
];

function navItemClass(active: boolean, tier: NavTier): string {
  const weight =
    tier === 0
      ? "text-[13px] font-medium"
      : tier === 1
        ? "text-[13px]"
        : "text-[12px]";
  if (active) {
    return `flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition bg-blue-500/12 text-blue-200 ${weight}`;
  }
  if (tier === 2) {
    return `flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition text-slate-500 hover:bg-white/4 hover:text-slate-300 ${weight}`;
  }
  return `flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition text-slate-400 hover:bg-white/4 hover:text-slate-200 ${weight}`;
}

type Finding = {
  layer: string;
  severity: string;
  detail: string;
  points: number;
  points_max: number;
};

type IgnoreAlignment = {
  surface: string;
  status: "Synced" | "Drifted" | "Missing" | "NotApplicable" | string;
  missing_patterns: string[];
};

type AuditReport = {
  score: number;
  score_max: number;
  findings: Finding[];
  blockers: string[];
  ignore_alignment?: IgnoreAlignment[];
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
  has_recipe?: boolean;
  has_provider_key?: boolean;
  audit: AuditReport;
  plan: PlanReport;
  handoff: HandoffMetrics;
  leases: PathLease[];
  tasks: AgentTask[];
  rebuild_lock_warnings?: string[];
  last_verify?: VerifyResult[];
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
  | {
      type: "budget_exhausted";
      kind: string;
      limit: number;
      used: number;
      detail: string;
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
  const [navOpen, setNavOpen] = useState(() => {
    if (typeof window === "undefined") return false;
    return window.localStorage.getItem(NAV_OPEN_KEY) === "1";
  });
  const debugChrome = surfaceMode === "dev";
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [gate] = useState("G3");
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
  const [agentAutoSubmitContext, setAgentAutoSubmitContext] = useState<
    "normal" | "continuity"
  >("normal");
  const [planFocusPhaseId, setPlanFocusPhaseId] = useState<string | null>(null);
  const [atlasFocusNodeId, setAtlasFocusNodeId] = useState<string | null>(null);
  const [pendingImproveWin, setPendingImproveWin] = useState(false);
  const [guidedWins, setGuidedWins] = useState<GuidedWinsState>({
    understand: false,
    verify: false,
    improve_ade: false,
  });
  const [understandBusy, setUnderstandBusy] = useState(false);
  const [lastUnderstandPath, setLastUnderstandPath] = useState<string | null>(null);
  const [browserApiProbeKey, setBrowserApiProbeKey] = useState(0);

  const setNavOpenPersisted = (open: boolean) => {
    setNavOpen(open);
    window.localStorage.setItem(NAV_OPEN_KEY, open ? "1" : "0");
  };

  const setSurfaceModePersisted = (mode: SurfaceMode) => {
    // Simple is parked — any guided request becomes Standard.
    const next: SurfaceMode = mode === "guided" ? "power" : mode;
    setSurfaceMode(next);
    window.localStorage.setItem(SURFACE_MODE_KEY, next);
    window.localStorage.setItem(DEV_MODE_KEY, next === "dev" ? "1" : "0");
  };

  useEffect(() => {
    if (surfaceMode === "guided") {
      setSurfaceModePersisted("power");
    }
  }, [surfaceMode]);

  /** Debug-only nav: harness maps / tools stay off the Standard rail. */
  const DEBUG_NAV_IDS = useMemo(
    () => new Set(["Browser", "Terminal", "MCP"]),
    [],
  );

  const visibleNav = useMemo(
    () =>
      navGroups
        .map((group) => ({
          ...group,
          items: group.items.filter((item) => {
            if (DEBUG_NAV_IDS.has(item.id) && surfaceMode !== "dev") return false;
            return true;
          }),
        }))
        .filter((group) => group.items.length > 0),
    [DEBUG_NAV_IDS, surfaceMode],
  );

  useEffect(() => {
    if (surfaceMode !== "dev" && DEBUG_NAV_IDS.has(activeView)) {
      setActiveView("Home");
    }
  }, [DEBUG_NAV_IDS, activeView, surfaceMode]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    setBrowserApiProbeKey((key) => key + 1);
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
      if (snapshot.last_verify && snapshot.last_verify.length > 0) {
        setVerifyResults(snapshot.last_verify);
      }
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
    if (activeView === "Environment") {
      void invoke("run_plan").catch(() => {});
      void refresh();
    }
  }, [activeView, refresh]);

  useEffect(() => {
    setAgentEvents([]);
    setAgentBusy(false);
  }, [dashboard?.workspace_root]);

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

  const runVerify = async (options?: { stayOnHome?: boolean; openChecks?: boolean }) => {
    setVerifying(true);
    setError(null);
    if (options?.openChecks) {
      setActiveView("Verify");
    }
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
      if (!options?.stayOnHome && !options?.openChecks) {
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
    setActiveView("Home");
  };

  const continueLastHandoff = async (id?: string) => {
    setError(null);
    try {
      const resume = await invoke<{
        available: boolean;
        resumePrompt: string;
        goal: string;
        nextSafeCommand: string;
        turnStatus?: string | null;
        hostRanNext?: boolean;
        hostExitCode?: number | null;
      }>("handoff_resume", { id: id ?? null });
      if (!resume.available || !resume.resumePrompt.trim()) {
        setError("No handoff capsule to continue yet. Run an agent turn or Check first.");
        return;
      }
      const stored =
        (window.localStorage.getItem(AGENT_EFFORT_KEY) as EffortLevel | null) ??
        "medium";
      const nextEffort = effectiveEffort("act", stored, "continuity");
      window.localStorage.setItem(AGENT_EFFORT_KEY, nextEffort);
      window.localStorage.setItem(AUTONOMY_KEY, "act");
      setAgentAutoSubmitContext("continuity");
      setHomePrompt(resume.resumePrompt);
      setAgentAutoSubmit(true);
      setActiveView("Home");
    } catch (reason) {
      setError(String(reason));
    }
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
    leaseAgentId: string | null;
    preferredShellCwd: string | null;
    executionRoot: string | null;
  }) => {
    if (!isTauri()) {
      const message =
        "Agent turns require the ADE desktop app; the browser preview is read-only.";
      setError(message);
      setAgentEvents([{ type: "failed", error: message }]);
      return;
    }
    setAgentBusy(true);
    setAgentEvents([]);
    setError(null);

    const failTurn = (message: string) => {
      const text = honestLeaseError(message);
      setError(text);
      setAgentEvents((current) => {
        const hasTerminal = current.some(
          (event) =>
            event.type === "failed" ||
            event.type === "completed" ||
            event.type === "cancelled",
        );
        if (hasTerminal) return current;
        return [...current, { type: "failed", error: text }];
      });
    };

    const mutating = input.autonomy === "act" || input.autonomy === "automate";
    const agentId = mutating ? input.leaseAgentId : null;
    const acquiredIds: string[] = [];
    let heartbeat: ReturnType<typeof setInterval> | null = null;

    try {
      if (agentId && input.approveOwnedPaths && input.ownedPaths.length > 0) {
        const conflict = writableConflict(
          (dashboard?.leases ?? []) as StripLease[],
          agentId,
          input.ownedPaths,
        );
        if (conflict) {
          failTurn(
            `Another agent (${conflict.agent_id.slice(0, 8)}) holds a write lease on ${conflict.path}. Suggest-only until it finishes or expires.`,
          );
          return;
        }
        for (const path of input.ownedPaths) {
          try {
            const lease = await invoke<{ id: string }>("lease_acquire", {
              agentId,
              path,
              mode: "strong",
              ttlSecs: 300,
            });
            acquiredIds.push(lease.id);
          } catch (reason) {
            const message = String(reason);
            if (/already holds/i.test(message)) {
              continue;
            }
            failTurn(message);
            return;
          }
        }
        if (acquiredIds.length > 0) {
          heartbeat = setInterval(() => {
            for (const leaseId of acquiredIds) {
              void invoke("lease_renew", {
                agentId,
                leaseId,
                ttlSecs: 300,
              }).catch(() => {});
            }
          }, 90_000);
        }
      }

      const onEvent = new Channel<AgentEvent>();
      onEvent.onmessage = (event) => {
        setAgentEvents((current) => [...current, event]);
        if (event.type === "verify_complete" && isTauri()) {
          void invoke<VerifyResult[]>("run_verify", {
            gate: input.verifyGate || "G3",
            through: true,
          })
            .then(setVerifyResults)
            .catch(() => {});
        }
      };
      await invoke("run_agent_turn", {
        prompt: input.prompt,
        provider: input.provider,
        baseUrl: input.baseUrl,
        model: input.model,
        inputCostPerMtok: input.inputCostPerMtok,
        outputCostPerMtok: input.outputCostPerMtok,
        sessionCapUsd: input.sessionCapUsd,
        dailyCapUsd: input.dailyCapUsd,
        profile: "local",
        leaseAgentId: agentId,
        autonomy: input.autonomy,
        maxSteps: input.maxSteps,
        maxTokens: input.maxTokens,
        verifyOnComplete: input.verifyOnComplete,
        verifyGate: input.verifyGate,
        approveOwnedPaths: input.approveOwnedPaths,
        ownedPaths: input.ownedPaths,
        preferredShellCwd: input.preferredShellCwd,
        executionRoot: input.executionRoot,
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
      failTurn(String(reason));
      setPendingImproveWin(false);
    } finally {
      if (heartbeat) clearInterval(heartbeat);
      for (const leaseId of acquiredIds) {
        await invoke("lease_release", { leaseId }).catch(() => {});
      }
      setAgentBusy(false);
      // Rebuild PLAN + dashboard so Environment reflects post-turn audit state.
      await invoke("run_plan").catch(() => {});
      void refresh();
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
          className="fixed inset-0 z-20 bg-black/50 md:hidden"
          onClick={() => setNavOpenPersisted(false)}
        />
      )}
      <aside
        className={`fixed inset-y-0 left-0 z-30 flex w-62 shrink-0 flex-col border-r border-white/7 bg-[#0b0f16] px-3 py-4 transition-transform md:static md:translate-x-0 ${
          navOpen ? "translate-x-0" : "-translate-x-full md:translate-x-0"
        }`}
      >
        <div className="flex items-center gap-2.5 px-2 pb-3">
          <div
            className="grid size-8 place-items-center rounded-lg border border-blue-400/35 bg-linear-to-br from-blue-500/25 to-cyan-500/10 text-xs font-black tracking-tight text-blue-200"
            aria-label="ADE"
            title="ADE"
          >
            ADE
          </div>
          <div className="min-w-0 flex-1" />
          <button
            type="button"
            className="grid size-8 place-items-center rounded-lg border border-white/10 text-slate-400 md:hidden"
            aria-label="Close menu"
            onClick={() => setNavOpenPersisted(false)}
          >
            ✕
          </button>
        </div>

        <div
          className="mb-4 grid grid-cols-2 gap-1 rounded-lg border border-white/8 bg-black/20 p-1"
          title="Standard = work rail. Debug = Standard + traces."
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

        <nav className="thin-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto">
          {visibleNav.map((group, groupIndex) => (
            <div key={group.title ?? `tier-${group.tier}-${groupIndex}`}>
              {group.title && (
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
                      className={navItemClass(activeView === item.id, group.tier)}
                    >
                      <span
                        className={`grid w-4 place-items-center opacity-80 ${
                          group.tier === 2 ? "text-xs" : "text-sm"
                        }`}
                      >
                        {item.icon === "gear" ? (
                          <GearIcon className="size-3.5" />
                        ) : (
                          item.icon
                        )}
                      </span>
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
          {surfaceMode === "dev" && (
            <div className="mb-1 flex items-center gap-2 px-3 py-1.5 text-xs text-amber-200/80">
              <span className="size-1.5 rounded-full bg-amber-400" />
              Debug · harness on
            </div>
          )}
          <button
            type="button"
            onClick={() => {
              setActiveView("Settings");
              setNavOpenPersisted(false);
            }}
            className={`mb-1 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-xs transition ${
              activeView === "Settings"
                ? "bg-blue-500/12 text-blue-200"
                : "text-slate-500 hover:bg-white/4 hover:text-slate-300"
            }`}
          >
            <GearIcon className="size-3.5 shrink-0 opacity-80" />
            Settings
          </button>
          <div className="px-3 py-1.5 text-[10px] leading-4 text-slate-600">
            {isTauri() ? "Desktop" : "Browser preview"}
            {mcpServers.length > 0 ? ` · ${mcpServers.length} MCP` : ""}
          </div>
        </div>
      </aside>

      <main
        className={`min-w-0 flex-1 ${
          activeView === "Home" || activeView === "Agent"
            ? "flex flex-col overflow-hidden"
            : "thin-scrollbar overflow-y-auto"
        }`}
      >
        <header className="flex h-12 shrink-0 items-center justify-between gap-2 border-b border-white/7 bg-[#080b11] px-3 sm:h-14 sm:px-5">
          <div className="flex min-w-0 items-center gap-2">
            <button
              type="button"
              className="grid size-8 shrink-0 place-items-center rounded-lg border border-white/10 bg-white/2.5 text-slate-300 md:hidden"
              aria-label="Open menu"
              onClick={() => setNavOpenPersisted(true)}
            >
              ☰
            </button>
            <div className="min-w-0">
              <h1 className="text-sm font-semibold leading-tight">
                {activeView === "Environment"
                  ? "Environment"
                  : activeView === "Rules"
                    ? "Guidance"
                    : activeView === "Plan"
                      ? "Plan Map"
                      : activeView === "Verify"
                        ? "Checks"
                        : activeView === "Audit"
                          ? "Trust"
                          : activeView === "Agent"
                          ? "Home"
                          : activeView}
                {debugChrome && (
                  <span className="ml-2 rounded bg-amber-400/15 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-amber-200">
                    Debug
                  </span>
                )}
              </h1>
              <p
                className="mt-0.5 max-w-[36vw] truncate text-[10px] text-slate-500 sm:max-w-[52vw]"
                title={dashboard?.workspace_root ?? undefined}
              >
                {dashboard?.workspace_root
                  ? `Working in ${workspaceLeaf(dashboard.workspace_root)}`
                  : "No environment attached"}
                {dashboard?.is_dogfood ? " · dogfood" : ""}
                {!isTauri() && " · browser"}
              </p>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-1.5">
            <button
              type="button"
              onClick={() => setActiveView("Workspaces")}
              title={dashboard?.workspace_root ?? "Change workspace"}
              className="hidden max-w-40 truncate rounded-md border border-white/10 bg-white/2.5 px-2 py-1 text-[10px] font-medium text-slate-300 hover:bg-white/6 sm:inline-block"
            >
              Change…
            </button>
            <button
              onClick={() => void refresh()}
              disabled={loading}
              aria-label="Refresh dashboard"
              className="grid size-7 place-items-center rounded-md border border-white/10 bg-white/2.5 text-slate-400 hover:text-white disabled:opacity-50"
            >
              ↻
            </button>
          </div>
        </header>

        <div
          className={
            activeView === "Home" || activeView === "Agent"
              ? "mx-auto flex min-h-0 w-full max-w-2xl flex-1 flex-col px-4 py-3 sm:px-5"
              : "mx-auto max-w-350 p-4 sm:p-5"
          }
        >
          {!isTauri() && (
            <div className="mb-5 space-y-3">
              <BrowserApiSetup
                refreshKey={browserApiProbeKey}
                onResolved={() => {
                  if (!dashboard) {
                    void refresh();
                  }
                }}
              />
              <CapabilityMatrix shell="browser" />
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
              {DESKTOP_REQUIRED_VIEWS.has(activeView) && !isTauri() ? (
                <DesktopRequired
                  view={activeView}
                  simpleMode={false}
                />
              ) : (
                <>
              {(activeView === "Home" || activeView === "Agent") &&
                (isTauri() ? (
                  <AgentView
                    key={dashboard.workspace_root}
                    events={agentEvents}
                    busy={agentBusy}
                    connectedTools={mcpTools.length}
                    initialPrompt={homePrompt}
                    autoSubmit={agentAutoSubmit}
                    autoSubmitContext={agentAutoSubmitContext}
                    onAutoSubmitHandled={() => {
                      setAgentAutoSubmit(false);
                      setAgentAutoSubmitContext("normal");
                    }}
                    sharedVerifyGate={gate}
                    devMode={debugChrome}
                    simpleMode={false}
                    workspaceRoot={dashboard.workspace_root}
                    onOpenWorkspaces={() => setActiveView("Workspaces")}
                    onOpenEnvironment={() => setActiveView("Environment")}
                    leases={dashboard.leases}
                    planOwnedPaths={[
                      ...new Set([
                        ...dashboard.plan.phases.flatMap((phase) => phase.owned_paths),
                        ...(!dashboard.has_recipe ? [".ade/recipe.json"] : []),
                      ]),
                    ]}
                    rebuildLockWarnings={dashboard.rebuild_lock_warnings ?? []}
                    handoffAvailable={
                      dashboard.handoff.capsule_count > 0 ||
                      Boolean(dashboard.handoff.latest_status)
                    }
                    handoffLatestStatus={dashboard.handoff.latest_status}
                    onContinueHandoff={() => void continueLastHandoff()}
                    onClearTranscript={() => setAgentEvents([])}
                    onOpenKeys={() => setActiveView("Keys")}
                    tasks={dashboard.tasks ?? []}
                    planPhaseCount={dashboard.plan.phases.length}
                    onRefresh={() => void refresh()}
                    onRun={(input) => void runAgentTurn(input)}
                  />
                ) : (
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
                    devMode={debugChrome}
                    simpleMode={false}
                    onOpenAgent={() => setActiveView("Home")}
                    onOpenHealth={() => setActiveView("Environment")}
                    onOpenWorkspaces={() => setActiveView("Workspaces")}
                    onOpenRecipes={() => setActiveView("Recipes")}
                    onOpenKeys={() => setActiveView("Keys")}
                    onOpenVerify={() => setActiveView("Verify")}
                    onUnderstand={() => void runUnderstandProject()}
                    onVerifyHome={() => void runVerify({ stayOnHome: true })}
                    onImproveAde={startImproveAde}
                    onOpenAdeOnItself={() => void openAdeOnItself()}
                    onRunAgent={() => {
                      if (!homePrompt.trim()) return;
                      setAgentAutoSubmit(true);
                      setActiveView("Home");
                    }}
                    onApplyPreset={(preset) => {
                      window.localStorage.setItem(AUTONOMY_KEY, preset.autonomy);
                      setHomePrompt(preset.prompt);
                      setAgentAutoSubmit(true);
                      setActiveView("Home");
                    }}
                  />
                ))}
              {activeView === "Environment" && (
                <Overview
                  dashboard={dashboard}
                  scorePercent={scorePercent}
                  verifyResults={verifyResults}
                  verifying={verifying}
                  executing={executing}
                  onExecute={() => void executePlan()}
                  onFixWithAde={(prompt) => {
                    window.localStorage.setItem(AUTONOMY_KEY, "act");
                    setHomePrompt(prompt);
                    setAgentAutoSubmit(true);
                    setActiveView("Home");
                  }}
                  onRunChecks={() => void runVerify({ openChecks: true })}
                  onOpenKeys={() => setActiveView("Keys")}
                  onOpenRecipes={() => setActiveView("Recipes")}
                  onOpenVerify={() => setActiveView("Verify")}
                  onOpenHome={() => setActiveView("Home")}
                  onOpenWorkspaces={() => setActiveView("Workspaces")}
                  onContinueHandoff={() => void continueLastHandoff()}
                  onReviewHandoffInEditor={() => {
                    window.sessionStorage.setItem(
                      ADE_EDITOR_INTENT_KEY,
                      JSON.stringify({ mode: "handoff" }),
                    );
                    setActiveView("Editor");
                  }}
                  onRefresh={() => void refresh()}
                  devMode={debugChrome}
                />
              )}
              {activeView === "Workspaces" &&
                (DESKTOP_REQUIRED_VIEWS.has("Workspaces") && !isTauri() ? (
                  <DesktopRequired view="Workspaces" />
                ) : (
                  <WorkspacesView
                    onOpened={() => {
                      void refresh();
                      setActiveView("Environment");
                    }}
                    onOpenEnvironment={() => setActiveView("Environment")}
                  />
                ))}
              {activeView === "Browser" &&
                (DESKTOP_REQUIRED_VIEWS.has("Browser") && !isTauri() ? (
                  <DesktopRequired view="Browser" />
                ) : (
                  <BrowserView />
                ))}
              {activeView === "Terminal" &&
                (DESKTOP_REQUIRED_VIEWS.has("Terminal") && !isTauri() ? (
                  <DesktopRequired view="Terminal" />
                ) : (
                  <TerminalView />
                ))}
              {activeView === "Editor" &&
                (DESKTOP_REQUIRED_VIEWS.has("Editor") && !isTauri() ? (
                  <DesktopRequired view="Editor" />
                ) : (
                  <EditorView />
                ))}
              {activeView === "Keys" && (
                <KeysView
                  simpleMode={false}
                  onContinueToAgent={() => setActiveView("Home")}
                />
              )}
              {activeView === "Audit" && (
                <AuditView
                  audit={dashboard.audit}
                  handoffs={dashboard.handoff.recent}
                  onRefresh={() => void refresh()}
                  onOpenSettings={() => setActiveView("Settings")}
                />
              )}
              {activeView === "Plan" && (
                <PlanMap
                  plan={dashboard.plan}
                  scorePercent={scorePercent}
                  verifyResults={verifyResults}
                  executing={executing}
                  focusPhaseId={planFocusPhaseId}
                  onExecute={() => void executePlan()}
                  onRunAudit={() => void runAudit()}
                  onRunVerify={() => void runVerify()}
                  onOpenGuidance={() => setActiveView("Rules")}
                  onOpenAtlas={(phaseId) => {
                    setAtlasFocusNodeId(phaseId ? `phase:${phaseId}` : "hub-plan");
                    setActiveView("Atlas");
                  }}
                />
              )}
              {activeView === "Atlas" && (
                <AtlasView
                  auditFindings={dashboard.audit.findings}
                  planPhases={dashboard.plan.phases}
                  verifyGates={verifyResults.map((r) => r.gate)}
                  handoffs={dashboard.handoff.recent}
                  focusNodeId={atlasFocusNodeId}
                  onOpenGuidance={() => setActiveView("Rules")}
                  onOpenPlan={(phaseId) => {
                    setPlanFocusPhaseId(phaseId ?? null);
                    setActiveView("Plan");
                  }}
                />
              )}
              {activeView === "Verify" && (
                <VerifyView
                  results={verifyResults}
                  busy={verifying}
                  simpleMode={false}
                  onRun={() => void runVerify({ openChecks: true })}
                />
              )}
              {activeView === "Rules" && (
                <RulesEditor
                  onOpenAtlas={() => {
                    setAtlasFocusNodeId("hub-workspace");
                    setActiveView("Atlas");
                  }}
                  onOpenPlan={() => setActiveView("Plan")}
                />
              )}
              {activeView === "Settings" && (
                <SettingsView onOpenKeys={() => setActiveView("Keys")} />
              )}
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
                  simpleMode={false}
                  onPreview={previewRecipe}
                  onInitialize={(input) => void initializeRecipe(input)}
                />
              )}
                </>
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
  onOpenWorkspaces,
  onOpenRecipes,
  onOpenKeys,
  onOpenVerify,
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
  onOpenWorkspaces: () => void;
  onOpenRecipes: () => void;
  onOpenKeys: () => void;
  onOpenVerify: () => void;
  onUnderstand: () => void;
  onVerifyHome: () => void;
  onImproveAde: () => void;
  onOpenAdeOnItself: () => void;
  onRunAgent: () => void;
  onApplyPreset: (preset: (typeof PROMPT_PRESETS)[number]) => void;
}) {
  const [keyReady, setKeyReady] = useState(Boolean(dashboard.has_provider_key));
  const latestHandoff = dashboard.handoff.recent[0];
  const recipeReady = Boolean(dashboard.has_recipe);
  const verifyReady = guidedWins.verify;
  const inBrowser = !isTauri();
  // Browser can finish stack + verify; Keys/Agent stay a Desktop gate (not a fake done).
  const browserSetupComplete = recipeReady && verifyReady;
  const readinessComplete = inBrowser
    ? browserSetupComplete
    : keyReady && recipeReady && verifyReady;
  const winsDone =
    Number(guidedWins.understand) +
    Number(guidedWins.verify) +
    (dashboard.is_dogfood ? Number(guidedWins.improve_ade) : 0);
  const winsTotal = dashboard.is_dogfood ? 3 : 2;

  useEffect(() => {
    if (dashboard.has_provider_key) {
      setKeyReady(true);
      return;
    }
    if (!isTauri()) {
      setKeyReady(false);
      return;
    }
    let cancelled = false;
    const providers = [
      window.localStorage.getItem(AGENT_PROVIDER_KEY) || "openai",
      ...PROVIDER_PRESETS.map((preset) => preset.id),
    ];
    const unique = [
      ...new Set(providers.map((id) => id.trim().toLowerCase()).filter(Boolean)),
    ];
    void (async () => {
      for (const provider of unique) {
        try {
          const status = await invoke<ProviderKeyStatus>("key_status", {
            provider,
            profile: "local",
          });
          if (cancelled) return;
          if (status.configured) {
            setKeyReady(true);
            return;
          }
        } catch {
          // keep probing
        }
      }
      if (!cancelled) setKeyReady(false);
    })();
    return () => {
      cancelled = true;
    };
  }, [dashboard.has_provider_key]);

  const nextWin = !guidedWins.understand
    ? "Learn this project"
    : !guidedWins.verify
      ? "Check that things still work"
      : dashboard.is_dogfood && !guidedWins.improve_ade
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
    ...(dashboard.is_dogfood
      ? [
          {
            id: "improve" as const,
            title: "Try a small safe change",
            detail: "Open Agent with a careful, check-after change",
            done: guidedWins.improve_ade,
            busy: agentBusy,
            onClick: onImproveAde,
          },
        ]
      : []),
  ];

  const readinessSteps = [
    {
      id: "keys",
      title: inBrowser ? "Add an API key (Desktop)" : "Add an API key",
      detail: inBrowser
        ? "BYOK vault is Desktop-only — open ADE Desktop for Keys"
        : "BYOK so Agent can call your model",
      done: inBrowser ? false : keyReady,
      desktopOnly: inBrowser,
      cta: inBrowser ? "Open Desktop path" : "Add API key",
      onClick: onOpenKeys,
    },
    {
      id: "recipe",
      title: "Choose a stack",
      detail: "Trust contract via Stack Fit / Recipes",
      done: recipeReady,
      desktopOnly: false,
      cta: "Choose stack",
      onClick: onOpenRecipes,
    },
    {
      id: "verify",
      title: "Check the workspace",
      detail: "Run verify once before trusting agent work",
      done: verifyReady,
      desktopOnly: false,
      cta: "Check workspace",
      onClick: onVerifyHome,
    },
  ];
  // In browser, prefer next actionable step that works here (skip Keys gate).
  const nextReadiness = inBrowser
    ? readinessSteps.find((step) => !step.done && !step.desktopOnly) ??
      readinessSteps.find((step) => !step.done)
    : readinessSteps.find((step) => !step.done);

  const heroTitle = "ADE";
  const envName = workspaceLeaf(dashboard.workspace_root);
  const heroSubtitle = dashboard.workspace_root
    ? `Working in ${envName}. Ask ADE about this environment.`
    : "Attach a workspace folder first — then ask ADE.";

  return (
    <div className="mx-auto max-w-3xl space-y-5">
      <section className="rounded-2xl border border-white/8 bg-[#0c121c] px-5 py-5 sm:px-6 sm:py-6">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h2 className="text-2xl font-semibold tracking-tight text-slate-50 sm:text-3xl">
              {heroTitle}
            </h2>
            <p className="mt-1.5 max-w-md text-sm leading-6 text-slate-400">{heroSubtitle}</p>
          </div>
          <button
            type="button"
            onClick={onOpenWorkspaces}
            className="rounded-lg border border-white/10 bg-white/4 px-3 py-1.5 text-[11px] font-semibold text-slate-300 hover:bg-white/8"
            title={dashboard.workspace_root}
          >
            {dashboard.workspace_root ? `Change · ${envName}` : "Attach workspace"}
          </button>
        </div>
        <p className="mt-3 text-[11px] text-slate-500">
          {scorePercent}% ready
          {dashboard.is_dogfood ? " · dogfood" : ""}
          {" · "}
          <button
            type="button"
            onClick={onOpenHealth}
            className="text-slate-400 hover:text-blue-200"
          >
            Environment audit
          </button>
        </p>
        {isTauri() && <SpendUsageStrip className="mt-3" compact />}

        {simpleMode && !readinessComplete && (
          <div className="mt-5 space-y-2 rounded-xl border border-amber-400/20 bg-amber-400/5 p-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="text-[11px] font-semibold uppercase tracking-wider text-amber-100/80">
                Get ready
              </p>
              {nextReadiness && (
                <button
                  type="button"
                  onClick={nextReadiness.onClick}
                  className="rounded-lg bg-blue-500/90 px-3 py-1.5 text-[11px] font-semibold text-white hover:bg-blue-400"
                >
                  {nextReadiness.cta}
                </button>
              )}
            </div>
            <div className="space-y-1.5">
              {readinessSteps.map((step) => (
                <button
                  key={step.id}
                  type="button"
                  onClick={step.onClick}
                  className={`flex w-full items-center justify-between gap-3 rounded-lg border px-3 py-2 text-left transition ${
                    step.done
                      ? "border-emerald-400/20 bg-emerald-500/8"
                      : "border-white/8 bg-black/20 hover:border-blue-400/30"
                  }`}
                >
                  <div>
                    <div className="text-[12px] font-medium text-slate-200">{step.title}</div>
                    <div className="text-[10px] text-slate-500">{step.detail}</div>
                  </div>
                  <span className="text-[10px] uppercase tracking-wide text-slate-500">
                    {step.done ? "done" : step.desktopOnly ? "Desktop" : "next"}
                  </span>
                </button>
              ))}
            </div>
            <button
              type="button"
              onClick={onOpenVerify}
              className="text-[10px] text-slate-500 hover:text-slate-300"
            >
              Open Checks →
            </button>
          </div>
        )}

        <div className="mt-4 flex flex-col gap-3 sm:flex-row sm:items-stretch">
          <textarea
            value={prompt}
            onChange={(event) => onPromptChange(event.target.value)}
            rows={simpleMode ? 4 : 3}
            placeholder="Describe what you want help with…"
            className={`w-full flex-1 resize-y rounded-xl border border-white/10 bg-black/25 px-4 py-3 text-sm text-slate-200 outline-none ring-blue-400/30 placeholder:text-slate-600 focus:ring-2 ${
              simpleMode ? "min-h-27.5" : "min-h-22"
            }`}
          />
          <button
            type="button"
            onClick={onRunAgent}
            disabled={!prompt.trim() || agentBusy || !isTauri() || !keyReady}
            title={
              !keyReady
                ? isTauri()
                  ? "Add an API key before running Agent"
                  : "Agent needs Desktop + an API key"
                : undefined
            }
            className="shrink-0 rounded-xl bg-blue-500 px-5 py-3 text-sm font-semibold text-white hover:bg-blue-400 disabled:opacity-50 sm:min-w-30"
          >
            {agentBusy ? "…" : isTauri() ? "Go" : "Desktop"}
          </button>
        </div>
        {!keyReady && (
          <p className="mt-2 text-[11px] text-amber-200/80">
            {isTauri() ? (
              <>
                Add an API key before Go —{" "}
                <button
                  type="button"
                  onClick={onOpenKeys}
                  className="font-semibold text-amber-100 underline-offset-2 hover:underline"
                >
                  open Keys
                </button>
                .
              </>
            ) : (
              "Open ADE Desktop → Keys, then run Agent there."
            )}
          </p>
        )}

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
            {!keyReady && (
              <button
                type="button"
                onClick={onOpenKeys}
                className="rounded-lg border border-white/10 px-3 py-2 text-[11px] font-semibold text-slate-300 hover:bg-white/5"
              >
                {isTauri() ? "Add API key" : "Add API key (Desktop)"}
              </button>
            )}
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
            title="Guided path"
            summary={`${winsDone}/${winsTotal}`}
            subtitle={
              dashboard.is_dogfood
                ? "Optional — Learn, Check, then a small safe change"
                : "Optional — Learn this project, then Check"
            }
            defaultOpen={!readinessComplete || winsDone < winsTotal}
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
          <div className="mt-4 border-t border-white/6 pt-3">
            <Disclosure
              title="Environment pulse"
              subtitle="Ready score, handoffs, Recipes"
              summary={`${scorePercent}% · ${dashboard.handoff.capsule_count} handoff${dashboard.handoff.capsule_count === 1 ? "" : "s"}`}
              defaultOpen={false}
              storageKey="ade_home_workspace_pulse"
            >
              <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-[11px] text-slate-400">
                <button
                  type="button"
                  onClick={onOpenHealth}
                  className="font-medium text-slate-300 hover:text-blue-200"
                >
                  {scorePercent}% ready → Environment
                </button>
                <button
                  type="button"
                  onClick={onOpenHealth}
                  className="hover:text-slate-200"
                  title={
                    latestHandoff
                      ? `${latestHandoff.turn_status ?? "capsule"} · ${latestHandoff.id}`
                      : undefined
                  }
                >
                  {dashboard.handoff.capsule_count} handoff
                  {dashboard.handoff.capsule_count === 1 ? "" : "s"}
                </button>
                <button
                  type="button"
                  onClick={onOpenWorkspaces}
                  className="hover:text-slate-200"
                >
                  Workspaces
                </button>
                <button
                  type="button"
                  onClick={onOpenRecipes}
                  className="hover:text-slate-200"
                >
                  Recipes
                </button>
              </div>
            </Disclosure>
          </div>
        )}
      </section>

      {devMode && (
        <section className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-100/80">
          <span className="font-mono text-amber-100/70">{dashboard.workspace_root}</span>
          <span className="text-amber-100/50">
            {" "}
            · leases {dashboard.leases.length} · guided {winsDone}/{winsTotal}
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
  verifying = false,
  executing,
  onExecute,
  onFixWithAde,
  onRunChecks,
  onOpenKeys,
  onOpenRecipes,
  onOpenVerify: _onOpenVerify,
  onOpenHome,
  onOpenWorkspaces,
  onContinueHandoff,
  onReviewHandoffInEditor,
  onRefresh,
  devMode = false,
}: {
  dashboard: DashboardSnapshot;
  scorePercent: number;
  verifyResults: VerifyResult[];
  verifying?: boolean;
  executing: boolean;
  onExecute: () => void;
  onFixWithAde: (prompt: string) => void;
  onRunChecks: () => void;
  onOpenKeys: () => void;
  onOpenRecipes: () => void;
  onOpenVerify: () => void;
  onOpenHome: () => void;
  onOpenWorkspaces: () => void;
  onContinueHandoff?: () => void;
  onReviewHandoffInEditor?: () => void;
  onRefresh?: () => void;
  devMode?: boolean;
}) {
  const passed = verifyResults.filter((result) => result.passed).length;
  const failedVerify = verifyResults.filter((result) => !result.passed);
  const openTasks = dashboard.tasks.filter(
    (task) => !["completed", "failed", "cancelled"].includes(task.status),
  ).length;
  const [globalAudit, setGlobalAudit] = useState<{
    ok: boolean;
    checks: { id: string; label: string; passed: boolean; detail: string }[];
  } | null>(null);
  const [keyReady, setKeyReady] = useState(Boolean(dashboard.has_provider_key));
  const [taskGoal, setTaskGoal] = useState("");
  const [taskBusy, setTaskBusy] = useState(false);
  const [taskNote, setTaskNote] = useState<string | null>(null);
  const agentId = readOrCreateAgentId(dashboard.workspace_root);
  const planPaths = [
    ...new Set([
      ...dashboard.plan.phases.flatMap((phase) => phase.owned_paths),
      ...(!dashboard.has_recipe ? [".ade/recipe.json"] : []),
    ]),
  ];

  useEffect(() => {
    void invoke<{
      ok: boolean;
      checks: { id: string; label: string; passed: boolean; detail: string }[];
    }>("run_global_audit")
      .then(setGlobalAudit)
      .catch(() => setGlobalAudit(null));
  }, [
    dashboard.workspace_root,
    dashboard.audit.score,
    dashboard.audit.score_max,
    dashboard.has_recipe,
    dashboard.handoff.capsule_count,
  ]);

  useEffect(() => {
    if (dashboard.has_provider_key) {
      setKeyReady(true);
      return;
    }
    if (!isTauri()) {
      setKeyReady(false);
      return;
    }
    let cancelled = false;
    const providers = [
      window.localStorage.getItem(AGENT_PROVIDER_KEY) || "openai",
      ...PROVIDER_PRESETS.map((preset) => preset.id),
    ];
    const unique = [
      ...new Set(providers.map((id) => id.trim().toLowerCase()).filter(Boolean)),
    ];
    void (async () => {
      for (const provider of unique) {
        try {
          const status = await invoke<{ configured: boolean }>("key_status", {
            provider,
            profile: "local",
          });
          if (cancelled) return;
          if (status.configured) {
            setKeyReady(true);
            return;
          }
        } catch {
          // keep probing
        }
      }
      if (!cancelled) setKeyReady(false);
    })();
    return () => {
      cancelled = true;
    };
  }, [dashboard.has_provider_key]);

  type SetupGap = {
    id: string;
    title: string;
    detail: string;
    severity: "block" | "warn";
    fixLabel?: string;
    onFix?: () => void;
  };

  const gaps: SetupGap[] = [];
  if (!keyReady) {
    gaps.push({
      id: "provider-key",
      title: "No provider API key",
      detail: isTauri()
        ? "Agent turns need a key in the Desktop vault (OpenAI, OpenCode Zen, FreeLLMAPI, …)."
        : "Add a key in ADE Desktop → Keys. Browser preview cannot store vault secrets.",
      severity: "block",
      fixLabel: isTauri() ? "Open Keys" : undefined,
      onFix: isTauri() ? onOpenKeys : undefined,
    });
  }
  if (!dashboard.has_recipe) {
    gaps.push({
      id: "recipe",
      title: "No stack recipe",
      detail: "This environment has no .ade/recipe.json. Pick a recipe so ADE knows the expected stack.",
      severity: "warn",
      fixLabel: "Open Recipes",
      onFix: onOpenRecipes,
    });
  }
  if (verifyResults.length === 0) {
    gaps.push({
      id: "verify-missing",
      title: "Checks not run yet",
      detail: "Run Checks to fill the gate evidence box — this does not use the agent chat.",
      severity: "warn",
      fixLabel: verifying ? "Checking…" : "Run checks",
      onFix: verifying ? undefined : onRunChecks,
    });
  } else if (failedVerify.length > 0) {
    gaps.push({
      id: "verify-fail",
      title: `${failedVerify.length} check${failedVerify.length === 1 ? "" : "s"} failing`,
      detail: failedVerify
        .slice(0, 3)
        .map((item) => item.gate)
        .join(", "),
      severity: "block",
      fixLabel: verifying ? "Checking…" : "Re-run checks",
      onFix: verifying ? undefined : onRunChecks,
    });
  }
  for (const blocker of dashboard.audit.blockers.slice(0, 5)) {
    gaps.push({
      id: `blocker-${blocker}`,
      title: "Audit blocker",
      detail: blocker,
      severity: "block",
    });
  }
  if (globalAudit && !globalAudit.ok) {
    for (const check of globalAudit.checks.filter((item) => !item.passed && item.id !== "turso")) {
      gaps.push({
        id: `machine-${check.id}`,
        title: `Machine: ${check.label}`,
        detail: check.detail,
        severity: "warn",
      });
    }
  }
  if (dashboard.plan.phases.length > 0 && gaps.every((gap) => !gap.id.startsWith("blocker"))) {
    gaps.push({
      id: "plan-phases",
      title: `${dashboard.plan.phases.length} remediation phase${dashboard.plan.phases.length === 1 ? "" : "s"}`,
      detail: dashboard.plan.phases
        .slice(0, 2)
        .map((phase) => phase.title)
        .join(" · "),
      severity: "warn",
    });
  }

  const auditFindings = dashboard.audit.findings;
  const deferredFindings = auditFindings.filter(
    (finding) => finding.points < finding.points_max || finding.severity === "info",
  );
  const deferredPoints = deferredFindings.reduce(
    (sum, finding) => sum + Math.max(0, finding.points_max - finding.points),
    0,
  );
  const actionableReady =
    gaps.length === 0 &&
    auditFindings
      .filter((finding) => finding.severity !== "info")
      .every((finding) => finding.points >= finding.points_max);
  const ignoreAlignments = dashboard.audit.ignore_alignment ?? [];
  const ignoreIssues = ignoreAlignments.filter(
    (item) => item.status === "Drifted" || item.status === "Missing",
  );

  const setupGaps = gaps.filter(
    (gap) => gap.id !== "verify-missing" && gap.id !== "verify-fail",
  );
  const checkGaps = gaps.filter(
    (gap) => gap.id === "verify-missing" || gap.id === "verify-fail",
  );
  const onlyChecksGap = setupGaps.length === 0 && checkGaps.length > 0;

  const heroTitle =
    setupGaps.length > 0
      ? `${setupGaps.length} gap${setupGaps.length === 1 ? "" : "s"} to clear`
      : checkGaps.length > 0
        ? checkGaps[0]?.id === "verify-fail"
          ? "Checks need attention"
          : "Environment ready — run Checks for gate evidence"
        : deferredPoints > 0
          ? `Ready for focused work · ${deferredPoints} pt${deferredPoints === 1 ? "" : "s"} deferred`
          : "Nothing blocking — ready for focused work";

  return (
    <div className="space-y-4">
      <section
        className={`rounded-xl border px-4 py-3 ${
          setupGaps.length === 0
            ? "border-emerald-400/20 bg-emerald-400/5"
            : "border-amber-400/25 bg-amber-400/8"
        }`}
      >
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <div className="text-[10px] font-semibold uppercase tracking-wider text-slate-500">
              Environment audit
            </div>
            <h2 className="mt-1 text-sm font-semibold text-slate-100">{heroTitle}</h2>
            <p className="mt-1 text-xs leading-5 text-slate-500">
              Attached folder:{" "}
              <span className="font-mono text-slate-400">{dashboard.workspace_root}</span>
              {" · "}
              <button
                type="button"
                onClick={onOpenWorkspaces}
                className="text-blue-300/90 hover:text-blue-200"
              >
                Change workspace
              </button>
              {actionableReady && deferredPoints === 0
                ? " · L0–L11 actionable layers complete"
                : null}
            </p>
            {deferredFindings.length > 0 && (
              <ul className="mt-2 space-y-1 border-t border-white/6 pt-2">
                {deferredFindings.map((finding) => (
                  <li
                    key={finding.layer}
                    className="text-[11px] leading-5 text-slate-400"
                  >
                    <span className="font-medium text-slate-300">{finding.layer}</span>
                    {" · "}
                    {finding.points}/{finding.points_max}
                    {" · "}
                    <span className="text-slate-500">{finding.detail}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
          {onlyChecksGap ? (
            <button
              type="button"
              disabled={verifying}
              onClick={onRunChecks}
              className="shrink-0 rounded-lg bg-blue-500 px-3.5 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
            >
              {verifying ? "Checking…" : "Run checks"}
            </button>
          ) : setupGaps.length > 0 ? (
            <button
              type="button"
              onClick={() =>
                onFixWithAde(
                  [
                    "Diagnose and fix this ADE environment setup. Be concrete; prefer Suggest first, then Apply only for safe PLAN-owned paths.",
                    "",
                    "Gaps:",
                    ...setupGaps.map(
                      (gap) => `- [${gap.severity}] ${gap.title}: ${gap.detail}`,
                    ),
                    "",
                    `Environment root: ${dashboard.workspace_root}`,
                    `Audit score: ${dashboard.audit.score}/${dashboard.audit.score_max} (${scorePercent}%)`,
                  ].join("\n"),
                )
              }
              className="shrink-0 rounded-lg bg-blue-500 px-3.5 py-2 text-xs font-semibold hover:bg-blue-400"
            >
              Fix with ADE
            </button>
          ) : (
            <button
              type="button"
              onClick={onOpenHome}
              className="shrink-0 rounded-lg border border-white/10 bg-white/4 px-3.5 py-2 text-xs font-semibold text-slate-200 hover:bg-white/8"
            >
              Ask ADE
            </button>
          )}
        </div>

        {gaps.length > 0 && (
          <ul className="mt-3 space-y-2">
            {gaps.map((gap) => (
              <li
                key={gap.id}
                className="flex flex-wrap items-start justify-between gap-2 rounded-lg border border-white/7 bg-black/20 px-3 py-2"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span
                      className={`rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${
                        gap.severity === "block"
                          ? "bg-red-400/15 text-red-200"
                          : "bg-amber-400/15 text-amber-100"
                      }`}
                    >
                      {gap.id.startsWith("verify-")
                        ? "checks"
                        : gap.severity === "block"
                          ? "blocking"
                          : "missing"}
                    </span>
                    <span className="text-xs font-medium text-slate-200">{gap.title}</span>
                  </div>
                  <p className="mt-1 text-[11px] leading-5 text-slate-500">{gap.detail}</p>
                </div>
                {gap.onFix && gap.fixLabel && (
                  <button
                    type="button"
                    onClick={gap.onFix}
                    disabled={verifying && gap.id.startsWith("verify-")}
                    className="shrink-0 rounded-md border border-white/10 px-2 py-1 text-[10px] font-semibold text-slate-300 hover:bg-white/6 disabled:opacity-50"
                  >
                    {gap.fixLabel}
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

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
                {check.id === "turso"
                  ? check.passed
                    ? "✓"
                    : "○"
                  : check.passed
                    ? "✓"
                    : "!"}{" "}
                {check.id === "turso" && !check.passed
                  ? "Turso optional"
                  : check.label}
              </span>
            ))}
          </div>
        </div>
      )}
      {devMode && (
        <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-1.5 text-[11px] text-amber-100/80">
          Debug · score {dashboard.audit.score}/{dashboard.audit.score_max} · leases{" "}
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
            <div className="min-w-0 flex-1 space-y-2 max-h-52 overflow-y-auto pr-1">
              {auditFindings.map((finding) => (
                <FindingBar key={finding.layer} finding={finding} />
              ))}
            </div>
          </div>
          {ignoreIssues.length > 0 && (
            <div className="mt-3 border-t border-white/6 pt-2">
              <div className="text-[10px] font-semibold uppercase tracking-wider text-slate-500">
                Ignore alignment
              </div>
              <ul className="mt-1.5 space-y-1">
                {ignoreIssues.map((item) => (
                  <li key={item.surface} className="text-[11px] text-slate-400">
                    <span className="font-medium text-amber-200/90">{item.status}</span>
                    {" · "}
                    {item.surface}
                    {item.missing_patterns?.length
                      ? ` · missing ${item.missing_patterns.slice(0, 3).join(", ")}`
                      : null}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Panel>

        <Panel title="Plan" subtitle="Execution scope" dense>
          {dashboard.plan.phases.length === 0 ? (
            <div className="flex h-44 flex-col items-center justify-center text-center">
              <div className="grid size-10 place-items-center rounded-full bg-emerald-400/10 text-emerald-300">
                ✓
              </div>
              <div className="mt-3 text-sm font-medium">
                {deferredPoints > 0
                  ? `No remediation — ${deferredPoints} pt${deferredPoints === 1 ? "" : "s"} deferred`
                  : "No remediation needed"}
              </div>
              <p className="mt-1 max-w-60 text-xs leading-5 text-slate-500">
                {deferredPoints > 0
                  ? "Info-severity audit points do not create PLAN phases. Clear them only if you need full coverage."
                  : "The current audit has no actionable gaps. ADE is ready for focused work."}
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
          <div className="mb-3 space-y-2 rounded-lg border border-white/6 bg-black/20 p-3">
            <div className="text-[10px] text-slate-500">
              Agent <span className="font-mono text-slate-400">{agentId.slice(0, 8)}</span>
              {planPaths.length > 0
                ? ` · enqueue uses ${planPaths.length} PLAN path${planPaths.length === 1 ? "" : "s"}`
                : " · set PLAN owned paths first"}
            </div>
            <input
              value={taskGoal}
              onChange={(event) => setTaskGoal(event.target.value)}
              placeholder="Task goal…"
              className="w-full rounded-md border border-white/10 bg-[#101620] px-2.5 py-1.5 text-[11px] text-slate-200 outline-none focus:border-blue-400/40"
            />
            <div className="flex flex-wrap gap-1.5">
              <button
                type="button"
                disabled={taskBusy || !taskGoal.trim() || planPaths.length === 0 || !isTauri()}
                onClick={() => {
                  setTaskBusy(true);
                  setTaskNote(null);
                  void invoke("task_enqueue", {
                    goal: taskGoal.trim(),
                    ownedPaths: planPaths,
                    leaseMode: "strong",
                    dependsOn: [],
                  })
                    .then(() => {
                      setTaskGoal("");
                      setTaskNote("Queued");
                      onRefresh?.();
                    })
                    .catch((reason) => setTaskNote(honestLeaseError(String(reason))))
                    .finally(() => setTaskBusy(false));
                }}
                className="rounded-md border border-blue-400/30 bg-blue-500/15 px-2.5 py-1 text-[11px] font-semibold text-blue-100 disabled:opacity-40"
              >
                Enqueue
              </button>
              <button
                type="button"
                disabled={taskBusy || !isTauri()}
                onClick={() => {
                  setTaskBusy(true);
                  setTaskNote(null);
                  void invoke<{ id: string; goal: string } | null>("task_claim", {
                    agentId,
                    ttlSecs: 300,
                  })
                    .then((task) => {
                      if (!task) {
                        setTaskNote("No ready task");
                      } else {
                        setTaskNote(`Claimed ${task.id.slice(0, 8)} · ${task.goal}`);
                      }
                      onRefresh?.();
                    })
                    .catch((reason) => setTaskNote(honestLeaseError(String(reason))))
                    .finally(() => setTaskBusy(false));
                }}
                className="rounded-md border border-white/10 px-2.5 py-1 text-[11px] font-semibold text-slate-300 hover:bg-white/5 disabled:opacity-40"
              >
                Claim next
              </button>
            </div>
            {taskNote && <div className="text-[10px] text-slate-400">{taskNote}</div>}
          </div>
          {dashboard.tasks.length === 0 ? (
            <p className="text-xs text-slate-500">No queued tasks yet.</p>
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

      <Disclosure
        title="Continuity"
        subtitle="Handoff capsules — resume work without re-explaining context"
        summary={
          dashboard.handoff.capsule_count > 0
            ? `${dashboard.handoff.capsule_count} capsule${dashboard.handoff.capsule_count === 1 ? "" : "s"}`
            : "none"
        }
        defaultOpen={dashboard.handoff.capsule_count > 0}
        storageKey="ade_workspace_continuity"
      >
        {(dashboard.handoff.capsule_count > 0 ||
          Boolean(dashboard.handoff.latest_status)) && (
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2 rounded-lg border border-blue-400/20 bg-blue-500/8 px-3 py-2">
            <div className="min-w-0 text-[11px] text-slate-300">
              <div className="font-semibold text-blue-100">Continue last handoff</div>
              <div className="mt-0.5 text-slate-500">
                {dashboard.handoff.latest_status?.replaceAll("_", " ") ?? "capsule"}
                {dashboard.handoff.latest_created_at
                  ? ` · ${dashboard.handoff.latest_created_at}`
                  : ""}
              </div>
            </div>
            <button
              type="button"
              onClick={onContinueHandoff}
              className="shrink-0 rounded-md border border-blue-400/30 bg-blue-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/25"
            >
              Continue → Home
            </button>
            {onReviewHandoffInEditor && (
              <button
                type="button"
                onClick={onReviewHandoffInEditor}
                className="shrink-0 rounded-md border border-amber-400/30 bg-amber-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-amber-100 hover:bg-amber-500/25"
              >
                Review in Editor
              </button>
            )}
          </div>
        )}
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
      </Disclosure>

      <Disclosure
        title="Audit findings"
        subtitle="All architecture layers, ordered by contract"
        summary={`${dashboard.audit.findings.length} layers`}
        defaultOpen={
          dashboard.audit.blockers.length > 0 ||
          deferredFindings.length > 0 ||
          dashboard.audit.findings.some((finding) => finding.severity !== "ok")
        }
        storageKey="ade_workspace_audit_findings"
      >
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
      </Disclosure>
    </div>
  );
}

function AgentView({
  events,
  busy,
  connectedTools: _connectedTools,
  onRun,
  initialPrompt = "",
  autoSubmit = false,
  autoSubmitContext = "normal",
  onAutoSubmitHandled,
  sharedVerifyGate = "G3",
  devMode = false,
  simpleMode = false,
  workspaceRoot = "",
  onOpenWorkspaces: _onOpenWorkspaces,
  onOpenEnvironment: _onOpenEnvironment,
  leases = [],
  planOwnedPaths = [],
  rebuildLockWarnings = [],
  handoffAvailable = false,
  handoffLatestStatus = null,
  onContinueHandoff,
  onClearTranscript,
  onOpenKeys,
  tasks = [],
  planPhaseCount = 0,
  onRefresh,
}: {
  events: AgentEvent[];
  busy: boolean;
  connectedTools: number;
  initialPrompt?: string;
  autoSubmit?: boolean;
  autoSubmitContext?: "normal" | "continuity";
  onAutoSubmitHandled?: () => void;
  sharedVerifyGate?: string;
  devMode?: boolean;
  simpleMode?: boolean;
  workspaceRoot?: string;
  onOpenWorkspaces?: () => void;
  onOpenEnvironment?: () => void;
  leases?: PathLease[];
  planOwnedPaths?: string[];
  rebuildLockWarnings?: string[];
  handoffAvailable?: boolean;
  handoffLatestStatus?: string | null;
  onContinueHandoff?: () => void;
  onClearTranscript?: () => void;
  onOpenKeys?: () => void;
  tasks?: AgentTask[];
  planPhaseCount?: number;
  onRefresh?: () => void;
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
    leaseAgentId: string | null;
    preferredShellCwd: string | null;
    executionRoot: string | null;
  }) => void;
}) {
  const [prompt, setPrompt] = useState(initialPrompt);
  const [provider, setProvider] = useState(() => {
    if (typeof window === "undefined") return DEFAULT_PROVIDER;
    return window.localStorage.getItem(AGENT_PROVIDER_KEY) || DEFAULT_PROVIDER;
  });
  const [baseUrl, setBaseUrl] = useState(() => {
    if (typeof window === "undefined") return DEFAULT_BASE_URL;
    const storedProvider =
      window.localStorage.getItem(AGENT_PROVIDER_KEY) || DEFAULT_PROVIDER;
    const stored =
      window.localStorage.getItem(AGENT_BASE_URL_KEY) || DEFAULT_BASE_URL;
    return canonicalBaseUrl(storedProvider, stored);
  });
  const [model, setModel] = useState(() => {
    if (typeof window === "undefined") return DEFAULT_MODEL;
    return window.localStorage.getItem(AGENT_MODEL_KEY) || DEFAULT_MODEL;
  });
  const [effort, setEffort] = useState<EffortLevel>(() => {
    if (typeof window === "undefined") return "low";
    const raw = window.localStorage.getItem(AGENT_EFFORT_KEY);
    return raw === "low" || raw === "high" || raw === "medium" ? raw : "low";
  });
  // Free / BYOK gateways: leave $/MTok at 0 so spend reservation stays off unless priced.
  const [inputCost, setInputCost] = useState("0");
  const [outputCost, setOutputCost] = useState("0");
  const [sessionCap, setSessionCap] = useState(() => {
    if (typeof window === "undefined") return "1";
    return window.localStorage.getItem("ade_session_cap_usd") || "1";
  });
  const [dailyCap, setDailyCap] = useState(() => {
    if (typeof window === "undefined") return "5";
    return window.localStorage.getItem("ade_daily_cap_usd") || "5";
  });
  const [autonomy, setAutonomy] = useState<AutonomyLevel>(readAutonomy);
  const [shellScope, setShellScope] = useState<ShellScope>(readShellScope);
  const [activeGoal, setActiveGoal] = useState<EngGoal | null>(null);
  const [goalBusy, setGoalBusy] = useState(false);
  const [claimedTask, setClaimedTask] = useState<AgentTask | null>(null);
  const [taskBusy, setTaskBusy] = useState(false);
  const [taskNote, setTaskNote] = useState<string | null>(null);
  const [applyIsolate, setApplyIsolate] = useState(readApplyIsolate);
  const [activeWorktree, setActiveWorktree] = useState<string | null>(null);
  const [maxSteps, setMaxSteps] = useState("");
  const [maxTokens, setMaxTokens] = useState("");
  const [verifyOnComplete, setVerifyOnComplete] = useState(false);
  const [verifyGate, setVerifyGate] = useState(sharedVerifyGate);
  /** When set (Debug dogfood), overrides PLAN owned_paths for this turn. */
  const [forceOwnedPaths, setForceOwnedPaths] = useState<string[] | null>(null);
  const effectiveOwnedPaths =
    forceOwnedPaths && forceOwnedPaths.length > 0
      ? forceOwnedPaths
      : planOwnedPaths;
  const [turnTokens, setTurnTokens] = useState({ input: 0, output: 0 });
  const [sessionTokens, setSessionTokens] = useState({
    input: 0,
    output: 0,
    costMicros: 0,
  });
  const sessionCountedRef = useRef<string | null>(null);
  const contextLimit = 128_000;
  type TurnRecord = {
    id: string;
    createdAt: string;
    user: string;
    events: AgentEvent[];
  };
  const [pastTurns, setPastTurns] = useState<TurnRecord[]>([]);
  const [currentUser, setCurrentUser] = useState<string | null>(null);
  const [chatReady, setChatReady] = useState(false);
  const [chatError, setChatError] = useState<string | null>(null);
  const [agentId, setAgentId] = useState(() => readOrCreateAgentId(workspaceRoot));
  const feedScrollRef = useRef<HTMLDivElement>(null);
  const feedBottomRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const autoFixedFailureRef = useRef<string | null>(null);
  const [failureNote, setFailureNote] = useState<string | null>(null);

  const latestFailed = useMemo(() => {
    return [...events]
      .reverse()
      .find(
        (event): event is Extract<AgentEvent, { type: "failed" }> =>
          event.type === "failed",
      );
  }, [events]);

  const failureAdvice = useMemo(() => {
    if (!latestFailed || busy) return null;
    return evaluateTurnFailure({
      error: latestFailed.error,
      providerId: provider,
      model,
      baseUrl,
      effort,
    });
  }, [latestFailed, busy, provider, model, baseUrl, effort]);

  const runPromptAgain = useCallback(
    (
      text: string,
      overrides?: {
        provider?: string;
        baseUrl?: string;
        model?: string;
        effort?: EffortLevel;
        maxSteps?: number;
      },
    ) => {
      const trimmed = text.trim();
      if (!trimmed || busy || !isTauri()) return;
      const nextProvider = overrides?.provider ?? provider;
      const nextBase = overrides?.baseUrl ?? baseUrl;
      const nextModel = overrides?.model ?? model;
      const nextEffort = effectiveEffort(
        autonomy,
        overrides?.effort ?? effort,
        "normal",
      );
      if (!nextProvider.trim() || !nextModel.trim()) return;
      setPrompt("");
      setCurrentUser(trimmed);
      stickToBottomRef.current = true;
      const effortOpts = EFFORT_OPTIONS.find((item) => item.id === nextEffort);
      const isMutating = autonomy === "act" || autonomy === "automate";
      onRun({
        prompt: trimmed,
        provider: nextProvider.trim(),
        baseUrl: nextBase.trim(),
        model: nextModel.trim(),
        inputCostPerMtok: Number(inputCost) || 0,
        outputCostPerMtok: Number(outputCost) || 0,
        sessionCapUsd: Number(sessionCap),
        dailyCapUsd: Number(dailyCap),
        autonomy,
        maxSteps: overrides?.maxSteps
          ? Math.max(1, overrides.maxSteps)
          : maxSteps.trim()
            ? Math.max(1, Number(maxSteps) || 32)
            : (effortOpts?.maxSteps ?? 32),
        maxTokens: maxTokens.trim()
          ? Number(maxTokens) || null
          : providerSupportsEffort(nextProvider)
            ? (effortOpts?.maxTokens ?? null)
            : null,
        verifyOnComplete: autonomy === "automate" ? true : verifyOnComplete,
        verifyGate: verifyGate.trim() || "G3",
        approveOwnedPaths: isMutating,
        ownedPaths: isMutating ? effectiveOwnedPaths : [],
        leaseAgentId: isMutating ? agentId : null,
        preferredShellCwd: preferredShellCwd(shellScope),
        executionRoot: null,
      });
    },
    [
      busy,
      provider,
      baseUrl,
      model,
      effort,
      inputCost,
      outputCost,
      sessionCap,
      dailyCap,
      autonomy,
      maxSteps,
      maxTokens,
      verifyOnComplete,
      verifyGate,
      effectiveOwnedPaths,
      agentId,
      shellScope,
      onRun,
    ],
  );

  const applyFailureAction = useCallback(
    (action: TurnFailureAction, opts?: { auto?: boolean }) => {
      const promptText = currentUser?.trim();
      if (action.id === "open_keys") {
        onOpenKeys?.();
        setFailureNote("Open Keys, fix the vault key / base URL, then retry.");
        return;
      }
      if (!promptText) {
        setFailureNote("No prompt to retry — type a message and Go again.");
        return;
      }
      if (action.id === "retry") {
        setFailureNote(opts?.auto ? "Auto-retrying same setup…" : "Retrying…");
        runPromptAgain(promptText);
        return;
      }
      if (action.id === "retry_alt_model") {
        setModel(action.model);
        window.localStorage.setItem(AGENT_MODEL_KEY, action.model);
        setFailureNote(
          opts?.auto
            ? `Auto-fixed: switched model → ${action.model}`
            : `Retrying with ${action.model}…`,
        );
        runPromptAgain(promptText, { model: action.model });
        return;
      }
      if (action.id === "switch_provider") {
        setProvider(action.providerId);
        setBaseUrl(action.baseUrl);
        setModel(action.model);
        window.localStorage.setItem(AGENT_PROVIDER_KEY, action.providerId);
        window.localStorage.setItem(AGENT_BASE_URL_KEY, action.baseUrl);
        window.localStorage.setItem(AGENT_MODEL_KEY, action.model);
        setFailureNote(
          opts?.auto
            ? `Auto-fixed: switched provider → ${action.providerId}`
            : `Retrying on ${action.providerId}…`,
        );
        runPromptAgain(promptText, {
          provider: action.providerId,
          baseUrl: action.baseUrl,
          model: action.model,
        });
        return;
      }
      if (action.id === "raise_steps") {
        setEffort(action.effort);
        window.localStorage.setItem(AGENT_EFFORT_KEY, action.effort);
        setMaxSteps(String(action.maxSteps));
        setFailureNote(
          opts?.auto
            ? `Auto-fixed: Effort → ${action.effort} (${action.maxSteps} tool rounds)`
            : `Retrying with Effort ${action.effort} (${action.maxSteps} rounds)…`,
        );
        runPromptAgain(promptText, {
          effort: action.effort,
          maxSteps: action.maxSteps,
        });
        return;
      }
      if (action.id === "continue_handoff") {
        setEffort(action.effort);
        window.localStorage.setItem(AGENT_EFFORT_KEY, action.effort);
        setMaxSteps(String(action.maxSteps));
        setAutonomyPersisted("act");
        setFailureNote(
          `Continuing handoff with Effort ${action.effort} (${action.maxSteps} rounds)…`,
        );
        onContinueHandoff?.();
        return;
      }
      if (action.id === "fix_base_url") {
        setBaseUrl(action.baseUrl);
        window.localStorage.setItem(AGENT_BASE_URL_KEY, action.baseUrl);
        setFailureNote(`Updated base URL → ${action.baseUrl}`);
        runPromptAgain(promptText, { baseUrl: action.baseUrl });
      }
    },
    [currentUser, onOpenKeys, onContinueHandoff, runPromptAgain],
  );

  useEffect(() => {
    if (!latestFailed || busy || !failureAdvice?.autoFix || !currentUser) return;
    const key = failureFingerprint({
      error: latestFailed.error,
      providerId: provider,
      model,
      baseUrl,
      effort,
    });
    if (autoFixedFailureRef.current === key) return;
    autoFixedFailureRef.current = key;
    applyFailureAction(failureAdvice.autoFix, { auto: true });
  }, [
    latestFailed,
    busy,
    failureAdvice,
    currentUser,
    provider,
    model,
    baseUrl,
    effort,
    applyFailureAction,
  ]);

  useEffect(() => {
    if (!latestFailed) {
      setFailureNote(null);
    }
  }, [latestFailed]);

  useEffect(() => {
    setAgentId(readOrCreateAgentId(workspaceRoot));
  }, [workspaceRoot]);

  useEffect(() => {
    if (!isTauri() || !workspaceRoot) {
      setActiveGoal(null);
      return;
    }
    let cancelled = false;
    void invoke<EngGoal | null>("goal_active")
      .then((goal) => {
        if (!cancelled) setActiveGoal(goal);
      })
      .catch(() => {
        if (!cancelled) setActiveGoal(null);
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot]);

  const mutating = autonomy === "act" || autonomy === "automate";
  // Budgets/leases/traces stay behind Debug panels (Full) or Debug surface.
  const showDebugHarness = !simpleMode && devMode;

  useEffect(() => {
    if (!isTauri() || !workspaceRoot) {
      setPastTurns([]);
      setChatReady(true);
      return;
    }
    let cancelled = false;
    setChatReady(false);
    setChatError(null);
    void invoke<{
      id: string;
      updatedAt: string;
      turns: {
        id: string;
        createdAt: string;
        user: string;
        events: AgentEvent[];
      }[];
    }>("chat_load")
      .then((thread) => {
        if (cancelled) return;
        setPastTurns(
          (thread.turns ?? []).map((turn) => ({
            id: turn.id,
            createdAt: turn.createdAt,
            user: turn.user,
            events: turn.events ?? [],
          })),
        );
        setChatReady(true);
      })
      .catch((reason) => {
        if (cancelled) return;
        setChatError(String(reason));
        setPastTurns([]);
        setChatReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot]);

  useEffect(() => {
    if (!chatReady || !isTauri() || !workspaceRoot) return;
    const turns = pastTurns.map((turn) => ({
      id: turn.id,
      createdAt: turn.createdAt,
      user: turn.user,
      events: turn.events,
    }));
    void invoke("chat_save", { turns }).catch((reason) => {
      setChatError(String(reason));
    });
  }, [pastTurns, chatReady, workspaceRoot]);

  const clearChat = () => {
    if (busy) return;
    if (
      pastTurns.length > 0 &&
      !window.confirm("Clear this workspace chat transcript?")
    ) {
      return;
    }
    setPastTurns([]);
    setCurrentUser(null);
    onClearTranscript?.();
    if (isTauri()) {
      void invoke("chat_clear")
        .then(() => setChatError(null))
        .catch((reason) => setChatError(String(reason)));
    }
  };

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
  };

  const setShellScopePersisted = (scope: ShellScope) => {
    setShellScope(scope);
    window.localStorage.setItem(SHELL_SCOPE_KEY, scope);
  };

  const applyGoalDials = (goal: EngGoal) => {
    const nextAutonomy =
      goal.autonomy === "act" ||
      goal.autonomy === "automate" ||
      goal.autonomy === "observe" ||
      goal.autonomy === "propose"
        ? goal.autonomy
        : "propose";
    setAutonomyPersisted(nextAutonomy);
    setShellScopePersisted(goal.shellScope === "home" ? "home" : "workspace");
    if (goal.verifyGate?.trim()) {
      setVerifyGate(goal.verifyGate.trim());
    }
    if (nextAutonomy === "automate") {
      setVerifyOnComplete(true);
    }
  };

  const savePromptAsGoal = async () => {
    const statement = prompt.trim();
    if (!statement || !isTauri() || goalBusy) return;
    setGoalBusy(true);
    try {
      const goal = await invoke<EngGoal>("goal_create", {
        input: {
          statement,
          successCriteria: [],
          shellScope,
          autonomy,
          verifyGate: verifyGate.trim() || null,
          ownedPaths: mutating ? (effectiveOwnedPaths) : [],
          activate: true,
        },
      });
      setActiveGoal(goal);
    } catch (reason) {
      window.alert(String(reason));
    } finally {
      setGoalBusy(false);
    }
  };

  const runActiveGoal = () => {
    if (!activeGoal || busy || !provider.trim() || !model.trim() || !isTauri()) {
      return;
    }
    applyGoalDials(activeGoal);
    const text = [
      activeGoal.statement.trim(),
      ...(activeGoal.successCriteria?.length
        ? [
            "",
            "Success criteria:",
            ...activeGoal.successCriteria.map((c) => `- ${c}`),
          ]
        : []),
    ].join("\n");
    const nextScope: ShellScope =
      activeGoal.shellScope === "home" ? "home" : "workspace";
    const nextAutonomy: AutonomyLevel =
      activeGoal.autonomy === "act" ||
      activeGoal.autonomy === "automate" ||
      activeGoal.autonomy === "observe" ||
      activeGoal.autonomy === "propose"
        ? activeGoal.autonomy
        : "propose";
    const nextMutating = nextAutonomy === "act" || nextAutonomy === "automate";
    if (nextMutating) {
      const conflict = writableConflict(
        leases as StripLease[],
        agentId,
        effectiveOwnedPaths,
      );
      if (conflict) {
        window.alert(
          `Another agent (${conflict.agent_id.slice(0, 8)}) holds a write lease on ${conflict.path}. Switch to Suggest or wait.`,
        );
        return;
      }
    }
    setPrompt("");
    setCurrentUser(text);
    stickToBottomRef.current = true;
    const effortOpts = EFFORT_OPTIONS.find(
      (item) => item.id === effectiveEffort(nextAutonomy, effort),
    );
    onRun({
      prompt: text,
      provider: provider.trim(),
      baseUrl: baseUrl.trim(),
      model: model.trim(),
      inputCostPerMtok: Number(inputCost) || 0,
      outputCostPerMtok: Number(outputCost) || 0,
      sessionCapUsd: Number(sessionCap),
      dailyCapUsd: Number(dailyCap),
      autonomy: nextAutonomy,
      maxSteps: maxSteps.trim()
        ? Math.max(1, Number(maxSteps) || 32)
        : (effortOpts?.maxSteps ?? 32),
      maxTokens: maxTokens.trim()
        ? Number(maxTokens) || null
        : providerSupportsEffort(provider)
          ? (effortOpts?.maxTokens ?? null)
          : null,
      verifyOnComplete:
        nextAutonomy === "automate" ? true : verifyOnComplete,
      verifyGate: (activeGoal.verifyGate?.trim() || verifyGate.trim() || "G3"),
      approveOwnedPaths: nextMutating,
      ownedPaths: nextMutating ? (effectiveOwnedPaths) : [],
      leaseAgentId: nextMutating ? agentId : null,
      preferredShellCwd: preferredShellCwd(nextScope),
      executionRoot: null,
    });
  };

  const markActiveGoalDone = async () => {
    if (!activeGoal || !isTauri() || goalBusy) return;
    setGoalBusy(true);
    try {
      await invoke("goal_mark_status", { id: activeGoal.id, status: "done" });
      setActiveGoal(null);
    } catch (reason) {
      window.alert(String(reason));
    } finally {
      setGoalBusy(false);
    }
  };

  const clearActiveGoal = async () => {
    if (!isTauri() || goalBusy) return;
    setGoalBusy(true);
    try {
      await invoke("goal_clear_active");
      setActiveGoal(null);
    } catch (reason) {
      window.alert(String(reason));
    } finally {
      setGoalBusy(false);
    }
  };

  const openTasks = tasks.filter(
    (task) =>
      task.status === "queued" ||
      task.status === "claimed" ||
      task.status === "running",
  );
  const queuedTasks = tasks.filter((task) => task.status === "queued");

  const syncPlanTasks = async () => {
    if (!isTauri() || taskBusy) return;
    setTaskBusy(true);
    setTaskNote(null);
    try {
      const created = await invoke<AgentTask[]>("task_sync_from_plan");
      setTaskNote(
        created.length === 0
          ? "PLAN tasks already queued"
          : `Queued ${created.length} task${created.length === 1 ? "" : "s"} from PLAN`,
      );
      onRefresh?.();
    } catch (reason) {
      setTaskNote(honestLeaseError(String(reason)));
    } finally {
      setTaskBusy(false);
    }
  };

  const applyClaimedTask = async (task: AgentTask, startTurn: boolean) => {
    if (!isTauri() || busy || taskBusy || !provider.trim() || !model.trim()) {
      return;
    }
    setTaskBusy(true);
    setTaskNote(null);
    try {
      let working = task;
      if (task.status === "queued") {
        working = await invoke<AgentTask>("task_claim_id", {
          taskId: task.id,
          agentId,
          ttlSecs: 300,
        });
      }
      working = await invoke<AgentTask>("task_start", {
        taskId: working.id,
        agentId,
      });
      let executionRoot: string | null = null;
      if (applyIsolate) {
        const info = await invoke<{ path: string }>("worktree_provision_for_task", {
          taskId: working.id,
        });
        executionRoot = info.path;
        setActiveWorktree(info.path);
        setTaskNote(`Isolated · ${info.path}`);
      } else {
        setActiveWorktree(null);
      }
      setClaimedTask(working);
      setForceOwnedPaths(working.owned_paths);
      const nextAutonomy: AutonomyLevel =
        autonomy === "automate" ? "automate" : "act";
      setAutonomyPersisted(nextAutonomy);
      if (!applyIsolate) {
        setTaskNote(`Applying ${working.id.slice(0, 8)}`);
      }
      onRefresh?.();
      if (startTurn) {
        const text = working.goal.trim();
        setPrompt("");
        setCurrentUser(text);
        stickToBottomRef.current = true;
        const effortOpts = EFFORT_OPTIONS.find(
          (item) => item.id === effectiveEffort(nextAutonomy, effort),
        );
        onRun({
          prompt: text,
          provider: provider.trim(),
          baseUrl: baseUrl.trim(),
          model: model.trim(),
          inputCostPerMtok: Number(inputCost) || 0,
          outputCostPerMtok: Number(outputCost) || 0,
          sessionCapUsd: Number(sessionCap),
          dailyCapUsd: Number(dailyCap),
          autonomy: nextAutonomy,
          maxSteps: maxSteps.trim()
            ? Math.max(1, Number(maxSteps) || 32)
            : (effortOpts?.maxSteps ?? 32),
          maxTokens: maxTokens.trim()
            ? Number(maxTokens) || null
            : providerSupportsEffort(provider)
              ? (effortOpts?.maxTokens ?? null)
              : null,
          verifyOnComplete:
            nextAutonomy === "automate" ? true : verifyOnComplete,
          verifyGate: verifyGate.trim() || "G3",
          approveOwnedPaths: true,
          ownedPaths: working.owned_paths,
          leaseAgentId: agentId,
          preferredShellCwd: preferredShellCwd(shellScope),
          executionRoot,
        });
      }
    } catch (reason) {
      setTaskNote(honestLeaseError(String(reason)));
      setActiveWorktree(null);
    } finally {
      setTaskBusy(false);
    }
  };

  useEffect(() => {
    window.localStorage.setItem(AGENT_PROVIDER_KEY, provider);
  }, [provider]);
  useEffect(() => {
    const next = canonicalBaseUrl(provider, baseUrl);
    if (next !== baseUrl) {
      setBaseUrl(next);
      return;
    }
    window.localStorage.setItem(AGENT_BASE_URL_KEY, next);
  }, [baseUrl, provider]);
  useEffect(() => {
    window.localStorage.setItem(AGENT_MODEL_KEY, model);
  }, [model]);
  useEffect(() => {
    window.localStorage.setItem(AGENT_EFFORT_KEY, effort);
  }, [effort]);
  useEffect(() => {
    window.localStorage.setItem("ade_session_cap_usd", sessionCap);
  }, [sessionCap]);
  useEffect(() => {
    window.localStorage.setItem("ade_daily_cap_usd", dailyCap);
  }, [dailyCap]);

  useEffect(() => {
    // Standard surface: keep Suggest/Apply only unless Debug advanced options are open.
    if (!devMode && (autonomy === "observe" || autonomy === "automate")) {
      setAutonomyPersisted(autonomy === "automate" ? "act" : "propose");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [devMode]);

  useEffect(() => {
    if (autonomy === "automate") {
      setVerifyOnComplete(true);
    }
    if (autonomy === "observe" || autonomy === "propose") {
      setVerifyOnComplete(false);
    }
  }, [autonomy]);

  const buildTurnInput = (overrides?: {
    autonomy?: AutonomyLevel;
    effort?: EffortLevel;
    context?: "normal" | "continuity";
  }) => {
    const nextAutonomy = overrides?.autonomy ?? autonomy;
    const resolvedEffort = effectiveEffort(
      nextAutonomy,
      overrides?.effort ?? effort,
      overrides?.context ?? "normal",
    );
    const effortOpts = EFFORT_OPTIONS.find((item) => item.id === resolvedEffort);
    return {
    prompt: prompt.trim(),
    provider: provider.trim(),
    baseUrl: baseUrl.trim(),
    model: model.trim(),
    inputCostPerMtok: Number(inputCost) || 0,
    outputCostPerMtok: Number(outputCost) || 0,
    sessionCapUsd: Number(sessionCap),
    dailyCapUsd: Number(dailyCap),
    autonomy: nextAutonomy,
    maxSteps: maxSteps.trim()
      ? Math.max(1, Number(maxSteps) || 32)
      : (effortOpts?.maxSteps ?? 32),
    maxTokens: maxTokens.trim()
      ? Number(maxTokens) || null
      : providerSupportsEffort(provider)
        ? (effortOpts?.maxTokens ?? null)
        : null,
    verifyOnComplete: nextAutonomy === "automate" ? true : verifyOnComplete,
    verifyGate: verifyGate.trim() || "G3",
    // Apply/Automate = write scope; Debug dogfood can pin .ade/dogfood.
    approveOwnedPaths:
      nextAutonomy === "act" || nextAutonomy === "automate",
    ownedPaths:
      nextAutonomy === "act" || nextAutonomy === "automate"
        ? effectiveOwnedPaths
        : [],
    leaseAgentId:
      nextAutonomy === "act" || nextAutonomy === "automate" ? agentId : null,
    preferredShellCwd: preferredShellCwd(shellScope),
    executionRoot: null,
  };
  };

  const submit = () => {
    const text = prompt.trim();
    if (!text) return;
    if (mutating) {
      const conflict = writableConflict(
        leases as StripLease[],
        agentId,
        effectiveOwnedPaths,
      );
      if (conflict) {
        window.alert(
          `Another agent (${conflict.agent_id.slice(0, 8)}) holds a write lease on ${conflict.path}. Switch to Suggest or wait.`,
        );
        return;
      }
    }
    setPrompt("");
    setCurrentUser(text);
    stickToBottomRef.current = true;
    onRun({ ...buildTurnInput(), prompt: text });
  };

  useEffect(() => {
    if (!currentUser || busy) return;
    const terminal = events.some(
      (event) =>
        event.type === "completed" ||
        event.type === "failed" ||
        event.type === "cancelled",
    );
    // G0: never leave an orphan YOU bubble after the host finishes.
    if (!terminal && events.length === 0) {
      setPastTurns((turns) => [
        ...turns,
        {
          id:
            typeof crypto !== "undefined" && "randomUUID" in crypto
              ? crypto.randomUUID()
              : `turn-${Date.now()}`,
          createdAt: new Date().toISOString(),
          user: currentUser,
          events: [{ type: "failed", error: "Turn ended without a result." }],
        },
      ]);
      setCurrentUser(null);
      if (claimedTask && isTauri()) {
        void invoke("task_fail", {
          taskId: claimedTask.id,
          agentId,
          failure: "Turn ended without a result.",
        })
          .then(() => {
            setClaimedTask(null);
            setForceOwnedPaths(null);
            setActiveWorktree(null);
            onRefresh?.();
          })
          .catch(() => {});
      }
      return;
    }
    if (!terminal) return;
    setPastTurns((turns) => [
      ...turns,
      {
        id:
          typeof crypto !== "undefined" && "randomUUID" in crypto
            ? crypto.randomUUID()
            : `turn-${Date.now()}`,
        createdAt: new Date().toISOString(),
        user: currentUser,
        events: [...events],
      },
    ]);
    setCurrentUser(null);

    // G3: settle claimed task when the Apply turn terminates.
    if (claimedTask && isTauri()) {
      const failed = [...events]
        .reverse()
        .find((event) => event.type === "failed" || event.type === "cancelled");
      const taskId = claimedTask.id;
      const holder = agentId;
      if (failed && (failed.type === "failed" || failed.type === "cancelled")) {
        const reason =
          failed.type === "failed"
            ? failed.error
            : failed.reason || "cancelled";
        void invoke("task_fail", {
          taskId,
          agentId: holder,
          failure: reason,
        })
          .then(() => {
            setClaimedTask(null);
            setForceOwnedPaths(null);
            setActiveWorktree(null);
            setTaskNote(`Task failed · ${taskId.slice(0, 8)}`);
            onRefresh?.();
          })
          .catch((err) => setTaskNote(honestLeaseError(String(err))));
      } else {
        const tree = activeWorktree;
        void invoke("task_complete", { taskId, agentId: holder })
          .then(async () => {
            if (tree) {
              try {
                await invoke("worktree_remove", { path: tree, force: true });
              } catch {
                // leave dirty worktree for human review
              }
            }
            setClaimedTask(null);
            setForceOwnedPaths(null);
            setActiveWorktree(null);
            setTaskNote(
              tree
                ? `Task done · cleaned worktree · ${taskId.slice(0, 8)}`
                : `Task done · ${taskId.slice(0, 8)}`,
            );
            onRefresh?.();
          })
          .catch((err) => setTaskNote(honestLeaseError(String(err))));
      }
    }
  }, [events, currentUser, busy, claimedTask, agentId, onRefresh, activeWorktree]);

  useEffect(() => {
    let input = 0;
    let output = 0;
    for (const event of events) {
      if (event.type === "started") {
        input = 0;
        output = 0;
      }
      if (event.type === "usage") {
        input = event.input_tokens;
        output = event.output_tokens;
      }
      if (event.type === "completed") {
        input = event.result.usage.input_tokens;
        output = event.result.usage.output_tokens;
        if (sessionCountedRef.current !== event.result.session_id) {
          sessionCountedRef.current = event.result.session_id;
          setSessionTokens((prev) => ({
            input: prev.input + event.result.usage.input_tokens,
            output: prev.output + event.result.usage.output_tokens,
            costMicros: prev.costMicros + event.result.cost_micros,
          }));
        }
      }
    }
    setTurnTokens({ input, output });
  }, [events]);

  const contextUsed = turnTokens.input > 0 ? turnTokens.input : Math.round(prompt.length / 4);
  const contextPct = Math.min(100, Math.round((contextUsed / contextLimit) * 100));

  const feedEvents = useMemo(() => {
    const archived = pastTurns.flatMap((turn) => [
      { type: "user_message" as const, text: turn.user },
      ...turn.events,
    ]);
    if (!currentUser) return archived;
    return [
      ...archived,
      { type: "user_message" as const, text: currentUser },
      ...events,
    ];
  }, [pastTurns, currentUser, events]);

  useEffect(() => {
    if (!stickToBottomRef.current) return;
    const node = feedBottomRef.current;
    if (!node) return;
    // Instant while streaming; smooth when idle after growth.
    node.scrollIntoView({ block: "end", behavior: busy ? "auto" : "smooth" });
  }, [feedEvents, busy, pastTurns.length, currentUser]);

  const onFeedScroll = () => {
    const el = feedScrollRef.current;
    if (!el) return;
    const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
    stickToBottomRef.current = distance < 96;
  };

  useEffect(() => {
    if (!autoSubmit) return;
    onAutoSubmitHandled?.();
    const nextPrompt = (initialPrompt.trim() || prompt).trim();
    if (!nextPrompt || !provider.trim() || !model.trim() || busy || !isTauri()) {
      return;
    }
    const continuity = autoSubmitContext === "continuity";
    const nextAutonomy: AutonomyLevel = continuity ? "act" : autonomy;
    const nextEffort = effectiveEffort(
      nextAutonomy,
      effort,
      continuity ? "continuity" : "normal",
    );
    if (continuity) {
      setAutonomyPersisted("act");
      setEffort(nextEffort);
      window.localStorage.setItem(AGENT_EFFORT_KEY, nextEffort);
      setMaxSteps(
        String(
          EFFORT_OPTIONS.find((item) => item.id === nextEffort)?.maxSteps ?? 24,
        ),
      );
    }
    setPrompt("");
    setCurrentUser(nextPrompt);
    stickToBottomRef.current = true;
    onRun({
      ...buildTurnInput({
        autonomy: nextAutonomy,
        effort: nextEffort,
        context: continuity ? "continuity" : "normal",
      }),
      prompt: nextPrompt,
    });
    // Intentionally one-shot when autoSubmit flips true
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoSubmit]);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      {rebuildLockWarnings.length > 0 && showDebugHarness && (
        <div className="shrink-0 rounded-xl border border-amber-400/25 bg-amber-400/8 px-4 py-3 text-[11px] leading-5 text-amber-100/85">
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

      {activeGoal && (
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 rounded-xl border border-emerald-400/20 bg-emerald-500/8 px-3 py-2">
          <div className="min-w-0">
            <div className="text-[11px] font-semibold text-emerald-100">
              Active goal
              <span className="ml-1.5 font-normal text-emerald-100/55">
                · {activeGoal.shellScope === "home" ? "Home" : "Workspace"} ·{" "}
                {activeGoal.autonomy === "act"
                  ? "Apply"
                  : activeGoal.autonomy === "automate"
                    ? "Automate"
                    : "Suggest"}
                {activeGoal.verifyGate ? ` · ${activeGoal.verifyGate}` : ""}
              </span>
            </div>
            <div className="truncate text-[11px] text-slate-300" title={activeGoal.statement}>
              {activeGoal.statement}
            </div>
          </div>
          <div className="flex shrink-0 flex-wrap items-center gap-1.5">
            <button
              type="button"
              disabled={busy || goalBusy || !provider.trim() || !model.trim()}
              onClick={runActiveGoal}
              className="rounded-md border border-emerald-400/30 bg-emerald-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-emerald-100 hover:bg-emerald-500/25 disabled:opacity-40"
            >
              Run goal
            </button>
            <button
              type="button"
              disabled={goalBusy}
              onClick={() => void markActiveGoalDone()}
              className="rounded-md border border-white/10 bg-white/5 px-2.5 py-1.5 text-[11px] font-semibold text-slate-300 hover:bg-white/8 disabled:opacity-40"
            >
              Done
            </button>
            <button
              type="button"
              disabled={goalBusy}
              onClick={() => void clearActiveGoal()}
              className="rounded-md px-1.5 py-1.5 text-[10px] font-semibold text-slate-500 hover:bg-white/5 hover:text-slate-300 disabled:opacity-40"
              title="Clear active pointer (keeps goal file)"
            >
              Clear
            </button>
          </div>
        </div>
      )}

      {(planPhaseCount > 0 || openTasks.length > 0 || claimedTask || taskNote) && (
        <div className="shrink-0 space-y-2 rounded-xl border border-violet-400/20 bg-violet-500/8 px-3 py-2">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="min-w-0">
              <div className="text-[11px] font-semibold text-violet-100">
                Role split
                <span className="ml-1.5 font-normal text-violet-100/55">
                  · Suggest queues · Apply claims one
                  {openTasks.length > 0
                    ? ` · ${openTasks.length} open`
                    : planPhaseCount > 0
                      ? ` · ${planPhaseCount} PLAN phase${planPhaseCount === 1 ? "" : "s"}`
                      : ""}
                </span>
              </div>
              {claimedTask ? (
                <div
                  className="truncate text-[11px] text-slate-300"
                  title={
                    activeWorktree
                      ? `${claimedTask.goal}\n${activeWorktree}`
                      : claimedTask.goal
                  }
                >
                  Claimed · {claimedTask.goal}
                  {activeWorktree ? " · isolated" : ""}
                </div>
              ) : (
                <div className="text-[10px] text-slate-500">
                  Queue PLAN as tasks, then Apply one under leases
                  {applyIsolate ? " (isolated worktree)" : ""}.
                </div>
              )}
            </div>
            <div className="flex shrink-0 flex-wrap items-center gap-1.5">
              <label
                className="flex cursor-pointer items-center gap-1 rounded-md border border-white/8 bg-black/20 px-1.5 py-1 text-[10px] font-semibold text-slate-400"
                title="G4: run Apply in a linked git worktree; leases stay on primary checkout"
              >
                <input
                  type="checkbox"
                  className="accent-violet-400"
                  checked={applyIsolate}
                  disabled={busy || taskBusy}
                  onChange={(event) => {
                    const next = event.target.checked;
                    setApplyIsolate(next);
                    window.localStorage.setItem(
                      APPLY_ISOLATE_KEY,
                      next ? "1" : "0",
                    );
                  }}
                />
                Isolate
              </label>
              <button
                type="button"
                disabled={taskBusy || busy || planPhaseCount === 0 || !isTauri()}
                onClick={() => void syncPlanTasks()}
                className="rounded-md border border-violet-400/30 bg-violet-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-violet-100 hover:bg-violet-500/25 disabled:opacity-40"
                title="Enqueue each PLAN phase as a lease-backed task"
              >
                Queue PLAN
              </button>
              {queuedTasks[0] && (
                <button
                  type="button"
                  disabled={taskBusy || busy || !provider.trim() || !model.trim()}
                  onClick={() => void applyClaimedTask(queuedTasks[0]!, true)}
                  className="rounded-md border border-blue-400/30 bg-blue-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/25 disabled:opacity-40"
                >
                  Apply next
                </button>
              )}
            </div>
          </div>
          {queuedTasks.length > 0 && (
            <div className="space-y-1">
              {queuedTasks.slice(0, 4).map((task) => (
                <div
                  key={task.id}
                  className="flex items-center justify-between gap-2 rounded-md border border-white/6 bg-black/20 px-2 py-1.5"
                >
                  <div className="min-w-0 truncate text-[11px] text-slate-300" title={task.goal}>
                    {task.goal}
                    <span className="ml-1.5 text-[10px] text-slate-600">
                      {task.owned_paths.length} path
                      {task.owned_paths.length === 1 ? "" : "s"}
                    </span>
                  </div>
                  <button
                    type="button"
                    disabled={taskBusy || busy || !provider.trim() || !model.trim()}
                    onClick={() => void applyClaimedTask(task, true)}
                    className="shrink-0 rounded-md px-1.5 py-0.5 text-[10px] font-semibold text-blue-200 hover:bg-blue-500/15 disabled:opacity-40"
                  >
                    Apply
                  </button>
                </div>
              ))}
            </div>
          )}
          {taskNote && (
            <div className="text-[10px] text-slate-400">{taskNote}</div>
          )}
        </div>
      )}

      {handoffAvailable && onContinueHandoff && !busy && (
        <div
          className={`flex shrink-0 flex-wrap items-center justify-between gap-2 rounded-xl border px-3 py-2 ${
            handoffLatestStatus === "budget_exhausted"
              ? "border-amber-400/25 bg-amber-500/8"
              : "border-blue-400/20 bg-blue-500/8"
          }`}
        >
          <div className="min-w-0">
            <div
              className={`text-[11px] font-semibold ${
                handoffLatestStatus === "budget_exhausted"
                  ? "text-amber-100"
                  : "text-blue-100"
              }`}
            >
              {handoffLatestStatus === "budget_exhausted"
                ? "Budget exhausted — continue with more Effort"
                : "Continue last handoff"}
            </div>
            <div className="text-[10px] text-slate-500">
              {handoffLatestStatus === "budget_exhausted"
                ? "Host runs next_safe_command, then resumes at Med+ Effort (Apply)."
                : "Host runs next_safe_command, then resumes with thrift Continuity prompt."}
            </div>
          </div>
          <button
            type="button"
            onClick={onContinueHandoff}
            className={`shrink-0 rounded-md border px-2.5 py-1.5 text-[11px] font-semibold ${
              handoffLatestStatus === "budget_exhausted"
                ? "border-amber-400/35 bg-amber-500/15 text-amber-100 hover:bg-amber-500/25"
                : "border-blue-400/30 bg-blue-500/15 text-blue-100 hover:bg-blue-500/25"
            }`}
          >
            {handoffLatestStatus === "budget_exhausted"
              ? "Continue · raise Effort"
              : "Continue"}
          </button>
        </div>
      )}

      <AgentSessionStrip
        agentId={agentId}
        mutating={mutating}
        leases={leases as StripLease[]}
        ownedPaths={mutating ? (effectiveOwnedPaths) : []}
        busy={busy}
        shellScope={shellScope}
        onNewAgent={() => setAgentId(rotateAgentId(workspaceRoot))}
      />

      <div
        ref={feedScrollRef}
        onScroll={onFeedScroll}
        className="thin-scrollbar min-h-0 flex-1 overflow-y-auto scroll-smooth"
      >
        {(pastTurns.length > 0 || chatError) && (
          <div className="mb-2 flex flex-wrap items-center justify-between gap-2 px-0.5">
            <div className="text-[10px] text-slate-600">
              {chatError
                ? `Chat save: ${chatError}`
                : `${pastTurns.length} saved turn${pastTurns.length === 1 ? "" : "s"} · .ade/chat`}
            </div>
            <button
              type="button"
              disabled={busy}
              onClick={clearChat}
              className="rounded-md px-1.5 py-0.5 text-[10px] font-semibold text-slate-500 hover:bg-white/5 hover:text-slate-300 disabled:opacity-40"
            >
              Clear chat
            </button>
          </div>
        )}
        <AgentActivityFeed
          events={feedEvents}
          busy={busy}
          simpleMode
          maxSteps={
            maxSteps.trim()
              ? Math.max(1, Number(maxSteps) || 32)
              : (EFFORT_OPTIONS.find(
                  (item) => item.id === effectiveEffort(autonomy, effort),
                )?.maxSteps ?? 32)
          }
          autonomyLabel={
            autonomy === "act"
              ? "Apply"
              : autonomy === "automate"
                ? "Automate"
                : autonomy === "observe"
                  ? "Observe"
                  : "Suggest"
          }
          scopeLabel={shellScope === "home" ? "Home" : "Workspace"}
          autonomySuggest={autonomy === "propose" || autonomy === "observe"}
          onSwitchToApply={() => setAutonomyPersisted("act")}
          onSwitchToHomeScope={() => setShellScopePersisted("home")}
          onPrefillPrompt={(text) => {
            setPrompt(text);
            requestAnimationFrame(() => composerRef.current?.focus());
          }}
          onSelectOption={(text) => {
            const trimmed = text.trim();
            if (!trimmed || busy || !provider.trim() || !model.trim() || !isTauri()) {
              return;
            }
            if (mutating) {
              const conflict = writableConflict(
                leases as StripLease[],
                agentId,
                effectiveOwnedPaths,
              );
              if (conflict) {
                window.alert(
                  `Another agent (${conflict.agent_id.slice(0, 8)}) holds a write lease on ${conflict.path}. Switch to Suggest or wait.`,
                );
                return;
              }
            }
            setPrompt("");
            setCurrentUser(trimmed);
            stickToBottomRef.current = true;
            onRun({ ...buildTurnInput(), prompt: trimmed });
          }}
          failureAdvice={failureAdvice}
          failureBusy={busy}
          onFailureAction={(action) => applyFailureAction(action)}
        />
        {failureNote && latestFailed && !busy && (
          <p className="mt-2 text-[11px] text-amber-100/90">{failureNote}</p>
        )}
        <div ref={feedBottomRef} className="h-px w-full shrink-0" aria-hidden />
        {showDebugHarness && (
          <div className="mt-3 space-y-3">
            <Disclosure
              title="Advanced run options"
              summary={
                autonomy === "observe" || autonomy === "automate"
                  ? autonomy
                  : verifyOnComplete
                    ? `verify ${verifyGate}`
                    : "defaults"
              }
              defaultOpen={false}
              storageKey="ade_agent_advanced_run"
            >
              <div className="space-y-2">
                <div className="grid grid-cols-2 gap-1">
                  <button
                    type="button"
                    onClick={() => setAutonomyPersisted("observe")}
                    className={`rounded-md border px-1.5 py-1.5 text-center text-[11px] font-semibold transition ${
                      autonomy === "observe"
                        ? "border-blue-400/40 bg-blue-500/20 text-blue-100"
                        : "border-white/8 bg-white/2 text-slate-400"
                    }`}
                  >
                    Observe
                  </button>
                  <button
                    type="button"
                    onClick={() => setAutonomyPersisted("automate")}
                    className={`rounded-md border px-1.5 py-1.5 text-center text-[11px] font-semibold transition ${
                      autonomy === "automate"
                        ? "border-blue-400/40 bg-blue-500/20 text-blue-100"
                        : "border-white/8 bg-white/2 text-slate-400"
                    }`}
                  >
                    Automate
                  </button>
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {PROMPT_PRESETS.map((item) => (
                    <Chip
                      key={item.label}
                      onClick={() => {
                        setForceOwnedPaths(null);
                        setAutonomyPersisted(item.autonomy);
                        setPrompt(item.prompt);
                      }}
                    >
                      {item.label}
                    </Chip>
                  ))}
                  <Chip
                    onClick={() => {
                      setAutonomyPersisted("automate");
                      setVerifyGate("G3");
                      setForceOwnedPaths([".ade/dogfood"]);
                      setPrompt(
                        "N3 dogfood Automate: write ONLY under .ade/dogfood/. Create or update automate-acceptance.md with ISO time, autonomy=automate, owned path=.ade/dogfood, and that verify G3 was requested. Do not edit crates/ or apps/.",
                      );
                    }}
                  >
                    Dogfood Automate
                  </Chip>
                </div>
                <ProviderSelect
                  value={provider}
                  showRecommended={false}
                  onChange={(preset) => {
                    setProvider(preset.id);
                    setBaseUrl(preset.baseUrl);
                    setModel(preset.models[0] ?? DEFAULT_MODEL);
                  }}
                />
                <Field label="Base URL" value={baseUrl} onChange={setBaseUrl} mono />
                <div className="grid grid-cols-2 gap-2">
                  <Field label="Input $/MTok" value={inputCost} onChange={setInputCost} />
                  <Field label="Output $/MTok" value={outputCost} onChange={setOutputCost} />
                  <Field
                    label="Max steps"
                    value={maxSteps}
                    onChange={setMaxSteps}
                    placeholder="from effort"
                  />
                  <Field
                    label="Max tokens"
                    value={maxTokens}
                    onChange={setMaxTokens}
                    placeholder="effort / unlimited"
                  />
                  <Field label="Session cap $" value={sessionCap} onChange={setSessionCap} />
                  <Field label="Daily cap $" value={dailyCap} onChange={setDailyCap} />
                </div>
              </div>
            </Disclosure>
          </div>
        )}
      </div>

      <div className="shrink-0 rounded-2xl border border-white/10 bg-[#141a22]">
        <textarea
          ref={composerRef}
          value={prompt}
          onChange={(event) => setPrompt(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              if (!busy && prompt.trim() && isTauri()) submit();
            }
          }}
          rows={3}
          className="min-h-16 w-full resize-none border-0 bg-transparent px-4 pt-3 text-[14px] leading-5 text-slate-200 outline-none placeholder:text-slate-500"
          placeholder="Ask ADE to plan or build…"
        />
        <div className="flex items-center gap-1.5 px-3 pb-3 pt-1">
          <DarkSelect
            ariaLabel="Mode"
            title={
              autonomy === "automate"
                ? "Auto: apply + verify when done"
                : autonomy === "act"
                  ? "Apply: shell + writes"
                  : "Suggest: plan / inspect only"
            }
            value={
              autonomy === "automate"
                ? "automate"
                : autonomy === "act"
                  ? "act"
                  : "propose"
            }
            options={[
              { value: "propose", label: "Suggest" },
              { value: "act", label: "Apply" },
              { value: "automate", label: "Auto" },
            ]}
            maxLabelChars={10}
            onChange={(next) => {
              if (next === "automate") {
                setAutonomyPersisted("automate");
                setVerifyOnComplete(true);
                if (effort === "low") {
                  setEffort("medium");
                  window.localStorage.setItem(AGENT_EFFORT_KEY, "medium");
                }
              } else if (next === "act") {
                setAutonomyPersisted("act");
                if (effort === "low") {
                  setEffort("medium");
                  window.localStorage.setItem(AGENT_EFFORT_KEY, "medium");
                }
              } else {
                setAutonomyPersisted("propose");
              }
            }}
          />
          <DarkSelect
            ariaLabel="Shell scope"
            title={
              shellScope === "home"
                ? "Home: shell defaults to Desktop"
                : "Workspace: shell defaults to this repo"
            }
            value={shellScope}
            options={[
              { value: "workspace", label: "Workspace" },
              { value: "home", label: "Home" },
            ]}
            maxLabelChars={12}
            onChange={(next) =>
              setShellScopePersisted(next === "home" ? "home" : "workspace")
            }
          />
          <ComposerModelSelect
            providerId={provider}
            baseUrl={baseUrl}
            model={model}
            onProviderChange={(preset) => {
              setProvider(preset.id);
              setBaseUrl(preset.baseUrl);
            }}
            onModelChange={setModel}
          />

          <div className="min-w-0 flex-1" />

          <ContextUsageButton
            contextPct={contextPct}
            contextUsed={contextUsed}
            contextLimit={contextLimit}
            turnTokens={turnTokens}
            sessionTokens={sessionTokens}
            showSpend={Number(inputCost) > 0 || Number(outputCost) > 0}
            effort={effort}
            showEffort={providerSupportsEffort(provider)}
            onEffort={setEffort}
            onSaveGoal={
              prompt.trim() && isTauri()
                ? () => void savePromptAsGoal()
                : undefined
            }
            goalBusy={goalBusy || busy}
          />

          <button
            type="button"
            onClick={submit}
            disabled={
              busy || !prompt.trim() || !provider.trim() || !model.trim() || !isTauri()
            }
            className="grid size-8 shrink-0 place-items-center rounded-lg bg-blue-500 text-sm font-bold text-white hover:bg-blue-400 disabled:opacity-40"
            title={isTauri() ? "Send" : "Desktop only"}
          >
            {busy ? "…" : "↑"}
          </button>
        </div>
      </div>
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
    if (typeof window === "undefined") return DEFAULT_PROVIDER;
    return window.localStorage.getItem(AGENT_PROVIDER_KEY) || DEFAULT_PROVIDER;
  });
  const [profile, setProfile] = useState("local");
  const [secret, setSecret] = useState("");
  const [baseUrl, setBaseUrl] = useState(() => {
    if (typeof window === "undefined") return DEFAULT_BASE_URL;
    const storedProvider =
      window.localStorage.getItem(AGENT_PROVIDER_KEY) || DEFAULT_PROVIDER;
    const stored =
      window.localStorage.getItem(AGENT_BASE_URL_KEY) || DEFAULT_BASE_URL;
    return canonicalBaseUrl(storedProvider, stored);
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
  const [vaultRows, setVaultRows] = useState<
    { provider: string; configured: boolean }[]
  >([]);
  const [smoke, setSmoke] = useState<ProviderKeySmokeResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    if (!provider.trim() || !profile.trim()) return;
    setBusy(true);
    setMessage(null);
    try {
      const [result, rows] = await Promise.all([
        invoke<ProviderKeyStatus>("key_status", {
          provider: provider.trim(),
          profile: profile.trim(),
        }),
        invoke<{ provider: string; configured: boolean }[]>("key_status_all", {
          profile: profile.trim(),
        }).catch(() => [] as { provider: string; configured: boolean }[]),
      ]);
      setStatus(result);
      setVaultRows(rows);
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  }, [profile, provider]);

  const importOpenCodeAuth = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await invoke<{
        imported: string[];
        skipped: string[];
        detail: string;
      }>("key_import_opencode_auth", { profile: profile.trim() || "local" });
      setMessage(
        `Imported ${result.imported.join(", ") || "(none)"}. ${result.detail}`,
      );
      if (result.imported.includes("opencode")) {
        applyPreset(presetById("opencode") ?? PROVIDER_PRESETS[0]);
      }
      await refreshStatus();
    } catch (reason) {
      setMessage(String(reason));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    window.localStorage.setItem(
      AGENT_PROVIDER_KEY,
      provider.trim() || DEFAULT_PROVIDER,
    );
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

  const applyPreset = (preset: (typeof PROVIDER_PRESETS)[number]) => {
    setProvider(preset.id);
    setBaseUrl(preset.baseUrl);
    setModel(preset.models[0] ?? DEFAULT_MODEL);
    setStatus(null);
    setSmoke(null);
  };

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
      window.localStorage.setItem(AGENT_BASE_URL_KEY, baseUrl.trim());
      window.localStorage.setItem(AGENT_MODEL_KEY, model.trim());
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
    <div className="grid grid-cols-1 gap-5 lg:grid-cols-[1fr_320px]">
      {isTauri() && (
        <div className="lg:col-span-2">
          <SpendUsageStrip />
        </div>
      )}
      <Panel
        title="Provider & model"
        subtitle="BYOK — keys persist across rebuilds (Credential Manager + %LOCALAPPDATA%\\ade\\provider-keys). OpenCode Zen ≠ FreeLLMAPI."
      >
        <div className="max-w-2xl space-y-4">
          <ProviderSelect
            value={provider}
            onChange={(preset) => {
              applyPreset(preset);
              // Always reset model when changing provider from Keys too.
              setModel(preset.models[0] ?? DEFAULT_MODEL);
            }}
          />

          <ModelPicker
            providerId={provider}
            baseUrl={baseUrl}
            value={model}
            onChange={setModel}
            apiKey={secret.trim() || null}
          />

          <label className="block text-[11px] text-slate-500">
            <span className="mb-1.5 block text-[10px] font-semibold uppercase tracking-wider text-slate-600">
              API key
            </span>
            <input
              type="password"
              value={secret}
              onChange={(event) => setSecret(event.target.value)}
              autoComplete="new-password"
              spellCheck={false}
              placeholder={
                status?.configured ? "Enter a replacement key" : "Paste provider key"
              }
              className="w-full rounded-lg border border-white/10 bg-[#101620] px-3 py-2 font-mono text-xs text-slate-200"
            />
          </label>

          <Disclosure
            title="Advanced"
            subtitle="Profile, base URL, smoke"
            summary={profile}
            hint="Most turns only need provider, model, and key."
            defaultOpen={false}
            storageKey="ade_keys_advanced"
          >
            <div className="space-y-3">
              <Field
                label="Profile"
                value={profile}
                onChange={(value) => {
                  setProfile(value);
                  setStatus(null);
                  setSmoke(null);
                }}
              />
              <Field label="API base URL" value={baseUrl} onChange={setBaseUrl} mono />
              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void runSmoke()}
                  disabled={busy || !provider.trim() || !profile.trim()}
                  className="rounded-lg border border-white/10 px-3 py-1.5 text-[11px] text-slate-300 hover:bg-white/5 disabled:opacity-50"
                >
                  Safe smoke preflight
                </button>
                <button
                  type="button"
                  onClick={() => void remove()}
                  disabled={busy || !status?.configured}
                  className="rounded-lg border border-red-400/20 px-3 py-1.5 text-[11px] text-red-300 hover:bg-red-400/5 disabled:opacity-40"
                >
                  Delete key
                </button>
              </div>

              {!simpleMode && (
                <Disclosure
                  title="Live credential validation"
                  subtitle="Optional billable smoke"
                  summary="capped"
                  defaultOpen={false}
                  storageKey="ade_keys_live_smoke"
                >
                  <p className="text-[10px] leading-5 text-slate-500">
                    One 16-token-max turn. Pricing required so ADE can refuse before network if
                    worst-case exceeds your cap.
                  </p>
                  <div className="mt-3 grid grid-cols-2 gap-3">
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
                    <Field
                      label="Maximum cost (USD)"
                      value={maxCostUsd}
                      onChange={setMaxCostUsd}
                    />
                  </div>
                  <label className="mt-3 flex items-start gap-2 text-[10px] leading-5 text-slate-400">
                    <input
                      type="checkbox"
                      checked={approveLiveCost}
                      onChange={(event) => setApproveLiveCost(event.target.checked)}
                      className="mt-1"
                    />
                    I approve one potentially billable provider request, bounded by the maximum
                    above.
                  </label>
                  <button
                    type="button"
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
                    className="mt-3 rounded-lg border border-blue-400/30 px-3 py-1.5 text-[11px] font-semibold text-blue-300 hover:bg-blue-400/5 disabled:opacity-40"
                  >
                    Run capped live smoke
                  </button>
                </Disclosure>
              )}
            </div>
          </Disclosure>

          <div className="flex flex-wrap gap-2 pt-1">
            <button
              type="button"
              onClick={() => void save(true)}
              disabled={busy || !provider.trim() || !profile.trim() || !secret.trim()}
              className="rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
            >
              {status?.configured ? "Replace key & open Home" : "Save key & open Home"}
            </button>
            {status?.configured && (
              <button
                type="button"
                onClick={() => onContinueToAgent?.()}
                disabled={busy}
                className="rounded-lg border border-blue-400/30 px-4 py-2 text-xs font-semibold text-blue-200 hover:bg-blue-400/5 disabled:opacity-50"
              >
                Continue to Home
              </button>
            )}
            <button
              type="button"
              onClick={() => void importOpenCodeAuth()}
              disabled={busy || !isTauri()}
              title="Copy keys from OpenCode Desktop auth.json into the ADE vault"
              className="rounded-lg border border-white/10 px-4 py-2 text-xs font-semibold text-slate-300 hover:bg-white/5 disabled:opacity-50"
            >
              Import from OpenCode
            </button>
            {!secret.trim() && status?.configured && (
              <button
                type="button"
                onClick={() => {
                  window.localStorage.setItem(AGENT_PROVIDER_KEY, provider.trim());
                  window.localStorage.setItem(AGENT_BASE_URL_KEY, baseUrl.trim());
                  window.localStorage.setItem(AGENT_MODEL_KEY, model.trim());
                  onContinueToAgent?.();
                }}
                className="rounded-lg border border-white/10 px-4 py-2 text-xs text-slate-300 hover:bg-white/5"
              >
                Use this model on Home
              </button>
            )}
          </div>

          {message && (
            <div className="rounded-lg border border-white/7 bg-white/2 p-3 text-xs text-slate-400">
              {message}
            </div>
          )}
        </div>
      </Panel>

      <div className="space-y-5">
        <Panel title="Configured providers" subtitle="Vault presence · FreeLLMAPI-style health strip">
          <div className="space-y-2">
            {(vaultRows.length > 0
              ? vaultRows
              : PROVIDER_PRESETS.map((preset) => ({
                  provider: preset.id,
                  configured: false,
                }))
            ).map((row) => {
              const preset = presetById(row.provider);
              const active = provider === row.provider;
              return (
                <button
                  key={row.provider}
                  type="button"
                  onClick={() => {
                    const next = presetById(row.provider);
                    if (next) applyPreset(next);
                  }}
                  className={`flex w-full items-center justify-between gap-3 rounded-lg border px-3 py-2 text-left transition ${
                    active
                      ? "border-blue-400/35 bg-blue-500/10"
                      : "border-white/7 bg-white/2 hover:border-white/15"
                  }`}
                >
                  <div className="min-w-0">
                    <div className="text-[12px] font-semibold text-slate-200">
                      {preset?.label ?? row.provider}
                    </div>
                    <div className="truncate font-mono text-[10px] text-slate-600">
                      {preset?.baseUrl ?? row.provider}
                    </div>
                  </div>
                  <span
                    className={`shrink-0 text-[10px] font-semibold uppercase tracking-wide ${
                      row.configured ? "text-emerald-300" : "text-slate-500"
                    }`}
                  >
                    {row.configured ? "● in vault" : "○ missing"}
                  </span>
                </button>
              );
            })}
          </div>
          <p className="mt-3 text-[10px] leading-5 text-slate-500">
            &quot;In vault&quot; only means ADE has a secret stored — it does not prove
            the API accepts it. A 401 means that stored key is rejected; Import from
            OpenCode copies the same keys from auth.json, so regenerate a fresh Zen
            key at opencode.ai if chat fails. OpenCode Zen = provider opencode +
            https://opencode.ai/zen/v1. FreeLLMAPI = freellmapi-… key + :31415
            (Desktop) or :3001 (Docker) — different products, different keys.
          </p>
        </Panel>

        <Panel title="Selected" subtitle={`${profile || "—"} / ${provider || "—"}`}>
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
              Presence only — the credential value never returns to the UI.
            </p>
            <p className="mt-2 font-mono text-[10px] text-slate-500">
              {presetById(provider)?.label ?? provider} · {model || "—"}
            </p>
          </div>
        </Panel>

        {smoke && (
          <Panel title="Smoke result" subtitle={smoke.status.toUpperCase()}>
            <p className="text-xs leading-5 text-slate-400">{smoke.detail}</p>
            {smoke.status === "ready" || smoke.status === "skipped" ? (
              <p className="mt-3 text-[10px] leading-5 text-slate-600">
                Safe preflight makes no network request and incurs no provider cost.
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

function AuditView({
  audit,
  handoffs,
  onRefresh,
  onOpenSettings,
}: {
  audit: AuditReport;
  handoffs: HandoffHistoryItem[];
  onRefresh: () => void;
  onOpenSettings: () => void;
}) {
  const [ignoreBusy, setIgnoreBusy] = useState(false);
  const [ignoreMessage, setIgnoreMessage] = useState<string | null>(null);
  const [spend, setSpend] = useState<{
    daily_usd: number;
    daily_cap_usd: number;
    session_cap_usd: number;
    period_key: string;
  } | null>(null);

  const alignments = audit.ignore_alignment ?? [];
  const driftCount = alignments.filter(
    (row) => row.status === "Drifted" || row.status === "Missing",
  ).length;

  useEffect(() => {
    if (!isTauri()) return;
    const sessionCap = Number(
      window.localStorage.getItem("ade_session_cap_usd") || "1",
    );
    const dailyCap = Number(window.localStorage.getItem("ade_daily_cap_usd") || "10");
    void invoke<{
      daily_usd: number;
      daily_cap_usd: number;
      session_cap_usd: number;
      period_key: string;
    }>("spend_summary", {
      sessionCapUsd: sessionCap,
      dailyCapUsd: dailyCap,
    })
      .then(setSpend)
      .catch(() => setSpend(null));
  }, [audit.score, handoffs.length]);

  const repairIgnores = async () => {
    if (!isTauri()) return;
    setIgnoreBusy(true);
    setIgnoreMessage(null);
    try {
      const result = await invoke<{ repaired: string[]; detail: string }>(
        "ensure_ignore_surfaces",
      );
      setIgnoreMessage(
        result.repaired.length > 0
          ? `Repaired ${result.repaired.join(", ")}`
          : result.detail || "Ignore surfaces already aligned",
      );
      onRefresh();
    } catch (reason) {
      setIgnoreMessage(String(reason));
    } finally {
      setIgnoreBusy(false);
    }
  };

  const statusTone = (status: string) => {
    if (status === "Synced") return "text-emerald-300 border-emerald-400/25 bg-emerald-400/10";
    if (status === "Drifted") return "text-amber-200 border-amber-400/30 bg-amber-400/10";
    if (status === "Missing") return "text-red-200 border-red-400/30 bg-red-400/10";
    return "text-slate-500 border-white/10 bg-white/3";
  };

  return (
    <div className="space-y-4 p-4 md:p-6">
      <Panel title="Trust" subtitle={`${audit.score}/${audit.score_max} audit points`}>
        <p className="mb-4 text-[12px] leading-5 text-slate-500">
          Ignore drift, spend, and a recent activity log — Trust is the scoreboard,
          not Home.
        </p>

        {audit.blockers.length > 0 && (
          <div className="mb-4 rounded-lg border border-red-400/20 bg-red-400/5 p-4">
            <div className="text-xs font-semibold text-red-300">Blocking findings</div>
            {audit.blockers.map((blocker) => (
              <div key={blocker} className="mt-2 text-xs text-red-200/70">
                • {blocker}
              </div>
            ))}
          </div>
        )}

        <div className="mb-5 rounded-xl border border-white/8 bg-white/2 p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
                Ignore surfaces
              </div>
              <div className="mt-0.5 text-[12px] text-slate-400">
                {driftCount === 0
                  ? "Aligned with ADE always-ignore policy"
                  : `${driftCount} surface${driftCount === 1 ? "" : "s"} need attention`}
              </div>
            </div>
            <button
              type="button"
              disabled={ignoreBusy || !isTauri()}
              onClick={() => void repairIgnores()}
              className="rounded-md border border-blue-400/30 bg-blue-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/25 disabled:opacity-50"
            >
              {ignoreBusy ? "Repairing…" : "Repair ignores"}
            </button>
          </div>
          {ignoreMessage && (
            <div className="mt-2 text-[11px] text-slate-400">{ignoreMessage}</div>
          )}
          <div className="mt-3 space-y-1.5">
            {alignments.length === 0 && (
              <div className="text-[11px] text-slate-600">No ignore data yet — refresh.</div>
            )}
            {alignments.map((row) => (
              <div
                key={row.surface}
                className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-white/6 bg-black/20 px-3 py-2"
              >
                <div className="min-w-0">
                  <div className="font-mono text-[11px] text-slate-200">{row.surface}</div>
                  {row.missing_patterns.length > 0 && (
                    <div className="mt-0.5 truncate text-[10px] text-slate-500">
                      Missing: {row.missing_patterns.slice(0, 4).join(", ")}
                      {row.missing_patterns.length > 4 ? "…" : ""}
                    </div>
                  )}
                </div>
                <span
                  className={`rounded border px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${statusTone(row.status)}`}
                >
                  {row.status}
                </span>
              </div>
            ))}
          </div>
        </div>

        <div className="mb-5 rounded-xl border border-white/8 bg-white/2 p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
                Spend
              </div>
              <div className="mt-0.5 text-[12px] text-slate-400">
                Workspace daily usage vs your hard caps
              </div>
            </div>
            <button
              type="button"
              onClick={onOpenSettings}
              className="inline-flex items-center gap-1.5 rounded-md border border-white/10 px-2.5 py-1.5 text-[11px] font-semibold text-slate-300 hover:bg-white/5"
            >
              <GearIcon className="size-3" />
              Caps
            </button>
          </div>
          <div className="mt-3 grid gap-2 sm:grid-cols-3">
            <MetricCard
              dense
              label="Today"
              value={spend ? `$${spend.daily_usd.toFixed(4)}` : "—"}
              accent="blue"
            />
            <MetricCard
              dense
              label="Daily cap"
              value={spend ? `$${spend.daily_cap_usd.toFixed(2)}` : "—"}
              accent="slate"
            />
            <MetricCard
              dense
              label="Session cap"
              value={spend ? `$${spend.session_cap_usd.toFixed(2)}` : "—"}
              accent="slate"
            />
          </div>
          {spend && (
            <div className="mt-2 font-mono text-[10px] text-slate-600">
              period {spend.period_key}
            </div>
          )}
        </div>

        <Disclosure
          title="Audit findings"
          summary={`${audit.findings.length}`}
          storageKey="ade-trust-findings"
        >
          <div className="space-y-2 pt-2">
            {audit.findings.map((finding) => (
              <div
                key={finding.layer}
                className="grid grid-cols-[120px_1fr_60px] items-center gap-3 rounded-lg border border-white/6 bg-white/2 px-3 py-2.5 sm:grid-cols-[170px_1fr_70px]"
              >
                <div className="text-xs font-medium">{finding.layer}</div>
                <div className="text-xs text-slate-500">{finding.detail}</div>
                <div className="text-right text-xs text-slate-400">
                  {finding.points}/{finding.points_max}
                </div>
              </div>
            ))}
          </div>
        </Disclosure>
      </Panel>

      <AuditViewer handoffs={handoffs} />
    </div>
  );
}

function VerifyView({
  results,
  onRun,
  busy = false,
}: {
  results: VerifyResult[];
  onRun: () => void;
  busy?: boolean;
  simpleMode?: boolean;
}) {
  const [openDetails, setOpenDetails] = useState<Record<string, boolean>>({});
  const passed = results.filter(
    (result) =>
      result.passed || result.status === "unavailable" || result.status === "skipped",
  ).length;

  return (
    <Panel
      title="Checks"
      subtitle="Gate evidence after changes. Not day-to-day setup."
    >
      {results.length === 0 ? (
        <div className="py-20 text-center">
          <div className="text-sm text-slate-400">No checks run yet</div>
          <p className="mx-auto mt-2 max-w-sm text-[11px] leading-5 text-slate-600">
            Runs gates in this box. Results stay here — not in agent chat.
          </p>
          <button
            onClick={onRun}
            disabled={busy}
            className="mt-4 rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold disabled:opacity-50"
          >
            {busy ? "Checking…" : "Check work"}
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-white/7 bg-white/2 px-3 py-2 text-[11px] text-slate-400">
            <span>
              {passed}/{results.length} gates clear
            </span>
            <button
              type="button"
              onClick={onRun}
              disabled={busy}
              className="rounded-md border border-white/10 px-2 py-1 text-[10px] font-semibold text-slate-300 hover:bg-white/6 disabled:opacity-50"
            >
              {busy ? "Checking…" : "Re-run"}
            </button>
          </div>
          {results.map((result) => {
            const detailsOpen = openDetails[result.gate] ?? false;
            return (
              <div key={result.gate} className="rounded-xl border border-white/7 bg-white/2 p-4">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <span className="text-sm font-semibold">
                      {verifyGateLabel(result.gate)}
                    </span>
                    <span className="ml-2 text-[10px] uppercase tracking-wide text-slate-600">
                      {result.gate}
                    </span>
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
                      ? "Pass"
                      : result.status === "unavailable"
                        ? "Skipped"
                        : "Needs attention"}
                  </span>
                </div>
                <button
                  type="button"
                  className="mt-2 text-[11px] font-semibold text-blue-200/90 hover:text-blue-100"
                  onClick={() =>
                    setOpenDetails((current) => ({
                      ...current,
                      [result.gate]: !detailsOpen,
                    }))
                  }
                >
                  {detailsOpen ? "Hide command & output" : "Show command & output"}
                </button>
                {detailsOpen && (
                  <div className="mt-2 space-y-2">
                    <div className="font-mono text-[11px] text-slate-500">{result.command}</div>
                    {(result.stderr || result.stdout) && (
                      <pre className="thin-scrollbar max-h-44 overflow-auto whitespace-pre-wrap rounded-lg bg-black/25 p-3 text-[10px] leading-5 text-slate-500">
                        {result.stderr || result.stdout}
                      </pre>
                    )}
                  </div>
                )}
              </div>
            );
          })}
          <button
            onClick={onRun}
            disabled={busy}
            className="rounded-lg border border-white/10 px-3 py-2 text-xs font-semibold text-slate-300 hover:bg-white/5 disabled:opacity-50"
          >
            {busy ? "Checking…" : "Check work again"}
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

function SpendUsageStrip({
  compact = false,
  className = "",
}: {
  compact?: boolean;
  className?: string;
}) {
  const [spend, setSpend] = useState<{
    daily_usd: number;
    daily_cap_usd: number;
    session_cap_usd: number;
    period_key: string;
  } | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    const sessionCap = Number(
      window.localStorage.getItem("ade_session_cap_usd") || "1",
    );
    const dailyCap = Number(window.localStorage.getItem("ade_daily_cap_usd") || "10");
    let cancelled = false;
    void invoke<{
      daily_usd: number;
      daily_cap_usd: number;
      session_cap_usd: number;
      period_key: string;
    }>("spend_summary", {
      sessionCapUsd: sessionCap,
      dailyCapUsd: dailyCap,
    })
      .then((next) => {
        if (!cancelled) setSpend(next);
      })
      .catch(() => {
        if (!cancelled) setSpend(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!spend) {
    return compact ? null : (
      <div
        className={`rounded-xl border border-white/8 bg-white/2 px-3 py-2 text-[11px] text-slate-500 ${className}`}
      >
        Usage — loading…
      </div>
    );
  }

  const overDaily = spend.daily_cap_usd > 0 && spend.daily_usd >= spend.daily_cap_usd;
  if (compact) {
    return (
      <div
        className={`font-mono text-[10px] tabular-nums ${
          overDaily ? "text-amber-200" : "text-slate-500"
        } ${className}`}
        title={`period ${spend.period_key}`}
      >
        Today ${spend.daily_usd.toFixed(4)}
        {" · "}
        daily cap ${spend.daily_cap_usd.toFixed(2)}
        {" · "}
        session ${spend.session_cap_usd.toFixed(2)}
      </div>
    );
  }

  return (
    <div
      className={`rounded-xl border border-white/8 bg-white/2 px-4 py-3 ${className}`}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
            Usage
          </div>
          <div className="mt-0.5 text-[12px] text-slate-400">
            Workspace daily spend vs hard caps (same ledger as Trust)
          </div>
        </div>
        <div
          className={`font-mono text-[11px] tabular-nums ${
            overDaily ? "text-amber-200" : "text-slate-300"
          }`}
        >
          Today ${spend.daily_usd.toFixed(4)}
        </div>
      </div>
      <div className="mt-2 flex flex-wrap gap-3 font-mono text-[10px] text-slate-500">
        <span>daily cap ${spend.daily_cap_usd.toFixed(2)}</span>
        <span>session cap ${spend.session_cap_usd.toFixed(2)}</span>
        <span>period {spend.period_key}</span>
      </div>
    </div>
  );
}

function ContextUsageButton({
  contextPct,
  contextUsed,
  contextLimit,
  turnTokens,
  sessionTokens,
  showSpend,
  effort,
  showEffort,
  onEffort,
  onSaveGoal,
  goalBusy,
}: {
  contextPct: number;
  contextUsed: number;
  contextLimit: number;
  turnTokens: { input: number; output: number };
  sessionTokens: { input: number; output: number; costMicros: number };
  showSpend: boolean;
  effort: EffortLevel;
  showEffort: boolean;
  onEffort: (effort: EffortLevel) => void;
  onSaveGoal?: () => void;
  goalBusy?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [panelStyle, setPanelStyle] = useState<CSSProperties>({});
  const rootRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (!open || !rootRef.current) return;
    const place = () => {
      const rect = rootRef.current!.getBoundingClientRect();
      const width = 280;
      const pad = 8;
      const sidebarSafe = 248;
      let left = rect.right - width;
      left = Math.max(sidebarSafe, Math.min(left, window.innerWidth - pad - width));
      if (left < pad) left = pad;
      setPanelStyle({
        position: "fixed",
        left,
        width,
        bottom: window.innerHeight - rect.top + 8,
        zIndex: 100,
      });
    };
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onPointer = (event: MouseEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target) || panelRef.current?.contains(target)) {
        return;
      }
      setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("click", onPointer, true);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onPointer, true);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="relative shrink-0">
      <button
        type="button"
        title="Context & usage"
        aria-expanded={open}
        onClick={(event) => {
          event.stopPropagation();
          setOpen((value) => !value);
        }}
        className={`rounded-md px-1.5 py-1 text-[10px] font-semibold tabular-nums ${
          contextPct >= 85
            ? "text-amber-200 hover:bg-amber-400/10"
            : "text-slate-500 hover:bg-white/5 hover:text-slate-300"
        }`}
      >
        {contextPct}%
      </button>
      {open && (
        <div
          ref={panelRef}
          style={panelStyle}
          className="rounded-xl border border-white/12 bg-[#121820] p-3"
          onClick={(event) => event.stopPropagation()}
        >
          {showEffort && (
            <div className="mb-3">
              <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-slate-600">
                Effort · turn gas tank
              </div>
              <p className="mb-1.5 text-[10px] leading-4 text-slate-500">
                Tool rounds + output tokens for this turn (not model smartness).
                Apply/Automate floor at Med.
              </p>
              <DarkSelect
                ariaLabel="Effort turn budget"
                value={effort}
                options={EFFORT_OPTIONS.map((item) => ({
                  value: item.id,
                  label: item.label,
                }))}
                maxLabelChars={12}
                onChange={(next) => onEffort(next as EffortLevel)}
              />
            </div>
          )}
          <div className="text-[11px] font-semibold text-slate-200">Context usage</div>
          <div className="mt-1 text-[10px] text-slate-500">
            {formatTokenCount(contextUsed)} / {formatTokenCount(contextLimit)}
          </div>
          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-800">
            <div
              className={`h-full rounded-full ${
                contextPct >= 85 ? "bg-amber-400/80" : "bg-cyan-400/80"
              }`}
              style={{ width: `${contextPct}%` }}
            />
          </div>
          <div className="mt-2 space-y-1">
            <ContextUsageRow label="Last turn input" tokens={turnTokens.input} tone="cyan" />
            <ContextUsageRow label="Last turn output" tokens={turnTokens.output} tone="violet" />
            <ContextUsageRow label="Session input" tokens={sessionTokens.input} tone="emerald" />
            <ContextUsageRow label="Session output" tokens={sessionTokens.output} tone="slate" />
          </div>
          {showSpend && (
            <div className="mt-2 text-[10px] text-slate-500">
              Session spend ≈ ${(sessionTokens.costMicros / 1_000_000).toFixed(4)}
            </div>
          )}
          {onSaveGoal && (
            <button
              type="button"
              disabled={goalBusy}
              onClick={() => {
                onSaveGoal();
                setOpen(false);
              }}
              className="mt-3 w-full rounded-md border border-white/10 px-2 py-1.5 text-[11px] font-semibold text-emerald-200/90 hover:bg-white/5 disabled:opacity-40"
            >
              Save as eng-goal
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function formatTokenCount(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(tokens >= 10_000 ? 0 : 1)}K`;
  return String(tokens);
}

function ContextUsageRow({
  label,
  tokens,
  tone,
}: {
  label: string;
  tokens: number;
  tone: "cyan" | "violet" | "emerald" | "slate";
}) {
  const dot =
    tone === "cyan"
      ? "bg-cyan-400"
      : tone === "violet"
        ? "bg-violet-400"
        : tone === "emerald"
          ? "bg-emerald-400"
          : "bg-slate-400";
  return (
    <div className="flex items-center justify-between gap-2 text-[11px]">
      <div className="flex min-w-0 items-center gap-2 text-slate-400">
        <span className={`size-1.5 shrink-0 rounded-full ${dot}`} />
        <span className="truncate">{label}</span>
      </div>
      <span className="shrink-0 tabular-nums text-slate-300">
        {formatTokenCount(tokens)}
      </span>
    </div>
  );
}

function FindingBar({ finding }: { finding: Finding }) {
  const percent = Math.round((finding.points / finding.points_max) * 100);
  const incomplete = finding.points < finding.points_max;
  const barClass =
    finding.severity === "error"
      ? "bg-red-400/80"
      : finding.severity === "warn" || incomplete
        ? "bg-amber-400/70"
        : finding.severity === "info"
          ? "bg-slate-400/70"
          : "bg-blue-400/80";
  return (
    <div>
      <div className="mb-0.5 flex items-center justify-between gap-2 text-[10px]">
        <span className="min-w-0 truncate text-slate-400">{finding.layer}</span>
        <span className="shrink-0 tabular-nums text-slate-600">
          {finding.points}/{finding.points_max}
        </span>
      </div>
      <div className="h-1 overflow-hidden rounded-full bg-slate-800">
        <div className={`h-full rounded-full ${barClass}`} style={{ width: `${percent}%` }} />
      </div>
      <div className="mt-0.5 truncate text-[10px] text-slate-600" title={finding.detail}>
        <span className="uppercase tracking-wide text-slate-500">{finding.severity}</span>
        {" · "}
        {finding.detail}
      </div>
    </div>
  );
}

function StatusPill({ severity }: { severity: string }) {
  const styles =
    severity === "ok"
      ? "bg-emerald-400/8 text-emerald-300"
      : severity === "info"
        ? "bg-slate-400/10 text-slate-300"
        : severity === "error"
          ? "bg-red-400/10 text-red-300"
          : "bg-amber-400/8 text-amber-300";
  return (
    <span
      className={`shrink-0 rounded-full px-2 py-1 text-[9px] font-semibold uppercase tracking-wider ${styles}`}
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
