import { Channel } from "@tauri-apps/api/core";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type DragEvent, type ClipboardEvent, type ReactNode } from "react";
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
import { AnalyticsView } from "./components/AnalyticsView";
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
import { KeysView } from "./components/KeysView";
import { WorkspacesView, type WorkspaceOpenIntent } from "./components/WorkspacesView";
import {
  SidebarRailLists,
  fetchWorkspaceList,
  type WorkspaceListSnapshot,
} from "./components/SidebarRailLists";
import { DarkSelect, GearIcon } from "./components/DarkSelect";
import { HeaderOverflowMenu } from "./components/HeaderPlusMenu";
import { ShellTabBar } from "./components/ShellTabBar";
import {
  defaultBrowserUrl,
  leafName,
  newTabId,
  viewForTabKind,
  agentTabTitleFromPrompt,
  type ShellTab,
} from "./shellTabs";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { AuditViewer } from "./components/AuditViewer";
import {
  AgentSessionStrip,
  honestLeaseError,
  readOrCreateAgentId,
  rotateAgentId,
  rotateSessionId,
  writableConflict,
  type PathLease as StripLease,
} from "./components/AgentSessionStrip";
import {
  DesktopRequired,
} from "./components/DesktopRequired";
import { ProviderSelect } from "./components/ModelPicker";
import { ComposerModelSelect } from "./components/ComposerModelSelect";
import { IntegrationsView } from "./components/IntegrationsView";
import { GettingStartedChecklist } from "./components/GettingStartedChecklist";
import { buildGettingStartedSteps } from "./components/gettingStarted";
import { BrandWell } from "./components/IntegrationIcons";
import {
  Chip,
  Disclosure,
  MetricCard,
  Panel,
  SubTabs,
} from "./components/ui";
import { AttachmentChips } from "./components/AttachmentChips";
import { MentionPalette, type MentionItem } from "./components/MentionPalette";
import {
  type ChatAttachment,
  type ChatAttachmentMeta,
  looksLikeFilesystemPath,
  packagePromptWithAttachments,
  toAttachmentMeta,
} from "./components/fileKind";
import {
  fetchUrlAttachment,
  ingestFiles,
  ingestPathText,
  ingestUrlText,
  openChatPath,
  pickAttachmentFiles,
  pickAttachmentFolder,
  pickAttachmentFilesViaInput,
} from "./components/attachIngest";
import { looksLikeHttpUrl } from "./components/urlAttach";
import { DESKTOP_REQUIRED_VIEWS } from "./capabilities";
import { usd } from "./format";
import {
  DEFAULT_BASE_URL,
  DEFAULT_MODEL,
  DEFAULT_PROVIDER,
  PROVIDER_PRESETS,
  autoModelForSlot,
  canonicalBaseUrl,
  firstModelId,
  modelContextWindow,
  modelSupportsVision,
  slotFromAutonomy,
} from "./providers";

const DEV_MODE_KEY = "ade_dev_mode";
const AUTONOMY_KEY = "ade_autonomy_level";
const FORCE_OWNED_KEY = "ade_force_owned_paths";
const SHELL_SCOPE_KEY = "ade_agent_shell_scope";
const APPLY_ISOLATE_KEY = "ade_apply_isolate_worktree";
const SURFACE_MODE_KEY = "ade_surface_mode";
const AGENT_PROVIDER_KEY = "ade_agent_provider";
const AGENT_BASE_URL_KEY = "ade_agent_base_url";
const AGENT_MODEL_KEY = "ade_agent_model";
const AGENT_MODEL_MODE_KEY = "ade_agent_model_mode";
const AGENT_CONTEXT_KEY = "ade_agent_context_window";
const AGENT_EFFORT_KEY = "ade_agent_effort";
const NAV_OPEN_KEY = "ade_nav_open";
type ModelMode = "auto" | "pin";

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
  outOfScope?: string[];
  shellScope: string;
  autonomy: string;
  verifyGate?: string | null;
  ownedPaths: string[];
  status: string;
  lastHandoffId?: string | null;
  contractWaive?: { at: string; reason: string } | null;
  clarifyResolutions?: string[];
};

function isGoalContractReady(goal: EngGoal | null | undefined): boolean {
  if (!goal || goal.status !== "active") return false;
  if (goal.contractWaive?.reason?.trim()) return true;
  const clarify = (goal.clarifyResolutions ?? []).filter((c) => c.trim());
  if (clarify.length >= 1 && clarify.length <= 3) return true;
  const ac = (goal.successCriteria ?? []).some((c) => c.trim());
  const oos = (goal.outOfScope ?? []).some((c) => c.trim());
  const verify = Boolean(goal.verifyGate?.trim());
  return ac && oos && verify;
}

function splitListInput(raw: string | null): string[] {
  if (!raw) return [];
  return raw
    .split(/[\n;]+/)
    .map((s) => s.trim())
    .filter(Boolean)
    .slice(0, 12);
}

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

/** Simple (`guided`) is parked — Standard is the product; Debug adds maps/harness. */
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
  if (!path) return "No folder attached";
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function readForceOwnedPaths(): string[] | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(FORCE_OWNED_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return null;
    const paths = parsed
      .filter((item): item is string => typeof item === "string")
      .map((item) => item.trim())
      .filter(Boolean);
    return paths.length > 0 ? paths : null;
  } catch {
    return null;
  }
}

function writeForceOwnedPaths(paths: string[] | null) {
  if (typeof window === "undefined") return;
  if (!paths || paths.length === 0) {
    window.localStorage.removeItem(FORCE_OWNED_KEY);
    return;
  }
  window.localStorage.setItem(FORCE_OWNED_KEY, JSON.stringify(paths));
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

type NavItem = {
  id: string;
  label: string;
  icon: string;
  desktopOnly?: boolean;
  /** Optional status light for first-run / setup guidance. */
  setupKey?: "environment" | "keys" | "integrations" | "recipes" | "verify";
};

/** 0 = daily work, 1 = setup / context, 2 = rare configure / debug density */
type NavTier = 0 | 1 | 2;

type NavGroup = { title?: string; tier: NavTier; items: NavItem[] };

type SetupLight = "ready" | "todo" | "warn";

/**
 * Usage-ranked rail:
 * Tier 0 — Home (+ Workplaces / Sessions lists under Home).
 * Tier 1 — Setup (first-run) + Trust.
 * Tier 2 — Guidance (and Debug-only tools).
 */
const navGroups: NavGroup[] = [
  {
    tier: 0,
    items: [{ id: "Home", label: "Home", icon: "⌂" }],
  },
  {
    tier: 1,
    title: "Setup",
    items: [
      {
        id: "Environment",
        label: "Environment",
        icon: "◎",
        setupKey: "environment",
      },
      { id: "Keys", label: "Keys", icon: "◈", desktopOnly: true, setupKey: "keys" },
      {
        id: "Integrations",
        label: "Integrations",
        icon: "⧉",
        desktopOnly: true,
        setupKey: "integrations",
      },
      { id: "Recipes", label: "Stack", icon: "▦", setupKey: "recipes" },
      { id: "Verify", label: "Test project", icon: "✓", setupKey: "verify" },
    ],
  },
  {
    tier: 1,
    items: [{ id: "Insight", label: "Insight", icon: "◉" }],
  },
  {
    tier: 2,
    title: "More",
    items: [
      { id: "Rules", label: "Guidance", icon: "☰" },
      { id: "MCP", label: "MCP", icon: "⬡", desktopOnly: true },
    ],
  },
];

/**
 * Insight = the four "looking" surfaces behind one nav destination.
 * Trust and Analytics ship on Standard; the maps stay Debug density.
 */
type InsightSectionId = "Audit" | "Analytics" | "Plan" | "Atlas";

const INSIGHT_SECTIONS: {
  id: InsightSectionId;
  label: string;
  hint: string;
  debugOnly?: boolean;
}[] = [
  {
    id: "Audit",
    label: "Trust",
    hint: "What happened, and is it safe — drift, risk, envelopes, audit log",
  },
  {
    id: "Analytics",
    label: "Analytics",
    hint: "What it cost and whether it worked — trend, attribution, reserve Δ",
  },
  {
    id: "Plan",
    label: "Plan Map",
    hint: "What is planned, in what order, gated by what",
    debugOnly: true,
  },
  {
    id: "Atlas",
    label: "Atlas",
    hint: "How authority relates to work",
    debugOnly: true,
  },
];

const INSIGHT_IDS = new Set<string>(INSIGHT_SECTIONS.map((section) => section.id));
const INSIGHT_STORAGE_KEY = "ade_insight_section";

function setupLightClass(tone: SetupLight): string {
  if (tone === "ready") return "bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.55)]";
  if (tone === "warn") return "bg-amber-300 shadow-[0_0_6px_rgba(252,211,77,0.55)]";
  return "bg-amber-400/90 shadow-[0_0_6px_rgba(251,191,36,0.45)]";
}

function setupLightTitle(tone: SetupLight): string {
  if (tone === "ready") return "Ready";
  if (tone === "warn") return "Needs attention";
  return "Recommended before first run";
}

function navItemClass(active: boolean, tier: NavTier): string {
  // One size for all rail destinations — hierarchy comes from weight/color, not micro-type.
  const weight = tier === 0 ? "text-[13px] font-medium" : "text-[13px] font-normal";
  if (active) {
    return `flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition bg-blue-500/12 text-blue-200 ${weight}`;
  }
  if (tier === 2) {
    return `flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition text-slate-500 hover:bg-white/4 hover:text-slate-300 ${weight}`;
  }
  return `flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left transition text-slate-400 hover:bg-white/4 hover:text-slate-200 ${weight}`;
}

/** Lightweight sidebar fold — sentence-case labels, same size as nav items. */
function NavFold({
  title,
  summary,
  storageKey,
  defaultOpen = false,
  forceOpen = false,
  light,
  children,
}: {
  title: string;
  summary?: string;
  storageKey: string;
  defaultOpen?: boolean;
  forceOpen?: boolean;
  light?: SetupLight | null;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(() => {
    if (forceOpen) return true;
    if (typeof window === "undefined") return defaultOpen;
    const stored = window.localStorage.getItem(storageKey);
    if (stored === "1") return true;
    if (stored === "0") return false;
    return defaultOpen;
  });

  useEffect(() => {
    if (forceOpen) setOpen(true);
  }, [forceOpen]);

  useEffect(() => {
    window.localStorage.setItem(storageKey, open ? "1" : "0");
  }, [open, storageKey]);

  const expanded = forceOpen || open;

  return (
    <div className="space-y-0.5">
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setOpen((value) => !value)}
        className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[13px] font-medium text-slate-500 transition hover:bg-white/4 hover:text-slate-300"
        data-testid={`ade-nav-fold-${title.toLowerCase()}`}
      >
        <span className="min-w-0 flex-1 truncate">{title}</span>
        {light && light !== "ready" && (
          <span
            className={`size-1.5 shrink-0 rounded-full ${setupLightClass(light)}`}
            title={setupLightTitle(light)}
          />
        )}
        {!expanded && summary && (
          <span className="max-w-[5.5rem] truncate text-[12px] font-normal text-slate-600">
            {summary}
          </span>
        )}
        <span className="shrink-0 text-[10px] text-slate-600" aria-hidden>
          {expanded ? "▴" : "▾"}
        </span>
      </button>
      {expanded ? <div className="space-y-px pl-0.5">{children}</div> : null}
    </div>
  );
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
  is_default?: boolean;
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
  | { type: "started"; session_id: string; provider: string; model: string; profile_id?: string; route_reason?: string; slot?: string }
  | { type: "text_delta"; text: string }
  | {
      type: "tool_call";
      server: string;
      tool: string;
      arguments: unknown;
      effect?: string;
      envelope?: {
        effect: string;
        paths?: string[];
        autonomy: string;
        risk_tier?: string;
        risk_category?: string;
      };
    }
  | { type: "tool_result"; server: string; tool: string; is_error: boolean; text: string }
  | {
      type: "context_compacted";
      trigger: string;
      tokens_before: number;
      tokens_after: number;
      occupancy_before: number;
    }
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
  | {
      type: "host_intent";
      action: string;
      path?: string;
      url?: string;
    }
  | { type: "completed"; result: AgentTurnResult }
  | { type: "failed"; error: string }
  | { type: "cancelled"; reason: string };

type ProviderKeyStatus = {
  profile: string;
  provider: string;
  configured: boolean;
};

const HOME_AGENT_TAB = "agent-home";

function App() {
  const [dashboard, setDashboard] = useState<DashboardSnapshot | null>(null);
  const [activeView, setActiveView] = useState("Home");
  const [shellTabs, setShellTabs] = useState<ShellTab[]>(() => [
    {
      id: HOME_AGENT_TAB,
      kind: "agent",
      title: typeof window !== "undefined" && !isTauri() ? "Home" : "Agent",
      closable: false,
      ephemeral: false,
    },
  ]);
  const [activeTabId, setActiveTabId] = useState<string | null>(HOME_AGENT_TAB);
  const [runningAgentTabId, setRunningAgentTabId] = useState<string | null>(null);
  /** When true, Workspaces opens with the New workspace form expanded. */
  const [workspacesStartNew, setWorkspacesStartNew] = useState(false);
  const [workspaceList, setWorkspaceList] = useState<WorkspaceListSnapshot | null>(
    null,
  );
  const [workspaceSwitchBusy, setWorkspaceSwitchBusy] = useState(false);
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
  const [continuityBusy, setContinuityBusy] = useState(false);
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

  const focusShellTab = useCallback(
    (id: string) => {
      const tab = shellTabs.find((row) => row.id === id);
      if (!tab) return;
      setActiveTabId(id);
      setActiveView(viewForTabKind(tab.kind));
    },
    [shellTabs],
  );

  const updateTabTitle = useCallback((id: string, title: string) => {
    setShellTabs((tabs) =>
      tabs.map((tab) => (tab.id === id ? { ...tab, title } : tab)),
    );
  }, []);

  const closeShellTab = useCallback(
    (id: string) => {
      setShellTabs((tabs) => {
        const target = tabs.find((tab) => tab.id === id);
        if (!target?.closable) return tabs;
        const next = tabs.filter((tab) => tab.id !== id);
        setActiveTabId((current) => {
          if (current !== id) return current;
          const fallback = next[next.length - 1] ?? null;
          if (fallback) {
            setActiveView(viewForTabKind(fallback.kind));
            return fallback.id;
          }
          setActiveView("Home");
          return null;
        });
        return next;
      });
    },
    [],
  );

  const openAgentTab = useCallback(() => {
    const id = newTabId("agent");
    const tab: ShellTab = {
      id,
      kind: "agent",
      title: "New Agent",
      closable: true,
      ephemeral: true,
    };
    setShellTabs((tabs) => [...tabs, tab]);
    setActiveTabId(id);
    setActiveView("Home");
  }, []);

  const openBrowserTab = useCallback((initialUrl?: string) => {
    const id = newTabId("browser");
    const url = (initialUrl?.trim() || defaultBrowserUrl()).trim();
    const tab: ShellTab = {
      id,
      kind: "browser",
      title: "Browser",
      closable: true,
      url,
    };
    setShellTabs((tabs) => [...tabs, tab]);
    setActiveTabId(id);
    setActiveView("Browser");
  }, []);

  const openEditorTab = useCallback(async (path?: string) => {
    let filePath = path?.trim() || "";
    if (!filePath && isTauri()) {
      const selected = await openDialog({
        multiple: false,
        directory: false,
        title: "Open file",
      });
      if (!selected || Array.isArray(selected)) return;
      filePath = selected;
    }
    // Editor lives under Debug nav — enable it so Continuity/header deep links stick.
    setSurfaceModePersisted("dev");
    const id = newTabId("editor");
    const tab: ShellTab = {
      id,
      kind: "editor",
      title: filePath ? leafName(filePath) : "Editor",
      closable: true,
      path: filePath || undefined,
    };
    setShellTabs((tabs) => [...tabs, tab]);
    setActiveTabId(id);
    setActiveView("Editor");
  }, []);

  const openTerminalTab = useCallback(() => {
    setShellTabs((tabs) => {
      const existing = tabs.find((tab) => tab.kind === "terminal");
      if (existing) {
        setActiveTabId(existing.id);
        setActiveView("Terminal");
        return tabs;
      }
      const id = newTabId("terminal");
      setActiveTabId(id);
      setActiveView("Terminal");
      return [
        ...tabs,
        {
          id,
          kind: "terminal" as const,
          title: "Terminal",
          closable: true,
        },
      ];
    });
  }, []);

  const openNavView = useCallback((viewId: string) => {
    if (viewId === "Home" || viewId === "Agent") {
      setActiveTabId(HOME_AGENT_TAB);
      setActiveView("Home");
      setShellTabs((tabs) =>
        tabs.some((tab) => tab.id === HOME_AGENT_TAB)
          ? tabs
          : [
              {
                id: HOME_AGENT_TAB,
                kind: "agent",
                title: "Agent",
                closable: false,
                ephemeral: false,
              },
              ...tabs,
            ],
      );
      return;
    }
    if (viewId === "Browser") {
      setShellTabs((tabs) => {
        const existing = [...tabs].reverse().find((tab) => tab.kind === "browser");
        if (existing) {
          setActiveTabId(existing.id);
          setActiveView("Browser");
          return tabs;
        }
        const id = newTabId("browser");
        setActiveTabId(id);
        setActiveView("Browser");
        return [
          ...tabs,
          {
            id,
            kind: "browser" as const,
            title: "Browser",
            closable: true,
            url: defaultBrowserUrl(),
          },
        ];
      });
      return;
    }
    if (viewId === "Editor") {
      const existing = [...shellTabs]
        .reverse()
        .find((tab) => tab.kind === "editor");
      if (existing) {
        setActiveTabId(existing.id);
        setActiveView("Editor");
      } else {
        void openEditorTab();
      }
      return;
    }
    if (viewId === "Terminal") {
      openTerminalTab();
      return;
    }
    if (viewId === "Insight") {
      const remembered = window.localStorage.getItem(INSIGHT_STORAGE_KEY);
      setActiveTabId(null);
      setActiveView(
        remembered && INSIGHT_IDS.has(remembered) ? remembered : "Audit",
      );
      return;
    }
    setActiveTabId(null);
    setActiveView(viewId);
  }, [openEditorTab, openTerminalTab, shellTabs]);

  /** Insight sub-tab switch — remembered so the rail returns you where you left. */
  const openInsightSection = useCallback((section: string) => {
    window.localStorage.setItem(INSIGHT_STORAGE_KEY, section);
    setActiveTabId(null);
    setActiveView(section);
  }, []);

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

  /**
   * Debug-only nav: editor / MCP — Terminal + Browser stay on Standard for
   * Default dogfood. Atlas and Plan Map are Insight sub-tabs now: Debug shows
   * them by default, and a deep link may still open one on Standard rather than
   * bouncing the user to Home.
   */
  const DEBUG_NAV_IDS = useMemo(() => new Set(["MCP", "Editor"]), []);

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

  /** Map sub-tabs appear on Debug, or on Standard once a deep link opened one. */
  const insightTabs = useMemo(
    () =>
      INSIGHT_SECTIONS.filter(
        (section) => !section.debugOnly || debugChrome || activeView === section.id,
      ).map((section) => ({
        id: section.id,
        label: section.label,
        hint: section.hint,
      })),
    [debugChrome, activeView],
  );

  useEffect(() => {
    if (surfaceMode !== "dev" && DEBUG_NAV_IDS.has(activeView)) {
      setActiveView("Home");
    }
  }, [DEBUG_NAV_IDS, activeView, surfaceMode]);

  /** Home must always show an agent session — never an empty main pane. */
  useEffect(() => {
    if (activeView !== "Home" && activeView !== "Agent") return;
    const agentTabs = shellTabs.filter((tab) => tab.kind === "agent");
    if (agentTabs.length === 0) {
      setShellTabs((tabs) => [
        {
          id: HOME_AGENT_TAB,
          kind: "agent",
          title: "Agent",
          closable: false,
          ephemeral: false,
        },
        ...tabs,
      ]);
      setActiveTabId(HOME_AGENT_TAB);
      return;
    }
    if (!agentTabs.some((tab) => tab.id === activeTabId)) {
      setActiveTabId(HOME_AGENT_TAB);
    }
  }, [activeView, activeTabId, shellTabs]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    setBrowserApiProbeKey((key) => key + 1);
    try {
      const [snapshot, wins, workspaces] = await Promise.all([
        invoke<DashboardSnapshot>("get_dashboard"),
        invoke<GuidedWinsState>("guided_wins_status").catch(
          (): GuidedWinsState => ({
            understand: false,
            verify: false,
            improve_ade: false,
            understand_artifact: null,
          }),
        ),
        fetchWorkspaceList().catch(() => null),
      ]);
      setDashboard({ ...snapshot, tasks: snapshot.tasks ?? [] });
      if (snapshot.last_verify && snapshot.last_verify.length > 0) {
        setVerifyResults(snapshot.last_verify);
      }
      setGuidedWins(wins);
      if (wins.understand_artifact) {
        setLastUnderstandPath(wins.understand_artifact);
      }
      setWorkspaceList(workspaces);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  }, []);

  const switchWorkspaceFromRail = useCallback(
    async (path: string) => {
      if (!isTauri() || workspaceSwitchBusy) return;
      setWorkspaceSwitchBusy(true);
      setError(null);
      try {
        await invoke("open_workspace", { path });
        await refresh();
        setActiveView("Home");
        setActiveTabId(HOME_AGENT_TAB);
        setNavOpenPersisted(false);
      } catch (reason) {
        setError(String(reason));
      } finally {
        setWorkspaceSwitchBusy(false);
      }
    },
    [refresh, workspaceSwitchBusy],
  );

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
    if (activeView === "MCP" || activeView === "Integrations") {
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

  const verifyFailed = useMemo(
    () =>
      verifyResults.some(
        (result) =>
          !result.passed &&
          result.status !== "unavailable" &&
          result.status !== "skipped",
      ),
    [verifyResults],
  );

  const setupLights = useMemo(() => {
    const keys: SetupLight = dashboard?.has_provider_key ? "ready" : "todo";
    const recipes: SetupLight = dashboard?.has_recipe ? "ready" : "todo";
    const verify: SetupLight = verifyFailed
      ? "warn"
      : verifyResults.length > 0
        ? "ready"
        : "todo";
    // Honesty: Ready only when an MCP session is live (vault token alone ≠ connected).
    // Does not gate first-run overall.
    const integrations: SetupLight =
      mcpServers.length > 0 ? "ready" : "todo";
    const blockers = (dashboard?.audit.blockers.length ?? 0) > 0;
    const environment: SetupLight = blockers
      ? "warn"
      : keys === "ready" && recipes === "ready" && verify !== "todo"
        ? "ready"
        : "todo";
    const overall: SetupLight =
      environment === "warn" || verify === "warn"
        ? "warn"
        : environment === "todo" || keys === "todo" || recipes === "todo" || verify === "todo"
          ? "todo"
          : "ready";
    return { overall, environment, keys, integrations, recipes, verify } as const;
  }, [dashboard, mcpServers.length, verifyFailed, verifyResults.length]);

  const lightForSetupKey = (key: NavItem["setupKey"]): SetupLight | null => {
    if (!key) return null;
    return setupLights[key];
  };

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
    setContinuityBusy(true);
    try {
      const resume = await invoke<{
        available: boolean;
        resumePrompt: string;
        goal: string;
        nextSafeCommand: string;
        turnStatus?: string | null;
        hostRanNext?: boolean;
        hostExitCode?: number | null;
      }>("handoff_resume", { id: id ?? null, hostRunNext: true });
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
    } finally {
      setContinuityBusy(false);
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
    if (!isTauri()) {
      setError(
        "Approve & execute requires ADE Desktop (no EXECUTE over HTTP by design).",
      );
      return;
    }
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
    vaultProvider?: string | null;
    vaultEnvKeys?: string[];
  }) => {
    setMcpBusy(true);
    setError(null);
    try {
      await invoke("mcp_connect", {
        name: input.name,
        command: input.command,
        args: input.args,
        approved: input.approved,
        vaultProvider: input.vaultProvider ?? null,
        vaultEnvKeys: input.vaultEnvKeys ?? [],
      });
      await refreshMcp();
    } catch (reason) {
      setError(String(reason));
      throw reason;
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

  const cancelAgentTurn = async () => {
    if (!isTauri()) return;
    try {
      await invoke<boolean>("cancel_agent_turn");
    } catch (reason) {
      setError(String(reason));
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
    approvedRiskCategories?: string[];
    approvedRiskTiers?: string[];
    allowUnpriced?: boolean;
    claimedTaskId?: string | null;
    waiveQueue?: boolean;
    slotOverride?: string | null;
    imagePaths?: string[];
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

    const noteHeartbeat = (message: string) => {
      setError(message);
    };

    const mutating = input.autonomy === "act" || input.autonomy === "automate";
    const isVerifier = (input.slotOverride ?? "").toLowerCase() === "verifier";
    const agentId = mutating && !isVerifier ? input.leaseAgentId : null;
    const acquiredIds: string[] = [];
    let heartbeat: ReturnType<typeof setInterval> | null = null;
    const leaseTtlSecs = 300;
    const ratesZero =
      (Number(input.inputCostPerMtok) || 0) <= 0 &&
      (Number(input.outputCostPerMtok) || 0) <= 0;
    const capsOn =
      (Number(input.sessionCapUsd) || 0) > 0 || (Number(input.dailyCapUsd) || 0) > 0;
    // $0 rates + caps: always continue unmetered. Caps cannot reserve dollars
    // without rates; blocking free models behind invoice jargon is a false wall.
    let allowUnpriced = Boolean(input.allowUnpriced);
    if (ratesZero && capsOn) {
      allowUnpriced = true;
      window.localStorage.setItem("ade_allow_unpriced", "1");
    }

    let waiveQueue = Boolean(input.waiveQueue);
    if (mutating && !isVerifier && !input.claimedTaskId && !waiveQueue) {
      const readyQueued = (dashboard?.tasks ?? []).filter(
        (task) => task.status === "queued",
      ).length;
      if (readyQueued > 0) {
        const continueFreeform = window.confirm(
          `claim_gate: ${readyQueued} queued task(s).\n\nOK = waive queue and Apply free-form\nCancel = stop (use Apply next from the queue)`,
        );
        if (!continueFreeform) {
          failTurn(
            `claim_gate: ${readyQueued} queued task(s) — Apply next or waive`,
          );
          setAgentBusy(false);
          return;
        }
        waiveQueue = true;
      }
    }

    try {
      const claimedTaskId = input.claimedTaskId?.trim() || null;
      if (claimedTaskId && agentId) {
        const intervalMs = Math.max(1_000, Math.floor((leaseTtlSecs * 1000) / 3));
        heartbeat = setInterval(() => {
          void invoke("task_heartbeat", {
            taskId: claimedTaskId,
            agentId,
            ttlSecs: leaseTtlSecs,
          }).catch((reason) => {
            noteHeartbeat(`heartbeat_failed: ${String(reason)}`);
          });
        }, intervalMs);
      } else if (agentId && input.approveOwnedPaths && input.ownedPaths.length > 0) {
        const conflict = writableConflict(
          (dashboard?.leases ?? []) as StripLease[],
          agentId,
          input.ownedPaths,
        );
        if (conflict) {
          failTurn(
            `lease conflict: another agent (${conflict.agent_id.slice(0, 8)}) holds a write lease on ${conflict.path}. Suggest-only until it finishes or expires.`,
          );
          return;
        }
        for (const path of input.ownedPaths) {
          try {
            const lease = await invoke<{ id: string }>("lease_acquire", {
              agentId,
              path,
              mode: "strong",
              ttlSecs: leaseTtlSecs,
              autonomy: input.autonomy,
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
          const intervalMs = Math.max(1_000, Math.floor((leaseTtlSecs * 1000) / 3));
          heartbeat = setInterval(() => {
            for (const leaseId of acquiredIds) {
              void invoke("lease_renew", {
                agentId,
                leaseId,
                ttlSecs: leaseTtlSecs,
              }).catch((reason) => {
                noteHeartbeat(`heartbeat_failed: ${String(reason)}`);
              });
            }
          }, intervalMs);
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
        if (event.type === "host_intent" && isTauri()) {
          void (async () => {
            try {
              if (event.action === "attach_workspace" && event.path?.trim()) {
                await invoke("open_workspace", { path: event.path.trim() });
                window.localStorage.setItem(AUTONOMY_KEY, "act");
                writeForceOwnedPaths(["."]);
                try {
                  const goal = await invoke<EngGoal>("goal_create", {
                    input: {
                      statement: `Greenfield work in newly attached workspace`,
                      successCriteria: [
                        "Scaffold and iterate project files under this workspace",
                      ],
                      outOfScope: [
                        "Deleting other ADE workspaces",
                        "Editing ADE source outside this folder",
                      ],
                      shellScope: "workspace",
                      autonomy: "act",
                      verifyGate: "G0",
                      ownedPaths: ["."],
                      activate: true,
                    },
                  });
                  if (goal?.id) {
                    await invoke("goal_waive_contract", {
                      id: goal.id,
                      reason: "bootstrap after workspace__create_named",
                    });
                  }
                } catch {
                  // Goal bootstrap is best-effort; Apply + force owned still help.
                }
                await refresh();
                setActiveView("Home");
                setActiveTabId(HOME_AGENT_TAB);
              } else if (event.action === "open_browser" && event.url?.trim()) {
                openBrowserTab(event.url.trim());
              }
            } catch (reason) {
              setError(String(reason));
            }
          })();
        }
      };
      // Bundle scalars under `args` so the Channel stays a sibling IPC value —
      // flat 28-arg invokes can mis-bind Channel maps onto bool fields like allowUnpriced.
      await invoke("run_agent_turn", {
        args: {
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
          autonomy: isVerifier ? "propose" : input.autonomy,
          maxSteps: input.maxSteps,
          maxTokens: input.maxTokens,
          verifyOnComplete: input.verifyOnComplete,
          verifyGate: input.verifyGate,
          approveOwnedPaths: isVerifier ? false : input.approveOwnedPaths,
          ownedPaths: isVerifier ? [] : input.ownedPaths,
          preferredShellCwd: input.preferredShellCwd,
          executionRoot: input.executionRoot,
          allowUnpriced: allowUnpriced === true,
          approvedRiskCategories: input.approvedRiskCategories ?? [],
          approvedRiskTiers: input.approvedRiskTiers ?? [],
          claimedTaskId,
          waiveQueue: waiveQueue === true,
          slotOverride: input.slotOverride ?? null,
          imagePaths: input.imagePaths ?? [],
        },
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
      setRunningAgentTabId(null);
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
    if (!isTauri()) {
      setError(
        "Initialize recipe writes files via Desktop. Use Preview in browser, then open ADE Desktop to apply.",
      );
      return;
    }
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
        data-testid="ade-sidebar"
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

        <nav className="thin-scrollbar min-h-0 flex-1 space-y-3 overflow-y-auto">
          {visibleNav.map((group, groupIndex) => {
            const renderItems = (opts?: { showLights?: boolean }) =>
              group.items.map((item) => {
                const needsDesktop = Boolean(item.desktopOnly) && !isTauri();
                const light = lightForSetupKey(item.setupKey);
                const showLights = opts?.showLights === true;
                return (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => {
                      openNavView(item.id);
                      setNavOpenPersisted(false);
                    }}
                    className={navItemClass(
                      activeView === item.id ||
                        (item.id === "Insight" && INSIGHT_IDS.has(activeView)) ||
                        (item.id === "Home" &&
                          (activeView === "Home" || activeView === "Agent")),
                      group.tier,
                    )}
                  >
                    <span className="grid w-4 place-items-center text-sm text-current/80">
                      {item.icon === "gear" ? (
                        <GearIcon className="size-3.5" />
                      ) : (
                        item.icon
                      )}
                    </span>
                    <span className="min-w-0 flex-1 truncate">{item.label}</span>
                    {showLights && light && light !== "ready" && (
                      <span
                        className={`size-1.5 shrink-0 rounded-full ${setupLightClass(light)}`}
                        title={setupLightTitle(light)}
                        aria-label={setupLightTitle(light)}
                      />
                    )}
                    {needsDesktop && (
                      <span
                        className="rounded bg-white/6 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-slate-500"
                        title="Requires ADE Desktop"
                      >
                        Desktop
                      </span>
                    )}
                  </button>
                );
              });

            return (
              <div key={group.title ?? `tier-${group.tier}-${groupIndex}`}>
                {group.tier === 0 ? (
                  <div className="space-y-px" data-testid="ade-nav-home">
                    {renderItems()}
                  </div>
                ) : group.title === "Setup" ? (
                  <NavFold
                    title="Setup"
                    summary={
                      setupLights.overall === "ready"
                        ? "Ready"
                        : setupLights.overall === "warn"
                          ? "Attention"
                          : "Recommended"
                    }
                    storageKey="ade_nav_setup_fold"
                    defaultOpen={setupLights.overall !== "ready"}
                    forceOpen={
                      setupLights.overall === "warn" &&
                      (activeView === "Environment" ||
                        activeView === "Keys" ||
                        activeView === "Recipes" ||
                        activeView === "Verify")
                    }
                    light={setupLights.overall}
                  >
                    {renderItems()}
                  </NavFold>
                ) : group.title === "More" ? (
                  <NavFold
                    title="More"
                    storageKey="ade_nav_more_fold"
                    defaultOpen={false}
                    forceOpen={
                      debugChrome && (activeView === "Rules" || activeView === "MCP")
                    }
                  >
                    {renderItems()}
                  </NavFold>
                ) : (
                  <div className="space-y-px">{renderItems()}</div>
                )}
                {groupIndex === 0 && (
                  <div className="mt-2">
                    <SidebarRailLists
                      workspaces={workspaceList}
                      workspaceBusy={workspaceSwitchBusy}
                      sessions={shellTabs}
                      activeSessionId={activeTabId}
                      sessionsActive={
                        activeView === "Home" || activeView === "Agent"
                      }
                      onOpenWorkspace={(path) => {
                        void switchWorkspaceFromRail(path);
                      }}
                      onManageWorkspaces={() => {
                        setWorkspacesStartNew(false);
                        openNavView("Workspaces");
                        setNavOpenPersisted(false);
                      }}
                      onNewWorkspace={() => {
                        setWorkspacesStartNew(true);
                        openNavView("Workspaces");
                        setNavOpenPersisted(false);
                      }}
                      onFocusSession={(id) => {
                        focusShellTab(id);
                        setNavOpenPersisted(false);
                      }}
                      onNewSession={() => {
                        openAgentTab();
                        setNavOpenPersisted(false);
                      }}
                      onCloseSession={(id) => {
                        closeShellTab(id);
                      }}
                    />
                  </div>
                )}
              </div>
            );
          })}
        </nav>

        <div className="mt-3 border-t border-white/6 pt-3">
          <button
            type="button"
            title={
              surfaceMode === "dev"
                ? "Debug on — click for Standard (hide Atlas / Plan / Editor / MCP)"
                : "Turn on Debug for Atlas, Plan Map, Editor, MCP, and harness panels"
            }
            onClick={() => {
              setSurfaceModePersisted(surfaceMode === "dev" ? "power" : "dev");
            }}
            className={`mb-1 flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-xs transition ${
              surfaceMode === "dev"
                ? "bg-amber-500/12 text-amber-100"
                : "text-slate-500 hover:bg-white/4 hover:text-slate-300"
            }`}
          >
            <span
              className={`size-1.5 shrink-0 rounded-full ${
                surfaceMode === "dev" ? "bg-amber-400" : "bg-slate-600"
              }`}
            />
            <span className="min-w-0 flex-1 truncate">
              {surfaceMode === "dev" ? "Debug on" : "Debug"}
            </span>
            <span className="text-[10px] text-slate-600">
              {surfaceMode === "dev" ? "harness" : "off"}
            </span>
          </button>
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
            <GearIcon className="size-3.5 shrink-0 text-current/80" />
            Settings
          </button>
          <div className="px-3 py-1.5 text-[10px] leading-4 text-slate-600">
            {isTauri() ? "Desktop" : "Browser preview"}
            {debugChrome && mcpServers.length > 0 ? ` · ${mcpServers.length} MCP` : ""}
          </div>
        </div>
      </aside>

      <main
        className={`min-w-0 flex-1 ${
          activeView === "Home" ||
          activeView === "Agent" ||
          activeView === "Browser" ||
          activeView === "Editor" ||
          activeView === "Terminal" ||
          INSIGHT_IDS.has(activeView)
            ? "flex flex-col overflow-hidden"
            : "thin-scrollbar overflow-y-auto"
        }`}
      >
        <header className="flex h-12 shrink-0 items-center gap-2 border-b border-white/7 bg-[#080b11] px-3 sm:h-14 sm:px-5">
          <button
            type="button"
            className="grid size-8 shrink-0 place-items-center rounded-lg border border-white/10 bg-white/2.5 text-slate-300 md:hidden"
            aria-label="Open menu"
            onClick={() => setNavOpenPersisted(true)}
          >
            ☰
          </button>
          <ShellTabBar
            tabs={shellTabs}
            activeTabId={
              activeTabId &&
              ["Home", "Agent", "Browser", "Editor", "Terminal"].includes(
                activeView,
              )
                ? activeTabId
                : null
            }
            onSelect={focusShellTab}
            onClose={closeShellTab}
          />
          {activeTabId === null && (
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <h1 className="text-sm font-semibold leading-tight">
                  {activeView === "Environment"
                    ? "Environment"
                    : activeView === "Rules"
                      ? "Guidance"
                      : INSIGHT_IDS.has(activeView)
                        ? "Insight"
                        : activeView === "Verify"
                          ? "Test project"
                          : activeView === "Recipes"
                            ? "Stack"
                            : activeView}
                </h1>
                {debugChrome && (
                  <span className="rounded bg-amber-400/15 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-amber-200">
                    Debug
                  </span>
                )}
              </div>
              <p
                className="mt-0.5 max-w-[36vw] truncate text-[10px] text-slate-500 sm:max-w-[52vw]"
                title={dashboard?.workspace_root ?? undefined}
              >
                {dashboard?.workspace_root
                  ? `Working in ${workspaceLeaf(dashboard.workspace_root)}`
                  : "No folder attached"}
                {!isTauri() && " · browser"}
              </p>
            </div>
          )}
          <div className="ml-auto flex shrink-0 items-center">
            <div className="flex items-center gap-0.5 rounded-lg border border-white/10 bg-white/[0.03] p-0.5">
              {isTauri() ? (
                <>
                  <button
                    type="button"
                    onClick={() => openAgentTab()}
                    aria-label="New Agent"
                    title="New Agent"
                    className="grid size-7 place-items-center rounded-md text-base font-medium leading-none text-slate-200 transition hover:bg-blue-500/20 hover:text-white"
                  >
                    <span aria-hidden className="relative -top-px">
                      +
                    </span>
                  </button>
                  <span className="mx-0.5 h-4 w-px bg-white/10" aria-hidden />
                  <button
                    type="button"
                    onClick={() => openBrowserTab()}
                    aria-label="New Browser"
                    title="New Browser"
                    className="grid size-7 place-items-center rounded-md text-[12px] text-slate-400 transition hover:bg-white/8 hover:text-slate-100"
                  >
                    ⬚
                  </button>
                  <button
                    type="button"
                    onClick={() => void openEditorTab()}
                    aria-label="Open File"
                    title="Open File"
                    className="grid size-7 place-items-center rounded-md text-[12px] text-slate-400 transition hover:bg-white/8 hover:text-slate-100"
                  >
                    ✎
                  </button>
                  <button
                    type="button"
                    onClick={() => openTerminalTab()}
                    aria-label="Terminal"
                    title="Terminal"
                    className="grid size-7 place-items-center rounded-md text-[12px] text-slate-400 transition hover:bg-white/8 hover:text-slate-100"
                  >
                    ▸
                  </button>
                </>
              ) : null}
              <HeaderOverflowMenu
                isDesktop={isTauri()}
                actions={[
                  {
                    id: "folder",
                    label: "Change folder…",
                    description: dashboard?.workspace_root
                      ? workspaceLeaf(dashboard.workspace_root)
                      : "Attach a workspace",
                    icon: "▤",
                    desktopOnly: true,
                    onSelect: () => {
                      setWorkspacesStartNew(false);
                      openNavView("Workspaces");
                    },
                  },
                ]}
              />
            </div>
            <button
              onClick={() => void refresh()}
              disabled={loading}
              aria-label="Refresh dashboard"
              title="Refresh"
              className="ml-1.5 grid size-7 place-items-center rounded-md text-slate-500 transition hover:bg-white/6 hover:text-slate-200 disabled:opacity-50"
            >
              ↻
            </button>
          </div>
        </header>

        <div
          className={
            activeView === "Home" || activeView === "Agent"
              ? "mx-auto flex min-h-0 w-full max-w-3xl flex-1 flex-col px-4 py-3 sm:px-5"
              : activeView === "Browser" ||
                  activeView === "Editor" ||
                  activeView === "Terminal"
                ? `flex min-h-0 flex-1 flex-col overflow-hidden ${
                    activeView === "Browser" ? "p-2 sm:p-3" : "p-4 sm:p-5"
                  }`
                : INSIGHT_IDS.has(activeView)
                  ? "mx-auto flex min-h-0 w-full max-w-350 flex-1 flex-col overflow-hidden p-4 sm:p-5"
                  : "mx-auto max-w-350 p-4 sm:p-5"
          }
        >
          {!isTauri() && (
            <div className="mb-4">
              <BrowserApiSetup
                refreshKey={browserApiProbeKey}
                onResolved={() => {
                  if (!dashboard) {
                    void refresh();
                  }
                }}
              />
            </div>
          )}
          {error && (
            <div className="mb-5 rounded-xl border border-red-400/20 bg-red-400/7 px-4 py-3 text-xs text-red-200">
              {error}
            </div>
          )}

          {/* Sub-navigation is chrome: it stays put even when the data read fails. */}
          {INSIGHT_IDS.has(activeView) && (
            <SubTabs
              className="mb-3 shrink-0 self-start"
              ariaLabel="Insight sections"
              items={insightTabs}
              activeId={activeView}
              onSelect={openInsightSection}
            />
          )}

          {/* Settings never needs the API — pure localStorage defaults. */}
          {activeView === "Settings" && (
            <SettingsView
              onOpenKeys={() => setActiveView("Keys")}
              onOpenIntegrations={() => openNavView("Integrations")}
            />
          )}

          {/* Desktop-only funnels work offline in browser (honest CTA, no blank pane). */}
          {activeView !== "Settings" &&
            DESKTOP_REQUIRED_VIEWS.has(activeView) &&
            !isTauri() && (
              <DesktopRequired view={activeView} simpleMode={false} />
            )}

          {activeView !== "Settings" &&
            !(DESKTOP_REQUIRED_VIEWS.has(activeView) && !isTauri()) &&
            (loading && !dashboard ? (
              <LoadingState />
            ) : !dashboard ? (
              activeView === "Home" || activeView === "Agent" ? (
                <div className="mx-auto max-w-lg rounded-xl border border-white/8 bg-white/[0.02] px-5 py-8 text-center">
                  <h2 className="text-xl font-semibold tracking-tight text-slate-50">
                    ADE
                  </h2>
                  <p className="mt-2 text-sm leading-6 text-slate-400">
                    Connect the local API above to load this workspace — or open
                    the Desktop app for chat, keys, and MCP.
                  </p>
                </div>
              ) : null
            ) : (
              <>
              {(activeView === "Home" || activeView === "Agent") &&
                (isTauri() ? (
                  (() => {
                    const agentTabs = shellTabs.filter(
                      (tab) => tab.kind === "agent",
                    );
                    const focusedAgentId = agentTabs.some(
                      (tab) => tab.id === activeTabId,
                    )
                      ? activeTabId
                      : HOME_AGENT_TAB;
                    return agentTabs.map((tab) => {
                      const active = tab.id === focusedAgentId;
                      return (
                        <div
                          key={tab.id}
                          className={
                            active
                              ? "flex min-h-0 flex-1 flex-col"
                              : "hidden"
                          }
                          aria-hidden={!active}
                        >
                          <AgentView
                            key={`${dashboard.workspace_root}:${tab.id}`}
                            events={
                              runningAgentTabId === tab.id ||
                              (runningAgentTabId === null && active)
                                ? agentEvents
                                : []
                            }
                            busy={
                              agentBusy &&
                              (runningAgentTabId === tab.id ||
                                (runningAgentTabId === null && active))
                            }
                            connectedTools={mcpTools.length}
                            mcpServerNames={mcpServers}
                            mcpTools={mcpTools}
                            initialPrompt={
                              tab.id === HOME_AGENT_TAB ? homePrompt : ""
                            }
                            autoSubmit={
                              tab.id === HOME_AGENT_TAB && agentAutoSubmit
                            }
                            autoSubmitContext={agentAutoSubmitContext}
                            onAutoSubmitHandled={() => {
                              setAgentAutoSubmit(false);
                              setAgentAutoSubmitContext("normal");
                            }}
                            newChatNonce={0}
                            ephemeralChat={Boolean(tab.ephemeral)}
                            sharedVerifyGate={gate}
                            devMode={debugChrome}
                            simpleMode={false}
                            workspaceRoot={dashboard.workspace_root}
                            guidedWins={guidedWins}
                            isDogfood={Boolean(dashboard.is_dogfood)}
                            understandBusy={understandBusy}
                            verifying={verifying}
                            onUnderstand={() => void runUnderstandProject()}
                            onVerifyHome={() =>
                              void runVerify({ stayOnHome: true })
                            }
                            onImproveAde={startImproveAde}
                            onOpenWorkspaces={() => {
                              setWorkspacesStartNew(false);
                              openNavView("Workspaces");
                            }}
                            onOpenEnvironment={() => openNavView("Environment")}
                            leases={dashboard.leases}
                            planOwnedPaths={[
                              ...new Set([
                                ...dashboard.plan.phases.flatMap(
                                  (phase) => phase.owned_paths,
                                ),
                                ...(!dashboard.has_recipe
                                  ? [".ade/recipe.json"]
                                  : []),
                              ]),
                            ]}
                            rebuildLockWarnings={
                              dashboard.rebuild_lock_warnings ?? []
                            }
                            handoffAvailable={
                              dashboard.handoff.capsule_count > 0 ||
                              Boolean(dashboard.handoff.latest_status)
                            }
                            handoffLatestStatus={dashboard.handoff.latest_status}
                            continuityBusy={continuityBusy}
                            onContinueHandoff={() => void continueLastHandoff()}
                            onClearTranscript={() => setAgentEvents([])}
                            onOpenKeys={() => openNavView("Keys")}
                            onOpenIntegrations={() => openNavView("Integrations")}
                            tasks={dashboard.tasks ?? []}
                            planPhaseCount={dashboard.plan.phases.length}
                            onRefresh={() => void refresh()}
                            onSurfaceFailure={(error) => {
                              setError(error);
                              setRunningAgentTabId(tab.id);
                              setAgentEvents([{ type: "failed", error }]);
                            }}
                            onRun={(input) => {
                              setRunningAgentTabId(tab.id);
                              void runAgentTurn(input);
                            }}
                            onCancel={() => void cancelAgentTurn()}
                            onRenameTab={(title) => updateTabTitle(tab.id, title)}
                          />
                        </div>
                      );
                    });
                  })()
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
                    onOpenAgent={() => openNavView("Home")}
                    onOpenHealth={() => openNavView("Environment")}
                    onOpenWorkspaces={() => openNavView("Workspaces")}
                    onOpenRecipes={() => openNavView("Recipes")}
                    onOpenKeys={() => openNavView("Keys")}
                    onOpenVerify={() => openNavView("Verify")}
                    onUnderstand={() => void runUnderstandProject()}
                    onVerifyHome={() => void runVerify({ stayOnHome: true })}
                    onImproveAde={startImproveAde}
                    onOpenAdeOnItself={() => void openAdeOnItself()}
                    onRunAgent={() => {
                      if (!homePrompt.trim()) return;
                      setAgentAutoSubmit(true);
                      openNavView("Home");
                    }}
                    onApplyPreset={(preset) => {
                      window.localStorage.setItem(AUTONOMY_KEY, preset.autonomy);
                      setHomePrompt(preset.prompt);
                      setAgentAutoSubmit(true);
                      openNavView("Home");
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
                  executeAvailable={isTauri()}
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
                  onOpenHome={() => openNavView("Home")}
                  onOpenWorkspaces={() => openNavView("Workspaces")}
                  onContinueHandoff={() => void continueLastHandoff()}
                  continuityBusy={continuityBusy}
                  onReviewHandoffInEditor={() => {
                    window.sessionStorage.setItem(
                      ADE_EDITOR_INTENT_KEY,
                      JSON.stringify({ mode: "handoff" }),
                    );
                    // Editor is under Debug nav — enable it so Continuity deep links stick.
                    setSurfaceModePersisted("dev");
                    const id = newTabId("editor");
                    setShellTabs((tabs) => [
                      ...tabs,
                      {
                        id,
                        kind: "editor",
                        title: "Handoff",
                        closable: true,
                      },
                    ]);
                    setActiveTabId(id);
                    setActiveView("Editor");
                  }}
                  onRefresh={() => void refresh()}
                  devMode={debugChrome}
                />
              )}
              {activeView === "Workspaces" && (
                <WorkspacesView
                  startWithNew={workspacesStartNew}
                  onStartWithNewConsumed={() => setWorkspacesStartNew(false)}
                  onOpened={(intent?: WorkspaceOpenIntent) => {
                    void refresh();
                    setWorkspacesStartNew(false);
                    openNavView(intent === "work" ? "Home" : "Environment");
                  }}
                  onOpenEnvironment={() => openNavView("Environment")}
                />
              )}
              {activeView === "Browser" &&
                shellTabs
                  .filter((tab) => tab.kind === "browser")
                  .map((tab) => (
                    <div
                      key={tab.id}
                      className={
                        tab.id === activeTabId
                          ? "flex min-h-0 flex-1 flex-col"
                          : "hidden"
                      }
                      aria-hidden={tab.id !== activeTabId}
                    >
                      <BrowserView
                        instanceId={tab.id}
                        initialUrl={tab.url ?? defaultBrowserUrl()}
                        active={tab.id === activeTabId && activeView === "Browser"}
                        onTitleChange={(title) =>
                          updateTabTitle(tab.id, title)
                        }
                      />
                    </div>
                  ))}
              {activeView === "Terminal" &&
                shellTabs
                  .filter((tab) => tab.kind === "terminal")
                  .map((tab) => (
                    <div
                      key={tab.id}
                      className={
                        tab.id === activeTabId
                          ? "flex min-h-0 flex-1 flex-col"
                          : "hidden"
                      }
                      aria-hidden={tab.id !== activeTabId}
                    >
                      <TerminalView />
                    </div>
                  ))}
              {activeView === "Editor" &&
                shellTabs
                  .filter((tab) => tab.kind === "editor")
                  .map((tab) => (
                    <div
                      key={tab.id}
                      className={
                        tab.id === activeTabId
                          ? "flex min-h-0 flex-1 flex-col"
                          : "hidden"
                      }
                      aria-hidden={tab.id !== activeTabId}
                    >
                      <EditorView
                        initialPath={tab.path}
                        autoPick={!tab.path}
                        onTitleChange={(title) =>
                          updateTabTitle(tab.id, title)
                        }
                      />
                    </div>
                  ))}
              {activeView === "Keys" && (
                <KeysView
                  simpleMode={false}
                  onContinueToAgent={() => openNavView("Home")}
                  onOpenIntegrations={() => openNavView("Integrations")}
                />
              )}
              {activeView === "Integrations" && (
                <IntegrationsView
                  mcpServers={mcpServers}
                  mcpToolCount={mcpTools.length}
                  onOpenKeys={() => openNavView("Keys")}
                  onOpenMcp={() => {
                    setSurfaceModePersisted("dev");
                    openNavView("MCP");
                  }}
                  onOpenView={(view) => {
                    if (view === "MCP") {
                      setSurfaceModePersisted("dev");
                    }
                    openNavView(view);
                  }}
                  onConnectMcp={connectMcp}
                  onRefreshMcp={() => void refreshMcp()}
                />
              )}
              {INSIGHT_IDS.has(activeView) && (
                <div
                  className={
                    activeView === "Atlas"
                      ? "flex min-h-0 flex-1 flex-col"
                      : "thin-scrollbar min-h-0 flex-1 overflow-y-auto pr-0.5"
                  }
                >
                  {activeView === "Audit" && (
                    <AuditView
                      audit={dashboard.audit}
                      handoffs={dashboard.handoff.recent}
                      onRefresh={() => void refresh()}
                      onOpenSettings={() => setActiveView("Settings")}
                      onOpenAnalytics={() => openInsightSection("Analytics")}
                    />
                  )}
                  {activeView === "Analytics" && (
                    <AnalyticsView
                      verifyResults={verifyResults}
                      tasks={dashboard.tasks}
                      handoffs={dashboard.handoff.recent}
                      onOpenSettings={() => setActiveView("Settings")}
                      onOpenTrust={() => openInsightSection("Audit")}
                      onOpenVerify={() => setActiveView("Verify")}
                    />
                  )}
                  {activeView === "Plan" && (
                    <PlanMap
                      plan={dashboard.plan}
                      scorePercent={scorePercent}
                      verifyResults={verifyResults}
                      executing={executing}
                      executeAvailable={isTauri()}
                      focusPhaseId={planFocusPhaseId}
                      onExecute={() => void executePlan()}
                      onRunAudit={() => void runAudit()}
                      onRunVerify={() => void runVerify()}
                      onOpenGuidance={() => setActiveView("Rules")}
                      onOpenAtlas={(phaseId) => {
                        setAtlasFocusNodeId(phaseId ? `phase:${phaseId}` : "hub-plan");
                        openInsightSection("Atlas");
                      }}
                    />
                  )}
                  {activeView === "Atlas" && (
                    <AtlasView
                      auditFindings={dashboard.audit.findings}
                      planPhases={dashboard.plan.phases}
                      verifyGates={verifyResults}
                      handoffs={dashboard.handoff.recent}
                      focusNodeId={atlasFocusNodeId}
                      onOpenGuidance={() => setActiveView("Rules")}
                      onOpenPlan={(phaseId) => {
                        setPlanFocusPhaseId(phaseId ?? null);
                        openInsightSection("Plan");
                      }}
                    />
                  )}
                </div>
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
                    openInsightSection("Atlas");
                  }}
                  onOpenPlan={() => openInsightSection("Plan")}
                />
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
                  initializeAvailable={isTauri()}
                  onPreview={previewRecipe}
                  onInitialize={(input) => void initializeRecipe(input)}
                />
              )}
              </>
            ))}
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

  const gettingStartedSteps = buildGettingStartedSteps({
    understand: guidedWins.understand,
    verify: guidedWins.verify,
    improveAde: guidedWins.improve_ade,
    isDogfood: Boolean(dashboard.is_dogfood),
    understandBusy,
    verifying,
    improveBusy: agentBusy,
    onUnderstand,
    onVerify: onVerifyHome,
    onImprove: onImproveAde,
    keysTrailing: (
      <button
        type="button"
        title="Keys"
        onClick={(event) => {
          event.stopPropagation();
          onOpenKeys();
        }}
        className="inline-flex rounded-md p-0.5 hover:bg-white/5"
      >
        <BrandWell id="keys" size="sm" status="info" />
      </button>
    ),
    improveTrailing: (
      <button
        type="button"
        title="Integrations"
        onClick={(event) => {
          event.stopPropagation();
          onOpenRecipes();
        }}
        className="inline-flex rounded-md p-0.5 hover:bg-white/5"
      >
        <BrandWell id="github" size="sm" status="info" title="Open stack / Integrations" />
      </button>
    ),
  });

  const readinessSteps = [
    {
      id: "keys",
      title: inBrowser ? "Add an API key (Desktop)" : "Add an API key",
      detail: inBrowser
        ? "Open ADE Desktop to save a key securely"
        : "So ADE can call your model",
      done: inBrowser ? false : keyReady,
      desktopOnly: inBrowser,
      cta: inBrowser ? "Open Desktop path" : "Add API key",
      onClick: onOpenKeys,
    },
    {
      id: "recipe",
      title: "Choose a stack",
      detail: "Pick a stack so ADE knows how this project is built",
      done: recipeReady,
      desktopOnly: false,
      cta: "Choose stack",
      onClick: onOpenRecipes,
    },
    {
      id: "verify",
      title: "Test the project",
      detail: "Run build/lint/test gates once before trusting agent changes",
      done: verifyReady,
      desktopOnly: false,
      cta: "Test project",
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
  const heroSubtitle = !dashboard.workspace_root
    ? "Attach a workspace folder first — then ask ADE."
    : dashboard.is_default
      ? "Scratch workspace (Default). Ask anything — or create a real project when you're ready."
      : `Working in ${envName}. Ask ADE about this environment.`;

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
          {dashboard.is_default ? " · Default scratch" : ""}
          {dashboard.is_dogfood ? " · dogfood" : ""}
          {" · "}
          <button
            type="button"
            onClick={onOpenHealth}
            className="text-slate-400 hover:text-blue-200"
          >
            Setup check
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
              Open Test project →
            </button>
          </div>
        )}

        <div className="mt-4 flex flex-col gap-3 sm:flex-row sm:items-stretch">
          <textarea
            value={prompt}
            onChange={(event) => onPromptChange(event.target.value)}
            rows={simpleMode ? 4 : 3}
            placeholder="Describe what you want help with…"
            className={`w-full flex-1 resize-y rounded-xl border border-white/10 bg-black/25 px-4 py-3 text-sm text-slate-200 outline-hidden ring-blue-400/30 placeholder:text-slate-600 focus:ring-2 ${
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
            {agentBusy ? "…" : isTauri() ? "Go" : "Open Desktop"}
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

        <GettingStartedChecklist
          className="mt-4"
          steps={gettingStartedSteps}
        />
        {guidedWins.understand && lastUnderstandPath && (
          <p className="mt-2 font-mono text-[10px] text-emerald-200/70">
            {lastUnderstandPath}
          </p>
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
  executeAvailable = true,
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
  continuityBusy = false,
  devMode = false,
}: {
  dashboard: DashboardSnapshot;
  scorePercent: number;
  verifyResults: VerifyResult[];
  verifying?: boolean;
  executing: boolean;
  executeAvailable?: boolean;
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
  continuityBusy?: boolean;
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
      title: "Project tests not run yet",
      detail:
        "Run Test project once after setup — it checks build/lint/test gates (not the chat).",
      severity: "warn",
      fixLabel: verifying ? "Testing…" : "Run tests",
      onFix: verifying ? undefined : onRunChecks,
    });
  } else if (failedVerify.length > 0) {
    gaps.push({
      id: "verify-fail",
      title: `${failedVerify.length} project test${failedVerify.length === 1 ? "" : "s"} failing`,
      detail: failedVerify
        .slice(0, 3)
        .map((item) => item.gate)
        .join(", "),
      severity: "block",
      fixLabel: verifying ? "Testing…" : "Re-run tests",
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
          ? "Project tests need attention"
          : "Almost ready — run Test project once"
        : "Ready to work";

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
              Setup check
            </div>
            <h2 className="mt-1 text-sm font-semibold text-slate-100">{heroTitle}</h2>
            <p className="mt-1 text-xs leading-5 text-slate-500">
              {dashboard.workspace_root ? (
                <>
                  Folder:{" "}
                  <span className="font-mono text-slate-400">
                    {workspaceLeaf(dashboard.workspace_root)}
                  </span>
                </>
              ) : (
                "No folder attached"
              )}
              {" · "}
              <button
                type="button"
                onClick={onOpenWorkspaces}
                className="text-blue-300/90 hover:text-blue-200"
              >
                Change folder
              </button>
            </p>
          </div>
          {onlyChecksGap ? (
            <button
              type="button"
              disabled={verifying}
              onClick={onRunChecks}
              className="shrink-0 rounded-lg bg-blue-500 px-3.5 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
            >
              {verifying ? "Testing…" : "Run tests"}
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

      {deferredFindings.length > 0 && (
        <Disclosure
          title="Deferred notes"
          summary={`${deferredFindings.length}`}
          defaultOpen={false}
          storageKey="ade_env_deferred"
        >
          <ul className="space-y-1 px-4 pb-4 sm:px-5">
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
        </Disclosure>
      )}

      {globalAudit && (
        <Disclosure
          title="This computer"
          summary={globalAudit.ok ? "ready" : "needs attention"}
          defaultOpen={!globalAudit.ok}
          storageKey="ade_env_machine"
        >
          <div
            className={`mx-4 mb-4 rounded-lg border px-3 py-2 text-[11px] sm:mx-5 ${
              globalAudit.ok
                ? "border-emerald-400/20 bg-emerald-400/5 text-emerald-100/85"
                : "border-amber-400/25 bg-amber-400/8 text-amber-100/90"
            }`}
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-semibold">
                {globalAudit.ok ? "ADE ready on this machine" : "This machine needs attention"}
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
        </Disclosure>
      )}
      {devMode && (
        <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-1.5 text-[11px] text-amber-100/80">
          Debug · score {dashboard.audit.score}/{dashboard.audit.score_max} · leases{" "}
          {dashboard.leases.length}
        </div>
      )}
      <Disclosure
        title="Scores"
        summary={`${scorePercent}% ready`}
        defaultOpen={false}
        storageKey="ade_env_scores"
      >
        <section
          className={`grid gap-2 px-4 pb-4 sm:px-5 ${
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
      </Disclosure>

      <Disclosure
        title="Health details"
        summary={`${scorePercent}%`}
        defaultOpen={false}
        storageKey="ade_env_health"
      >
        <div className="px-4 pb-4 sm:px-5">
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
        </div>
      </Disclosure>

      <Disclosure
        title="Remediation plan"
        summary={
          dashboard.plan.phases.length > 0
            ? `${dashboard.plan.phases.length} phase${dashboard.plan.phases.length === 1 ? "" : "s"}`
            : "none"
        }
        forceOpen={dashboard.plan.phases.length > 0}
        storageKey="ade_env_plan"
      >
        <div className="px-4 pb-4 sm:px-5">
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
                disabled={executing || !executeAvailable}
                title={
                  executeAvailable
                    ? undefined
                    : "Approve & execute requires ADE Desktop"
                }
                className="w-full rounded-lg bg-blue-500/90 py-2 text-xs font-semibold text-white hover:bg-blue-400 disabled:opacity-50"
              >
                {executing
                  ? "Executing…"
                  : executeAvailable
                    ? "Review and execute"
                    : "Execute · Desktop"}
              </button>
            </div>
          )}
        </div>
      </Disclosure>

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
              className="w-full rounded-md border border-white/10 bg-[#101620] px-2.5 py-1.5 text-[11px] text-slate-200 outline-hidden focus:border-blue-400/40"
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
                    autonomy: "act",
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
              disabled={continuityBusy}
              onClick={onContinueHandoff}
              className="shrink-0 rounded-md border border-blue-400/30 bg-blue-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/25 disabled:opacity-40"
            >
              {continuityBusy ? "Working…" : "Continue → Home"}
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
  connectedTools,
  mcpServerNames = [],
  mcpTools = [],
  onRun,
  initialPrompt = "",
  autoSubmit = false,
  autoSubmitContext = "normal",
  onAutoSubmitHandled,
  sharedVerifyGate = "G3",
  newChatNonce = 0,
  ephemeralChat = false,
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
  continuityBusy = false,
  onContinueHandoff,
  onClearTranscript,
  onOpenKeys,
  onOpenIntegrations,
  tasks = [],
  planPhaseCount = 0,
  onRefresh,
  onSurfaceFailure,
  onCancel,
  onRenameTab,
  guidedWins,
  isDogfood = false,
  understandBusy = false,
  verifying = false,
  onUnderstand,
  onVerifyHome,
  onImproveAde,
}: {
  events: AgentEvent[];
  busy: boolean;
  connectedTools: number;
  mcpServerNames?: string[];
  mcpTools?: McpToolInfo[];
  initialPrompt?: string;
  autoSubmit?: boolean;
  autoSubmitContext?: "normal" | "continuity";
  onAutoSubmitHandled?: () => void;
  sharedVerifyGate?: string;
  /** Parent header + Chat bumps this to clear the transcript. */
  newChatNonce?: number;
  /** Extra Agent tabs: in-memory only (do not touch workspace thread.json). */
  ephemeralChat?: boolean;
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
  continuityBusy?: boolean;
  onContinueHandoff?: () => void;
  onClearTranscript?: () => void;
  onOpenKeys?: () => void;
  onOpenIntegrations?: () => void;
  tasks?: AgentTask[];
  planPhaseCount?: number;
  onRefresh?: () => void;
  onSurfaceFailure?: (error: string) => void;
  onCancel?: () => void;
  /** Rename the shell tab from the first user prompt. */
  onRenameTab?: (title: string) => void;
  guidedWins?: GuidedWinsState;
  isDogfood?: boolean;
  understandBusy?: boolean;
  verifying?: boolean;
  onUnderstand?: () => void;
  onVerifyHome?: () => void;
  onImproveAde?: () => void;
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
    approvedRiskCategories?: string[];
    approvedRiskTiers?: string[];
    allowUnpriced?: boolean;
    claimedTaskId?: string | null;
    waiveQueue?: boolean;
    slotOverride?: string | null;
    imagePaths?: string[];
  }) => void;
}) {
  const [prompt, setPrompt] = useState(initialPrompt);
  const [attachments, setAttachments] = useState<ChatAttachment[]>([]);
  const [attachNote, setAttachNote] = useState<string | null>(null);
  const [attachBusy, setAttachBusy] = useState(false);
  const [composerDragOver, setComposerDragOver] = useState(false);
  type QueuedPrompt = { id: string; prompt: string; imagePaths: string[] };
  const [promptQueue, setPromptQueue] = useState<QueuedPrompt[]>([]);
  const promptQueueRef = useRef(promptQueue);
  promptQueueRef.current = promptQueue;
  const queueDrainLockRef = useRef(false);
  const tabTitledRef = useRef(false);

  const renameTabFromPrompt = useCallback(
    (text: string) => {
      if (tabTitledRef.current || !onRenameTab) return;
      const title = agentTabTitleFromPrompt(text);
      if (!title) return;
      onRenameTab(title);
      tabTitledRef.current = true;
    },
    [onRenameTab],
  );
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
  const [modelMode, setModelMode] = useState<ModelMode>(() => {
    if (typeof window === "undefined") return "auto";
    const raw = window.localStorage.getItem(AGENT_MODEL_MODE_KEY);
    return raw === "pin" ? "pin" : "auto";
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
    if (typeof window === "undefined") return "0";
    return window.localStorage.getItem("ade_session_cap_usd") || "0";
  });
  const [dailyCap, setDailyCap] = useState(() => {
    if (typeof window === "undefined") return "0";
    return window.localStorage.getItem("ade_daily_cap_usd") || "0";
  });
  const [autonomy, setAutonomy] = useState<AutonomyLevel>(readAutonomy);
  // One-time: older builds defaulted session/daily caps with $0 rates, which
  // blocked free models. Clear stale caps unless the user set real $/MTok.
  useEffect(() => {
    if (typeof window === "undefined") return;
    if (window.localStorage.getItem("ade_spend_caps_migrated_v1") === "1") return;
    const ratesZero =
      (Number(inputCost) || 0) <= 0 && (Number(outputCost) || 0) <= 0;
    if (ratesZero && (Number(sessionCap) > 0 || Number(dailyCap) > 0)) {
      setSessionCap("0");
      setDailyCap("0");
      window.localStorage.setItem("ade_session_cap_usd", "0");
      window.localStorage.setItem("ade_daily_cap_usd", "0");
    }
    window.localStorage.setItem("ade_spend_caps_migrated_v1", "1");
  }, []);
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
  const [forceOwnedPaths, setForceOwnedPaths] = useState<string[] | null>(
    () => readForceOwnedPaths(),
  );
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
  const contextLimit = useMemo(() => {
    const stored =
      typeof window !== "undefined"
        ? Number(window.localStorage.getItem(AGENT_CONTEXT_KEY) || "")
        : NaN;
    const override =
      provider === "custom" && Number.isFinite(stored) && stored > 0
        ? Math.floor(stored)
        : null;
    return modelContextWindow(provider, model, override);
  }, [provider, model]);
  type TurnRecord = {
    id: string;
    createdAt: string;
    user: string;
    events: AgentEvent[];
    attachments?: ChatAttachmentMeta[];
  };
  const [pastTurns, setPastTurns] = useState<TurnRecord[]>([]);
  const [currentUser, setCurrentUser] = useState<string | null>(null);
  /** Empty-canvas Home: hide saved transcript until resume / new turn. */
  const [historyOpen, setHistoryOpen] = useState(false);
  /** Survives archival so Retry CTAs still have the failed prompt. */
  const lastPromptRef = useRef<string | null>(null);
  /** Image paths for multimodal retry after vision_required gate / switch model. */
  const lastImagePathsRef = useRef<string[]>([]);
  /** Attachment metas for the in-flight turn (cleared from UI on submit). */
  const pendingTurnAttachmentsRef = useRef<ChatAttachmentMeta[]>([]);
  const [feedDragOver, setFeedDragOver] = useState(false);
  const [mentionOpen, setMentionOpen] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionIndex, setMentionIndex] = useState(0);
  const [mentionPaths, setMentionPaths] = useState<string[]>([]);
  const mentionRangeRef = useRef<{ start: number; end: number } | null>(null);
  const [chatReady, setChatReady] = useState(false);
  const [chatError, setChatError] = useState<string | null>(null);
  const [agentId, setAgentId] = useState(() => readOrCreateAgentId(workspaceRoot));
  const feedScrollRef = useRef<HTMLDivElement>(null);
  const feedBottomRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const autoFixedFailureRef = useRef<string | null>(null);
  const [failureNote, setFailureNote] = useState<string | null>(null);
  const [forceAdvancedOpen, setForceAdvancedOpen] = useState(false);

  const surfaceLeaseConflict = (conflict: PathLease) => {
    const error = `lease conflict: another agent (${conflict.agent_id.slice(0, 8)}) holds a write lease on ${conflict.path}`;
    setFailureNote(error);
    onSurfaceFailure?.(error);
  };

  const latestFailed = useMemo(() => {
    const fromLive = [...events]
      .reverse()
      .find(
        (event): event is Extract<AgentEvent, { type: "failed" }> =>
          event.type === "failed",
      );
    if (fromLive) return fromLive;
    const last = pastTurns[pastTurns.length - 1];
    if (!last) return undefined;
    return [...last.events]
      .reverse()
      .find(
        (event): event is Extract<AgentEvent, { type: "failed" }> =>
          event.type === "failed",
      );
  }, [events, pastTurns]);

  const failureAdvice = useMemo(() => {
    if (!latestFailed || busy) return null;
    return evaluateTurnFailure({
      error: latestFailed.error,
      providerId: provider,
      model,
      baseUrl,
      effort,
      handoffAvailable,
    });
  }, [latestFailed, busy, provider, model, baseUrl, effort, handoffAvailable]);

  const promptForRetry = () => {
    const live = (currentUser ?? lastPromptRef.current ?? "").trim();
    if (live) return live;
    // After archive / reload: recover from the latest failed saved turn.
    const last = pastTurns[pastTurns.length - 1];
    if (!last?.user?.trim()) return "";
    const failed = [...last.events]
      .reverse()
      .some((event) => event.type === "failed");
    return failed ? last.user.trim() : "";
  };

  const [approvedRiskCategories, setApprovedRiskCategories] = useState<string[]>(
    [],
  );
  const [approvedRiskTiers, setApprovedRiskTiers] = useState<string[]>([]);

  const runPromptAgain = useCallback(
    (
      text: string,
      overrides?: {
        provider?: string;
        baseUrl?: string;
        model?: string;
        effort?: EffortLevel;
        maxSteps?: number;
        approvedRiskCategories?: string[];
        approvedRiskTiers?: string[];
        allowUnpriced?: boolean;
        waiveQueue?: boolean;
        claimedTaskId?: string | null;
        slotOverride?: string | null;
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
      lastPromptRef.current = trimmed;
      setCurrentUser(trimmed);
      stickToBottomRef.current = true;
      renameTabFromPrompt(trimmed);
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
        approvedRiskCategories:
          overrides?.approvedRiskCategories ?? approvedRiskCategories,
        approvedRiskTiers: overrides?.approvedRiskTiers ?? approvedRiskTiers,
        allowUnpriced: overrides?.allowUnpriced,
        waiveQueue: overrides?.waiveQueue,
        claimedTaskId:
          overrides?.claimedTaskId !== undefined
            ? overrides.claimedTaskId
            : claimedTask &&
                (claimedTask.status === "claimed" ||
                  claimedTask.status === "running")
              ? claimedTask.id
              : null,
        slotOverride: overrides?.slotOverride,
        imagePaths: lastImagePathsRef.current,
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
      approvedRiskCategories,
      approvedRiskTiers,
      claimedTask,
      renameTabFromPrompt,
    ],
  );

  const applyFailureAction = useCallback(
    (action: TurnFailureAction, opts?: { auto?: boolean }) => {
      if (action.id === "open_keys") {
        onOpenKeys?.();
        setFailureNote("Open Keys, fix the vault key / base URL, then retry.");
        return;
      }
      if (action.id === "define_goal") {
        void completeActiveGoalContract();
        setFailureNote("Complete the eng-goal contract, then retry Apply.");
        return;
      }
      if (action.id === "switch_suggest") {
        setAutonomyPersisted("propose");
        setFailureNote("Switched to Suggest — inspect/plan without Act tools.");
        return;
      }
      if (action.id === "switch_apply") {
        setAutonomyPersisted("act");
        if (effort === "low") {
          setEffort("medium");
          window.localStorage.setItem(AGENT_EFFORT_KEY, "medium");
        }
        setFailureNote("Switched to Apply — retry when ready.");
        return;
      }
      if (action.id === "confirm_unmetered") {
        const promptText = promptForRetry();
        if (!promptText) {
          setFailureNote("No prompt to retry — type a message and Go again.");
          return;
        }
        window.localStorage.setItem("ade_allow_unpriced", "1");
        setFailureNote("Retrying without metering…");
        runPromptAgain(promptText, { allowUnpriced: true });
        return;
      }
      if (action.id === "open_spend_rates") {
        // Advanced $/MTok fields live under Debug — on Home, clear $0-rate caps
        // so the next turn is not blocked, then retry.
        setSessionCap("0");
        setDailyCap("0");
        window.localStorage.setItem("ade_session_cap_usd", "0");
        window.localStorage.setItem("ade_daily_cap_usd", "0");
        window.localStorage.setItem("ade_allow_unpriced", "1");
        window.localStorage.setItem("ade_agent_advanced_run", "1");
        setForceAdvancedOpen(true);
        const promptText = promptForRetry();
        if (promptText) {
          setFailureNote("Cleared spend caps for $0 rates — retrying…");
          runPromptAgain(promptText, { allowUnpriced: true });
        } else {
          setFailureNote(
            "Spend caps cleared. For paid models, set Input/Output $/MTok in Debug → Advanced.",
          );
        }
        return;
      }
      if (action.id === "apply_next") {
        onRefresh?.();
        const next = tasks.find((task) => task.status === "queued");
        if (next) {
          setFailureNote("Applying next queued task…");
          void applyClaimedTask(next, true);
        } else {
          setFailureNote("No queued task — refresh or Queue PLAN first.");
        }
        return;
      }
      if (action.id === "waive_queue") {
        const promptText = promptForRetry();
        if (!promptText) {
          setFailureNote("No prompt to retry — type a message and Go again.");
          return;
        }
        setFailureNote("Waiving queue — retrying free-form…");
        runPromptAgain(promptText, { waiveQueue: true });
        return;
      }
      if (action.id === "approve_risk") {
        const cats = action.categories ?? [];
        const tiers = action.tiers ?? ["high"];
        setApprovedRiskCategories(cats);
        setApprovedRiskTiers(tiers);
        const promptText = promptForRetry();
        if (!promptText) {
          setFailureNote(
            `Approved risk ${[...cats, ...tiers].join(", ")} — retry the prompt.`,
          );
          return;
        }
        setFailureNote(
          `Approved risk ${[...cats, ...tiers].join(", ")} — retrying…`,
        );
        runPromptAgain(promptText, {
          approvedRiskCategories: cats,
          approvedRiskTiers: tiers,
        });
        return;
      }
      if (action.id === "rotate_lease") {
        setAgentId(rotateAgentId(workspaceRoot));
        setFailureNote("Rotated lease id — retry Apply when ready.");
        return;
      }
      if (action.id === "enable_isolate") {
        setApplyIsolate(true);
        window.localStorage.setItem(APPLY_ISOLATE_KEY, "1");
        setFailureNote("Isolate enabled — next Apply uses a worktree.");
        return;
      }
      if (action.id === "wait_refresh") {
        onRefresh?.();
        setFailureNote("Refreshed leases — wait for the other agent or retry.");
        return;
      }
      if (action.id === "continue_handoff") {
        setEffort(action.effort);
        window.localStorage.setItem(AGENT_EFFORT_KEY, action.effort);
        setMaxSteps(String(action.maxSteps));
        setAutonomyPersisted("act");
        setFailureNote(
          opts?.auto
            ? `Auto-fixed: continue handoff · Effort ${action.effort}`
            : `Continuing handoff with Effort ${action.effort} (${action.maxSteps} rounds)…`,
        );
        onContinueHandoff?.();
        return;
      }
      const promptText = promptForRetry();
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
        setModelMode("pin");
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
        setModelMode("pin");
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
      if (action.id === "fix_base_url") {
        setBaseUrl(action.baseUrl);
        window.localStorage.setItem(AGENT_BASE_URL_KEY, action.baseUrl);
        setFailureNote(`Updated base URL → ${action.baseUrl}`);
        runPromptAgain(promptText, { baseUrl: action.baseUrl });
      }
    },
    [currentUser, pastTurns, onOpenKeys, onContinueHandoff, runPromptAgain, activeGoal, effort, tasks, onRefresh, workspaceRoot],
  );

  useEffect(() => {
    if (!latestFailed || busy || !failureAdvice?.autoFix) return;
    // Spend / contract CTAs need an explicit click — don't surprise with prompts.
    if (
      failureAdvice.autoFix.id === "confirm_unmetered" ||
      failureAdvice.autoFix.id === "define_goal"
    ) {
      return;
    }
    // continue_handoff does not need the prior user prompt; other autofixes do.
    if (failureAdvice.autoFix.id !== "continue_handoff" && !promptForRetry()) {
      return;
    }
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
      setHistoryOpen(false);
      setChatReady(true);
      return;
    }
    if (ephemeralChat) {
      setPastTurns([]);
      setHistoryOpen(false);
      setChatReady(true);
      return;
    }
    let cancelled = false;
    setChatReady(false);
    setChatError(null);
    setHistoryOpen(false);
    void invoke<{
      id: string;
      updatedAt: string;
      turns: {
        id: string;
        createdAt: string;
        user: string;
        events: AgentEvent[];
        attachments?: ChatAttachmentMeta[];
      }[];
    }>("chat_load")
      .then((thread) => {
        if (cancelled) return;
        const loaded = (thread.turns ?? []).map((turn) => ({
          id: turn.id,
          createdAt: turn.createdAt,
          user: turn.user,
          events: turn.events ?? [],
          attachments: turn.attachments,
        }));
        setPastTurns(loaded);
        setHistoryOpen(false);
        const firstUser = loaded.find((turn) => turn.user?.trim())?.user;
        if (firstUser) {
          renameTabFromPrompt(firstUser);
        }
        const last = loaded[loaded.length - 1];
        if (
          last?.user &&
          [...last.events].reverse().some((event) => event.type === "failed")
        ) {
          lastPromptRef.current = last.user;
        }
        setChatReady(true);
      })
      .catch((reason) => {
        if (cancelled) return;
        setChatError(String(reason));
        setPastTurns([]);
        setHistoryOpen(false);
        setChatReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceRoot, ephemeralChat]);

  useEffect(() => {
    if (busy || currentUser) setHistoryOpen(true);
  }, [busy, currentUser]);

  useEffect(() => {
    if (ephemeralChat || !chatReady || !isTauri() || !workspaceRoot) return;
    const turns = pastTurns.map((turn) => ({
      id: turn.id,
      createdAt: turn.createdAt,
      user: turn.user,
      events: turn.events,
      attachments: turn.attachments ?? null,
    }));
    void invoke("chat_save", { turns }).catch((reason) => {
      setChatError(String(reason));
    });
  }, [pastTurns, chatReady, workspaceRoot, ephemeralChat]);

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
    setHistoryOpen(false);
    setAttachments([]);
    setAttachNote(null);
    setPrompt("");
    setPromptQueue([]);
    tabTitledRef.current = false;
    onRenameTab?.(ephemeralChat ? "New Agent" : "Agent");
    lastPromptRef.current = null;
    lastImagePathsRef.current = [];
    rotateSessionId(workspaceRoot);
    onClearTranscript?.();
    if (isTauri() && !ephemeralChat) {
      void invoke("chat_clear")
        .then(() => setChatError(null))
        .catch((reason) => setChatError(String(reason)));
    }
  };

  useEffect(() => {
    if (!newChatNonce) return;
    clearChat();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only fire on parent + Chat
  }, [newChatNonce]);

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

  const setForceOwnedPathsPersisted = (paths: string[] | null) => {
    setForceOwnedPaths(paths);
    writeForceOwnedPaths(paths);
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
    const criteriaRaw = window.prompt(
      "Acceptance criteria (one per line; required for Apply)",
      "",
    );
    if (criteriaRaw === null) return;
    const oosRaw = window.prompt(
      "Out of scope (one per line; required for Apply)",
      "",
    );
    if (oosRaw === null) return;
    const successCriteria = splitListInput(criteriaRaw);
    const outOfScope = splitListInput(oosRaw);
    const gate =
      window.prompt(
        "Verify pointer (e.g. G3)",
        verifyGate.trim() || "G3",
      )?.trim() || "G3";
    setGoalBusy(true);
    try {
      const goal = await invoke<EngGoal>("goal_create", {
        input: {
          statement,
          successCriteria,
          outOfScope,
          shellScope,
          autonomy,
          verifyGate: gate || null,
          ownedPaths: mutating ? effectiveOwnedPaths : [],
          activate: true,
        },
      });
      setActiveGoal(goal);
      if (!isGoalContractReady(goal)) {
        setFailureNote(
          "Goal saved but Apply contract incomplete — add AC, out-of-scope, and verify (or waive).",
        );
      }
    } catch (reason) {
      window.alert(String(reason));
    } finally {
      setGoalBusy(false);
    }
  };

  const completeActiveGoalContract = async () => {
    if (!isTauri() || goalBusy) return;
    setGoalBusy(true);
    try {
      let goal = activeGoal;
      if (!goal) {
        const statement =
          prompt.trim() ||
          window.prompt("Goal statement", "")?.trim() ||
          "";
        if (!statement) {
          window.alert("Need a goal statement first.");
          return;
        }
        const criteriaRaw = window.prompt(
          "Acceptance criteria (one per line)",
          "",
        );
        if (criteriaRaw === null) return;
        const oosRaw = window.prompt("Out of scope (one per line)", "");
        if (oosRaw === null) return;
        const gate =
          window.prompt(
            "Verify pointer (e.g. G3)",
            verifyGate.trim() || "G3",
          )?.trim() || "G3";
        goal = await invoke<EngGoal>("goal_create", {
          input: {
            statement,
            successCriteria: splitListInput(criteriaRaw),
            outOfScope: splitListInput(oosRaw),
            shellScope,
            autonomy: autonomy === "observe" ? "propose" : autonomy,
            verifyGate: gate || null,
            ownedPaths: mutating ? effectiveOwnedPaths : [],
            activate: true,
          },
        });
      } else if (!isGoalContractReady(goal)) {
        const criteriaRaw = window.prompt(
          "Acceptance criteria (one per line)",
          (goal.successCriteria ?? []).join("\n"),
        );
        if (criteriaRaw === null) return;
        const oosRaw = window.prompt(
          "Out of scope (one per line)",
          (goal.outOfScope ?? []).join("\n"),
        );
        if (oosRaw === null) return;
        const gate =
          window.prompt(
            "Verify pointer (e.g. G3)",
            goal.verifyGate?.trim() || verifyGate.trim() || "G3",
          )?.trim() || "G3";
        goal = await invoke<EngGoal>("goal_update_contract", {
          id: goal.id,
          successCriteria: splitListInput(criteriaRaw),
          outOfScope: splitListInput(oosRaw),
          verifyGate: gate || null,
          clarifyResolutions: null,
        });
      }
      setActiveGoal(goal);
      if (!isGoalContractReady(goal)) {
        window.alert(
          "Contract still incomplete. Fill AC, out-of-scope, and verify — or waive.",
        );
      }
    } catch (reason) {
      window.alert(String(reason));
    } finally {
      setGoalBusy(false);
    }
  };

  const waiveActiveGoalContract = async () => {
    if (!activeGoal || !isTauri() || goalBusy) return;
    const reason = window.prompt(
      "Waive Apply contract — reason (logged)",
      "dogfood / emergency",
    );
    if (reason === null || !reason.trim()) return;
    setGoalBusy(true);
    try {
      const goal = await invoke<EngGoal>("goal_waive_contract", {
        id: activeGoal.id,
        reason: reason.trim(),
      });
      setActiveGoal(goal);
      setFailureNote("Contract waived — Apply tools unlocked for this goal.");
    } catch (reason) {
      window.alert(String(reason));
    } finally {
      setGoalBusy(false);
    }
  };

  const requestApplyAutonomy = (next: "act" | "automate") => {
    // Mode switch stays prompt-free. Contract gaps surface on the turn as
    // contract_gate CTAs (Define goal / Waive) instead of blocking the dial.
    setAutonomyPersisted(next);
    if (next === "automate") setVerifyOnComplete(true);
    if (effort === "low") {
      setEffort("medium");
      window.localStorage.setItem(AGENT_EFFORT_KEY, "medium");
    }
    if (!isGoalContractReady(activeGoal)) {
      setFailureNote(
        "Apply on — writes still need an eng-goal contract (or waive) when Act tools run.",
      );
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
        surfaceLeaseConflict(conflict);
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
      approvedRiskCategories,
      approvedRiskTiers,
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
          autonomy: autonomy === "automate" ? "automate" : "act",
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
          approvedRiskCategories,
          approvedRiskTiers,
          claimedTaskId: working.id,
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
    window.localStorage.setItem(AGENT_MODEL_MODE_KEY, modelMode);
  }, [modelMode]);
  useEffect(() => {
    if (modelMode !== "auto") return;
    const picked = autoModelForSlot(provider, slotFromAutonomy(autonomy));
    if (picked.model !== model) {
      setModel(picked.model);
    }
  }, [modelMode, provider, autonomy, model]);
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
    slotOverride?: string | null;
  }) => {
    const nextAutonomy = overrides?.autonomy ?? autonomy;
    const resolvedEffort = effectiveEffort(
      nextAutonomy,
      overrides?.effort ?? effort,
      overrides?.context ?? "normal",
    );
    const effortOpts = EFFORT_OPTIONS.find((item) => item.id === resolvedEffort);
    const slot = slotFromAutonomy(nextAutonomy, overrides?.slotOverride);
    const routed =
      modelMode === "auto" ? autoModelForSlot(provider, slot) : null;
    const resolvedModel = routed?.model ?? model.trim();
    return {
    prompt: prompt.trim(),
    provider: provider.trim(),
    baseUrl: baseUrl.trim(),
    model: resolvedModel,
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
    approvedRiskCategories,
    approvedRiskTiers,
    claimedTaskId:
      claimedTask &&
      (claimedTask.status === "claimed" || claimedTask.status === "running")
        ? claimedTask.id
        : null,
    imagePaths: lastImagePathsRef.current,
  };
  };

  const submit = () => {
    const text = prompt.trim();
    if (!text && attachments.length === 0) return;
    if (!provider.trim() || !model.trim() || !isTauri()) return;
    if (mutating) {
      const conflict = writableConflict(
        leases as StripLease[],
        agentId,
        effectiveOwnedPaths,
      );
      if (conflict) {
        surfaceLeaseConflict(conflict);
        return;
      }
    }
    const imagePaths = attachments
      .filter((item) => item.kind === "image")
      .map((item) => item.absolute ?? item.path)
      .filter((path) => Boolean(path.trim()));
    lastImagePathsRef.current = imagePaths;
    const vision = modelSupportsVision(model);
    const packed = packagePromptWithAttachments(text, attachments, {
      visionCapable: vision,
    });
    pendingTurnAttachmentsRef.current = attachments.map(toAttachmentMeta);
    setPrompt("");
    setAttachments([]);
    setAttachNote(null);
    setMentionOpen(false);
    if (imagePaths.length > 0 && !vision) {
      onSurfaceFailure?.(
        `vision_required: model \`${model}\` does not support images. Switch to a vision-capable model.`,
      );
      return;
    }
    if (busy) {
      setPromptQueue((queue) => [
        ...queue,
        {
          id:
            typeof crypto !== "undefined" && "randomUUID" in crypto
              ? crypto.randomUUID()
              : `q-${Date.now()}-${queue.length}`,
          prompt: packed,
          imagePaths,
        },
      ]);
      renameTabFromPrompt(packed);
      return;
    }
    lastPromptRef.current = packed;
    setCurrentUser(packed);
    stickToBottomRef.current = true;
    renameTabFromPrompt(packed);
    onRun({ ...buildTurnInput(), prompt: packed, imagePaths });
  };

  useEffect(() => {
    if (busy) {
      queueDrainLockRef.current = false;
      return;
    }
    if (queueDrainLockRef.current) return;
    const queue = promptQueueRef.current;
    if (queue.length === 0) return;
    queueDrainLockRef.current = true;
    const next = queue[0];
    setPromptQueue((current) => current.slice(1));
    lastPromptRef.current = next.prompt;
    lastImagePathsRef.current = next.imagePaths;
    setCurrentUser(next.prompt);
    stickToBottomRef.current = true;
    renameTabFromPrompt(next.prompt);
    onRun({
      ...buildTurnInput(),
      prompt: next.prompt,
      imagePaths: next.imagePaths,
    });
    // Drain once when a turn ends; buildTurnInput/onRun identity must not re-fire.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [busy]);

  const addAttachments = useCallback(async (files: FileList | File[]) => {
    setAttachBusy(true);
    try {
      const { attachments: next, errors } = await ingestFiles(files, attachments.length);
      if (next.length > 0) {
        setAttachments((current) => [...current, ...next].slice(0, 8));
      }
      setAttachNote(errors.length > 0 ? errors.join(" · ") : null);
    } finally {
      setAttachBusy(false);
    }
  }, [attachments.length]);

  const ingestDroppedOrPastedText = useCallback(
    (text: string) => {
      if (looksLikeHttpUrl(text)) {
        const { attachments: next, errors } = ingestUrlText(text, attachments.length);
        if (next.length > 0) {
          setAttachments((current) => [...current, ...next].slice(0, 8));
        }
        setAttachNote(errors.length > 0 ? errors.join(" · ") : null);
        return true;
      }
      if (!looksLikeFilesystemPath(text)) return false;
      void (async () => {
        const { attachments: next, errors } = await ingestPathText(
          text,
          attachments.length,
        );
        if (next.length > 0) {
          setAttachments((current) => [...current, ...next].slice(0, 8));
        }
        setAttachNote(
          errors.length > 0
            ? errors.join(" · ")
            : next.length === 0
              ? "Could not attach that path"
              : null,
        );
      })();
      return true;
    },
    [attachments.length],
  );

  const onComposerDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setComposerDragOver(false);
    setFeedDragOver(false);
    const files = event.dataTransfer?.files;
    if (files?.length) {
      void addAttachments(files);
      return;
    }
    const uri =
      event.dataTransfer?.getData("text/uri-list") ||
      event.dataTransfer?.getData("text/plain") ||
      "";
    if (uri.trim()) ingestDroppedOrPastedText(uri.trim().split("\n")[0] ?? uri);
  };

  const onComposerPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = event.clipboardData?.files;
    if (files?.length) {
      event.preventDefault();
      void addAttachments(files);
      return;
    }
    const text = event.clipboardData?.getData("text/plain") ?? "";
    if (looksLikeHttpUrl(text) || looksLikeFilesystemPath(text)) {
      event.preventDefault();
      ingestDroppedOrPastedText(text);
    }
  };

  const fetchAttachment = async (item: ChatAttachment) => {
    setAttachBusy(true);
    try {
      const result = await fetchUrlAttachment(item);
      if (result.ok) {
        setAttachments((current) =>
          current.map((row) => (row.id === item.id ? result.attachment : row)),
        );
        setAttachNote(null);
      } else {
        setAttachNote(result.error);
      }
    } finally {
      setAttachBusy(false);
    }
  };

  const mentionItems = useMemo((): MentionItem[] => {
    const q = mentionQuery.trim().toLowerCase();
    const pathItems: MentionItem[] = mentionPaths
      .filter((path) => !q || path.toLowerCase().includes(q))
      .slice(0, 20)
      .map((path) => ({
        id: `path:${path}`,
        kind: "path" as const,
        insert: `@${path}`,
        label: path,
        detail: "Workspace path",
      }));
    const toolItems: MentionItem[] = mcpTools
      .filter((tool) => {
        if (!q) return true;
        const hay = `${tool.server}/${tool.name} ${tool.description}`.toLowerCase();
        return hay.includes(q);
      })
      .slice(0, 20)
      .map((tool) => ({
        id: `mcp:${tool.server}/${tool.name}`,
        kind: "mcp" as const,
        insert: `@mcp:${tool.server}/${tool.name}`,
        label: `${tool.server}/${tool.name}`,
        detail: tool.description?.slice(0, 80) || "MCP tool",
      }));
    return [...pathItems, ...toolItems].slice(0, 24);
  }, [mentionPaths, mentionQuery, mcpTools]);

  const closeMention = () => {
    setMentionOpen(false);
    setMentionQuery("");
    setMentionIndex(0);
    mentionRangeRef.current = null;
  };

  const applyMention = (item: MentionItem) => {
    const range = mentionRangeRef.current;
    const el = composerRef.current;
    if (!range || !el) {
      setPrompt((prev) => `${prev}${item.insert} `);
      closeMention();
      return;
    }
    const before = prompt.slice(0, range.start);
    const after = prompt.slice(range.end);
    const next = `${before}${item.insert} ${after}`;
    setPrompt(next);
    closeMention();
    requestAnimationFrame(() => {
      const pos = before.length + item.insert.length + 1;
      el.focus();
      el.setSelectionRange(pos, pos);
    });
  };

  const syncMentionFromPrompt = (value: string, cursor: number) => {
    const before = value.slice(0, cursor);
    const at = before.lastIndexOf("@");
    if (at < 0) {
      closeMention();
      return;
    }
    if (at > 0 && !/\s/.test(before[at - 1] ?? " ")) {
      closeMention();
      return;
    }
    const frag = before.slice(at + 1);
    if (frag.includes("\n") || frag.length > 64) {
      closeMention();
      return;
    }
    mentionRangeRef.current = { start: at, end: cursor };
    setMentionQuery(frag);
    setMentionOpen(true);
    setMentionIndex(0);
    if (isTauri()) {
      void invoke<string[]>("workspace_mention_candidates", {
        query: frag.replace(/^mcp:/i, ""),
        limit: 40,
      })
        .then((paths) => setMentionPaths(paths ?? []))
        .catch(() => setMentionPaths(["AGENTS.md", "README.md"]));
    } else {
      setMentionPaths(["AGENTS.md", "README.md"]);
    }
  };

  const pickAttachments = async (opts?: { folder?: boolean }) => {
    setAttachBusy(true);
    try {
      let { attachments: next, errors } = opts?.folder
        ? await pickAttachmentFolder()
        : await pickAttachmentFiles();
      // Native dialog ACL missing → fall back to HTML file input (files only).
      if (
        !opts?.folder &&
        next.length === 0 &&
        errors.some((e) => /not allowed|permission|failed/i.test(e))
      ) {
        const fallback = await pickAttachmentFilesViaInput();
        next = fallback.attachments;
        errors = [...errors, ...fallback.errors];
      }
      if (next.length > 0) {
        setAttachments((current) => [...current, ...next].slice(0, 8));
      }
      setAttachNote(errors.length > 0 ? errors.join(" · ") : null);
    } catch (reason) {
      if (opts?.folder) {
        setAttachNote(String(reason));
        return;
      }
      try {
        const fallback = await pickAttachmentFilesViaInput();
        if (fallback.attachments.length > 0) {
          setAttachments((current) =>
            [...current, ...fallback.attachments].slice(0, 8),
          );
        }
        setAttachNote(
          [String(reason), ...fallback.errors].filter(Boolean).join(" · ") ||
            null,
        );
      } catch (inner) {
        setAttachNote(String(inner || reason));
      }
    } finally {
      setAttachBusy(false);
    }
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
      const turnAttachments = pendingTurnAttachmentsRef.current;
      pendingTurnAttachmentsRef.current = [];
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
          attachments: turnAttachments.length > 0 ? turnAttachments : undefined,
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
            setForceOwnedPathsPersisted(null);
            setActiveWorktree(null);
            onRefresh?.();
          })
          .catch(() => {});
      }
      return;
    }
    if (!terminal) return;
    const turnAttachments = pendingTurnAttachmentsRef.current;
    pendingTurnAttachmentsRef.current = [];
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
        attachments: turnAttachments.length > 0 ? turnAttachments : undefined,
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
            setForceOwnedPathsPersisted(null);
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
            setForceOwnedPathsPersisted(null);
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
  const contextSegments = useMemo(() => {
    const estimate = (text: string) =>
      Math.max(0, Math.round(text.replace(/\s+/g, " ").trim().length / 4));
    let conversation = 0;
    let tools = 0;
    for (const turn of pastTurns) {
      conversation += estimate(turn.user);
      for (const event of turn.events) {
        if (event.type === "text_delta") conversation += estimate(event.text);
        if (event.type === "tool_call" || event.type === "tool_result") {
          tools += 120;
        }
      }
    }
    if (currentUser) conversation += estimate(currentUser);
    for (const event of events) {
      if (event.type === "text_delta") conversation += estimate(event.text);
      if (event.type === "tool_call" || event.type === "tool_result") {
        tools += 120;
      }
    }
    const draft = estimate(prompt);
    const lastIn = turnTokens.input;
    const lastOut = turnTokens.output;
    // When provider usage exists, prefer it for the live window; else char estimates.
    const rows: { label: string; tokens: number; color: string }[] = [];
    if (lastIn > 0) {
      const systemish = Math.min(lastIn, Math.max(400, Math.round(lastIn * 0.15)));
      const rest = Math.max(0, lastIn - systemish - tools);
      rows.push({ label: "System & rules", tokens: systemish, color: "#94a3b8" });
      if (tools > 0) {
        rows.push({
          label: "Tools",
          tokens: Math.min(tools, lastIn),
          color: "#a78bfa",
        });
      }
      rows.push({
        label: "Conversation",
        tokens: Math.max(rest, conversation || draft),
        color: "#38bdf8",
      });
      if (lastOut > 0) {
        rows.push({ label: "Last reply", tokens: lastOut, color: "#f472b6" });
      }
    } else {
      if (draft > 0) {
        rows.push({ label: "Draft", tokens: draft, color: "#64748b" });
      }
      if (conversation > 0) {
        rows.push({ label: "Conversation", tokens: conversation, color: "#38bdf8" });
      }
      if (tools > 0) {
        rows.push({ label: "Tools", tokens: tools, color: "#a78bfa" });
      }
      if (rows.length === 0) {
        rows.push({ label: "Empty", tokens: 0, color: "#334155" });
      }
    }
    return rows;
  }, [pastTurns, currentUser, events, prompt, turnTokens]);

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
    const nextPrompt = (initialPrompt.trim() || prompt).trim();
    // Wait while another turn is running — do not clear the Continuity kick.
    if (busy) return;
    if (!nextPrompt || !provider.trim() || !model.trim() || !isTauri()) {
      onAutoSubmitHandled?.();
      return;
    }
    onAutoSubmitHandled?.();
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
    lastPromptRef.current = nextPrompt;
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
    // Re-run when busy clears so Continuity autoSubmit is not lost mid-turn.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoSubmit, busy]);

  const showTranscript =
    historyOpen || busy || Boolean(currentUser) || events.length > 0;
  const lastTurn = pastTurns[pastTurns.length - 1];
  const lastTurnFailed = Boolean(
    lastTurn &&
      [...lastTurn.events]
        .reverse()
        .some((event) => event.type === "failed"),
  );
  const lastTurnTitle = (lastTurn?.user ?? "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 80);
  const canvasSubtitle = ephemeralChat
    ? "New session — ask ADE anything."
    : pastTurns.length > 0
      ? "Start fresh below, or open the last run when you need it."
      : "Ask ADE to plan or build — your turns will show up here.";

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

      {showTranscript && activeGoal && (
        <Disclosure
          title="Active goal"
          subtitle={`${activeGoal.shellScope === "home" ? "Home" : "Workspace"} · ${
            activeGoal.autonomy === "act"
              ? "Apply"
              : activeGoal.autonomy === "automate"
                ? "Automate"
                : "Suggest"
          }${activeGoal.verifyGate ? ` · ${activeGoal.verifyGate}` : ""} · ${
            isGoalContractReady(activeGoal) ? "contract ready" : "contract incomplete"
          }`}
          summary={
            activeGoal.statement.length > 48
              ? `${activeGoal.statement.slice(0, 48)}…`
              : activeGoal.statement
          }
          defaultOpen={false}
          forceOpen={!isGoalContractReady(activeGoal)}
          storageKey="ade_agent_active_goal"
          className="shrink-0 border-emerald-400/20 bg-emerald-500/8"
        >
          <div className="flex flex-wrap items-center justify-between gap-2 px-4 pb-3 sm:px-5">
            <div className="min-w-0">
              <div className="truncate text-[11px] text-slate-300" title={activeGoal.statement}>
                {activeGoal.statement}
              </div>
            </div>
            <div className="flex shrink-0 flex-wrap items-center gap-1.5">
              {!isGoalContractReady(activeGoal) && (
                <>
                  <button
                    type="button"
                    disabled={goalBusy}
                    onClick={() => void completeActiveGoalContract()}
                    className="rounded-md border border-amber-400/35 bg-amber-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-amber-100 hover:bg-amber-500/25 disabled:opacity-40"
                  >
                    Complete contract
                  </button>
                  <button
                    type="button"
                    disabled={goalBusy}
                    onClick={() => void waiveActiveGoalContract()}
                    className="rounded-md border border-white/10 bg-white/5 px-2.5 py-1.5 text-[11px] font-semibold text-slate-300 hover:bg-white/8 disabled:opacity-40"
                    title="Log a waive so Apply can proceed without full AC/OOS/verify"
                  >
                    Waive
                  </button>
                </>
              )}
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
        </Disclosure>
      )}

      {showTranscript &&
        (planPhaseCount > 0 || openTasks.length > 0 || claimedTask || taskNote) && (
        <Disclosure
          title="Tasks"
          subtitle="Suggest queues · Apply claims one"
          summary={
            openTasks.length > 0
              ? `${openTasks.length} open`
              : claimedTask
                ? "claimed"
                : planPhaseCount > 0
                  ? `${planPhaseCount} phase${planPhaseCount === 1 ? "" : "s"}`
                  : undefined
          }
          defaultOpen={false}
          forceOpen={openTasks.length > 0 || Boolean(claimedTask)}
          storageKey="ade_agent_tasks"
          className="shrink-0 border-violet-400/20 bg-violet-500/8"
        >
          <div className="space-y-2 px-4 pb-3 sm:px-5">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="min-w-0">
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
        </Disclosure>
      )}

      {showTranscript && handoffAvailable && onContinueHandoff && (
        handoffLatestStatus === "budget_exhausted" ? (
        <Disclosure
          title="Continuity"
          subtitle="Host runs next_safe_command, then resumes at Med+ Effort (Apply)."
          summary={continuityBusy ? "preparing" : "budget exhausted"}
          defaultOpen={false}
          forceOpen
          storageKey="ade_agent_continuity"
          className="shrink-0 border-amber-400/25 bg-amber-500/8"
        >
          <div className="flex flex-wrap items-center justify-between gap-2 px-4 pb-3 sm:px-5">
            <div className="min-w-0 text-[10px] text-slate-500">
              {continuityBusy
                ? "Preparing Continuity…"
                : busy
                  ? "Handoff ready — wait for turn"
                  : "Budget exhausted — continue with more Effort"}
            </div>
            <button
              type="button"
              disabled={continuityBusy || busy}
              onClick={onContinueHandoff}
              className="shrink-0 rounded-md border border-amber-400/35 bg-amber-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-amber-100 hover:bg-amber-500/25 disabled:opacity-40"
            >
              {continuityBusy ? "Working…" : "Continue · raise Effort"}
            </button>
          </div>
        </Disclosure>
        ) : (
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 rounded-xl border border-blue-400/20 bg-blue-500/8 px-3 py-2">
          <div className="min-w-0 text-[10px] text-slate-400">
            {continuityBusy
              ? "Preparing Continuity…"
              : busy
                ? "Handoff ready — wait for turn"
                : "Continue last handoff"}
          </div>
          <button
            type="button"
            disabled={continuityBusy || busy}
            onClick={onContinueHandoff}
            className="shrink-0 rounded-md border border-blue-400/30 bg-blue-500/15 px-2.5 py-1.5 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/25 disabled:opacity-40"
          >
            {continuityBusy ? "Working…" : "Continue"}
          </button>
        </div>
        )
      )}

      {showTranscript && (
      <AgentSessionStrip
        agentId={agentId}
        mutating={mutating}
        leases={leases as StripLease[]}
        ownedPaths={mutating ? (effectiveOwnedPaths) : []}
        busy={busy}
        shellScope={shellScope}
        compact
        onNewLease={() => setAgentId(rotateAgentId(workspaceRoot))}
        onNewChat={clearChat}
        onWaitRefresh={() => onRefresh?.()}
        onEnableIsolate={() => {
          setApplyIsolate(true);
          window.localStorage.setItem(APPLY_ISOLATE_KEY, "1");
        }}
        onSwitchSuggest={() => setAutonomyPersisted("propose")}
        isolateEnabled={applyIsolate}
      />
      )}

      <div
        ref={feedScrollRef}
        onScroll={onFeedScroll}
        className={`thin-scrollbar min-h-0 flex-1 overflow-y-auto scroll-smooth ${
          feedDragOver ? "ring-1 ring-inset ring-accent/40" : ""
        }`}
        onDragEnter={(event) => {
          event.preventDefault();
          setFeedDragOver(true);
        }}
        onDragOver={(event) => {
          event.preventDefault();
          setFeedDragOver(true);
        }}
        onDragLeave={(event) => {
          if (event.currentTarget.contains(event.relatedTarget as Node)) return;
          setFeedDragOver(false);
        }}
        onDrop={onComposerDrop}
      >
        {showTranscript ? (
          <>
        {(pastTurns.length > 0 || chatError) && (
          <div className="mb-2 flex flex-wrap items-center justify-between gap-2 px-0.5">
            <div className="text-[10px] text-slate-600">
              {chatError
                ? `Chat save: ${chatError}`
                : `${pastTurns.length} saved turn${pastTurns.length === 1 ? "" : "s"} · .ade/chat`}
            </div>
            {!busy && !currentUser && pastTurns.length > 0 && (
              <button
                type="button"
                onClick={() => setHistoryOpen(false)}
                className="rounded px-1.5 py-0.5 text-[10px] font-semibold text-slate-500 hover:bg-white/5 hover:text-slate-300"
              >
                Hide history
              </button>
            )}
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
              ? "Apply · Worker"
              : autonomy === "automate"
                ? "Automate · Worker"
                : autonomy === "observe"
                  ? "Observe · Planner"
                  : "Suggest · Planner"
          }
          scopeLabel={shellScope === "home" ? "Home" : "Workspace"}
          autonomySuggest={autonomy === "propose" || autonomy === "observe"}
          onSwitchToApply={() => requestApplyAutonomy("act")}
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
                surfaceLeaseConflict(conflict);
                return;
              }
            }
            setPrompt("");
            lastPromptRef.current = trimmed;
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
          </>
        ) : (
          <div
            className="flex min-h-full flex-col items-center justify-center px-4 py-8"
            data-testid="ade-home-canvas"
          >
            <div className="w-full max-w-lg text-center">
              <h2 className="text-2xl font-semibold tracking-tight text-ink sm:text-3xl">
                ADE
              </h2>
              <p className="mt-2 text-sm leading-6 text-ink-dim">
                {canvasSubtitle}
              </p>
              {guidedWins && onUnderstand && onVerifyHome && (
                <GettingStartedChecklist
                  className="mt-6"
                  steps={buildGettingStartedSteps({
                    understand: guidedWins.understand,
                    verify: guidedWins.verify,
                    improveAde: guidedWins.improve_ade,
                    isDogfood,
                    understandBusy,
                    verifying,
                    improveBusy: busy,
                    onUnderstand,
                    onVerify: onVerifyHome,
                    onImprove: onImproveAde,
                    keysTrailing: onOpenKeys ? (
                      <button
                        type="button"
                        title="Keys"
                        onClick={(event) => {
                          event.stopPropagation();
                          onOpenKeys();
                        }}
                        className="inline-flex rounded-md p-0.5 hover:bg-white/5"
                      >
                        <BrandWell id="keys" size="sm" status="info" />
                      </button>
                    ) : undefined,
                    improveTrailing: onOpenIntegrations ? (
                      <button
                        type="button"
                        title="Integrations"
                        onClick={(event) => {
                          event.stopPropagation();
                          onOpenIntegrations();
                        }}
                        className="inline-flex rounded-md p-0.5 hover:bg-white/5"
                      >
                        <BrandWell
                          id="github"
                          size="sm"
                          status="info"
                          title="Open Integrations"
                        />
                      </button>
                    ) : undefined,
                  })}
                />
              )}
              {pastTurns.length > 0 && lastTurnTitle && (
                <div className="mt-6 rounded-xl border border-line bg-surface-2 px-4 py-3 text-left">
                  <div className="flex flex-wrap items-start justify-between gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="text-[10px] font-semibold uppercase tracking-wider text-ink-faint">
                        Last run
                      </div>
                      <div
                        className="mt-1 truncate text-[13px] font-medium text-ink"
                        title={lastTurn?.user}
                      >
                        {lastTurnTitle}
                      </div>
                      <div className="mt-1.5 flex flex-wrap items-center gap-2">
                        <span
                          className={`inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium ${
                            lastTurnFailed
                              ? "bg-danger/15 text-red-200"
                              : "bg-ready/10 text-emerald-300"
                          }`}
                        >
                          <span
                            className={`size-1.5 rounded-full ${
                              lastTurnFailed ? "bg-danger" : "bg-ready"
                            }`}
                            aria-hidden
                          />
                          {lastTurnFailed ? "Failed" : "Done"}
                        </span>
                        <span className="text-[10px] text-ink-faint">
                          {pastTurns.length} turn
                          {pastTurns.length === 1 ? "" : "s"} saved
                        </span>
                      </div>
                    </div>
                    <div className="flex shrink-0 flex-wrap items-center gap-1.5">
                      <button
                        type="button"
                        onClick={() => {
                          setHistoryOpen(true);
                          stickToBottomRef.current = true;
                        }}
                        className="rounded-md border border-line bg-surface-3 px-2.5 py-1.5 text-[11px] font-semibold text-ink hover:bg-white/8"
                      >
                        Show history
                      </button>
                      {handoffAvailable && onContinueHandoff && (
                        <button
                          type="button"
                          disabled={continuityBusy || busy}
                          onClick={() => {
                            setHistoryOpen(true);
                            onContinueHandoff();
                          }}
                          className="rounded-md bg-accent px-2.5 py-1.5 text-[11px] font-semibold text-white hover:bg-blue-400 disabled:opacity-40"
                        >
                          {continuityBusy ? "Working…" : "Continue"}
                        </button>
                      )}
                      <button
                        type="button"
                        disabled={busy}
                        onClick={clearChat}
                        title="Clear saved turns and start empty"
                        className="rounded-md px-2 py-1.5 text-[11px] font-semibold text-ink-faint hover:bg-white/5 hover:text-ink-dim disabled:opacity-40"
                      >
                        New chat
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {showTranscript && showDebugHarness && (
        <Disclosure
          title="Harness"
          subtitle="Dogfood · Verify · Observe/Automate · rates"
          summary={
            autonomy === "observe" || autonomy === "automate"
              ? autonomy
              : verifyOnComplete
                ? `verify ${verifyGate}`
                : "collapsed"
          }
          defaultOpen={false}
          forceOpen={forceAdvancedOpen}
          storageKey="ade_agent_harness"
          className="shrink-0"
        >
          <div className="space-y-2 px-1 pb-1">
            <div className="flex flex-wrap gap-1.5">
              <button
                type="button"
                disabled={busy || !provider.trim() || !model.trim() || !isTauri()}
                onClick={() => {
                  void invoke<VerifyResult[]>("run_verify", {
                    gate: verifyGate.trim() || "G3",
                    through: true,
                  })
                    .then((results) => {
                      setTaskNote(
                        `Verify G${verifyGate.trim() || "G3"} · ${results.filter((r) => r.passed).length}/${results.length} passed`,
                      );
                    })
                    .catch((reason) => setTaskNote(honestLeaseError(String(reason))));
                  const text =
                    "Verifier (judge): grade evidence only. Summarize what sensors prove; do not propose patches or write files. List pass/fail and next safe command.";
                  setPrompt("");
                  setCurrentUser(text);
                  stickToBottomRef.current = true;
                  onRun({
                    ...buildTurnInput({
                      autonomy: "propose",
                      slotOverride: "verifier",
                    }),
                    prompt: text,
                    autonomy: "propose",
                    approveOwnedPaths: false,
                    ownedPaths: [],
                    leaseAgentId: null,
                    slotOverride: "verifier",
                    claimedTaskId: null,
                  });
                }}
                className="rounded-md border border-emerald-400/30 bg-emerald-500/10 px-2 py-1 text-[10px] font-semibold text-emerald-100 hover:bg-emerald-500/20 disabled:opacity-40"
                title="Sensors-first Verifier slot — no write leases"
              >
                Verify (judge)
              </button>
            </div>
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
                    setForceOwnedPathsPersisted(null);
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
              <Chip
                onClick={() => {
                  setAutonomyPersisted("act");
                  if (effort === "low") {
                    setEffort("medium");
                    window.localStorage.setItem(AGENT_EFFORT_KEY, "medium");
                  }
                  setForceOwnedPaths([".ade/dogfood"]);
                  setPrompt(
                    "N4 dogfood Continuity: after a turn under .ade/dogfood/, use Continue last handoff (or raise Effort) and append continuity-acceptance.md with ISO time and that Continuity resumed. Do not edit crates/ or apps/.",
                  );
                }}
              >
                Dogfood Continuity
              </Chip>
              <Chip
                onClick={() => {
                  setApplyIsolate(true);
                  window.localStorage.setItem(APPLY_ISOLATE_KEY, "1");
                  setAutonomyPersisted("act");
                  setForceOwnedPaths([".ade/dogfood/isolate"]);
                  setPrompt(
                    "G4 dogfood Isolate: Apply under .ade/dogfood/isolate with Isolate enabled. Write isolate-acceptance.md noting worktree path if used. Do not edit crates/ or apps/.",
                  );
                }}
              >
                Dogfood Isolate
              </Chip>
            </div>
            <ProviderSelect
              value={provider}
              showRecommended={false}
              onChange={(preset) => {
                setProvider(preset.id);
                setBaseUrl(
                  preset.custom
                    ? baseUrl.trim() || preset.baseUrl
                    : preset.baseUrl,
                );
                if (modelMode === "pin" && !preset.custom) {
                  setModel(firstModelId(preset));
                }
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
            {Number(inputCost) <= 0 &&
              Number(outputCost) <= 0 &&
              (Number(sessionCap) > 0 || Number(dailyCap) > 0) && (
                <p className="text-[10px] leading-snug text-amber-200/90">
                  Spend honesty: rates are $0 — caps cannot reserve real dollars. Set
                  Input/Output $/MTok to match your provider invoice class.
                </p>
              )}
          </div>
        </Disclosure>
      )}

      <Disclosure
        title="Tools"
        subtitle="What this turn may call — host FS/shell/web plus connected MCP"
        summary={
          mcpServerNames.length > 0
            ? `${mcpServerNames.length} MCP · ${connectedTools} tools · ${shellScope === "home" ? "Home" : "Workspace"} shell`
            : `Host tools · ${shellScope === "home" ? "Home" : "Workspace"} shell`
        }
        hint="Integrations are standing connectors (GitHub, Stripe, …). Tools are what the model can invoke this turn."
        storageKey="ade_composer_tools"
        className="shrink-0"
      >
        <div className="flex flex-wrap items-center gap-2 pt-1">
          <span className="rounded-md border border-white/10 bg-white/4 px-2 py-1 text-[10px] text-slate-300">
            FS · Shell · Web
          </span>
          <span className="rounded-md border border-white/10 bg-white/4 px-2 py-1 text-[10px] text-slate-300">
            Shell: {shellScope === "home" ? "Home" : "Workspace"}
          </span>
          {mcpServerNames.length > 0 ? (
            mcpServerNames.map((name) => (
              <span
                key={name}
                className="rounded-md border border-violet-400/25 bg-violet-500/10 px-2 py-1 text-[10px] text-violet-100"
                title={`${connectedTools} MCP tool${connectedTools === 1 ? "" : "s"} total`}
              >
                MCP · {name}
              </span>
            ))
          ) : (
            <span className="rounded-md border border-white/10 bg-white/4 px-2 py-1 text-[10px] text-slate-400">
              No MCP connected
            </span>
          )}
          {onOpenIntegrations && (
            <button
              type="button"
              onClick={onOpenIntegrations}
              className="rounded-md border border-blue-400/30 bg-blue-500/15 px-2 py-1 text-[10px] font-semibold text-blue-100 hover:bg-blue-500/25"
            >
              Integrations
            </button>
          )}
          {onOpenKeys && (
            <button
              type="button"
              onClick={onOpenKeys}
              className="rounded-md border border-white/10 px-2 py-1 text-[10px] font-semibold text-slate-300 hover:bg-white/5"
            >
              Keys
            </button>
          )}
        </div>
      </Disclosure>

      <div
        className={`relative shrink-0 rounded-2xl border bg-[#141a22] ${
          composerDragOver
            ? "border-cyan-400/40 bg-cyan-500/5"
            : "border-white/10"
        }`}
        onDragEnter={(event) => {
          event.preventDefault();
          setComposerDragOver(true);
        }}
        onDragOver={(event) => {
          event.preventDefault();
          setComposerDragOver(true);
        }}
        onDragLeave={(event) => {
          if (event.currentTarget.contains(event.relatedTarget as Node)) return;
          setComposerDragOver(false);
        }}
        onDrop={onComposerDrop}
      >
        {mentionOpen && (
          <div className="pointer-events-auto absolute inset-x-3 bottom-[calc(100%-0.25rem)] z-30">
            <MentionPalette
              items={mentionItems}
              activeIndex={mentionIndex}
              onActiveIndexChange={setMentionIndex}
              onPick={applyMention}
              onClose={closeMention}
            />
          </div>
        )}
        {(attachments.length > 0 || attachNote) && (
          <div className="px-3 pt-3">
            <AttachmentChips
              items={attachments}
              onRemove={(id) =>
                setAttachments((current) => current.filter((item) => item.id !== id))
              }
              onClearAll={() => {
                setAttachments([]);
                setAttachNote(null);
              }}
              onOpen={(item) => void openChatPath(item.absolute ?? item.path)}
              onFetch={(item) => void fetchAttachment(item)}
            />
            {attachNote && (
              <p className="mb-2 text-[10px] leading-4 text-amber-200/90">{attachNote}</p>
            )}
          </div>
        )}
        {promptQueue.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5 px-3 pt-2">
            <span className="text-[10px] font-semibold uppercase tracking-wide text-slate-500">
              Queued
            </span>
            {promptQueue.map((item, index) => (
              <span
                key={item.id}
                className="inline-flex max-w-[14rem] items-center gap-1 rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-[11px] text-slate-300"
                title={item.prompt}
              >
                <span className="shrink-0 text-slate-500">{index + 1}.</span>
                <span className="truncate">
                  {item.prompt.replace(/\s+/g, " ").slice(0, 48) || "(attachment)"}
                </span>
                <button
                  type="button"
                  title="Remove from queue"
                  onClick={() =>
                    setPromptQueue((queue) =>
                      queue.filter((entry) => entry.id !== item.id),
                    )
                  }
                  className="shrink-0 text-slate-500 hover:text-slate-200"
                >
                  ×
                </button>
              </span>
            ))}
            {promptQueue.length > 1 && (
              <button
                type="button"
                title="Clear queue"
                onClick={() => setPromptQueue([])}
                className="text-[10px] text-slate-500 hover:text-slate-300"
              >
                Clear
              </button>
            )}
          </div>
        )}
        <textarea
          ref={composerRef}
          value={prompt}
          onChange={(event) => {
            const value = event.target.value;
            setPrompt(value);
            syncMentionFromPrompt(
              value,
              event.target.selectionStart ?? value.length,
            );
          }}
          onClick={(event) => {
            const el = event.currentTarget;
            syncMentionFromPrompt(el.value, el.selectionStart ?? el.value.length);
          }}
          onPaste={onComposerPaste}
          onKeyDown={(event) => {
            if (mentionOpen) {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setMentionIndex((i) =>
                  mentionItems.length === 0 ? 0 : (i + 1) % mentionItems.length,
                );
                return;
              }
              if (event.key === "ArrowUp") {
                event.preventDefault();
                setMentionIndex((i) =>
                  mentionItems.length === 0
                    ? 0
                    : (i - 1 + mentionItems.length) % mentionItems.length,
                );
                return;
              }
              if (event.key === "Enter" || event.key === "Tab") {
                const item = mentionItems[mentionIndex];
                if (item) {
                  event.preventDefault();
                  applyMention(item);
                  return;
                }
              }
              if (event.key === "Escape") {
                event.preventDefault();
                closeMention();
                return;
              }
            }
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              if ((prompt.trim() || attachments.length > 0) && isTauri()) {
                submit();
              }
            }
            if (event.key === "Escape" && busy) {
              event.preventDefault();
              onCancel?.();
            }
          }}
          rows={3}
          className="min-h-16 w-full resize-none border-0 bg-transparent px-4 pt-3 text-[14px] leading-5 text-slate-200 outline-hidden placeholder:text-slate-500"
          placeholder={
            busy
              ? "Add to queue… (Enter) · Esc to stop"
              : "Ask ADE… @path or @mcp · drop/paste files or URLs"
          }
        />
        <div className="flex items-center gap-1.5 px-3 pb-3 pt-1">
          <button
            type="button"
            title="Attach files (Alt+click = folder)"
            disabled={attachBusy || !isTauri()}
            onClick={(event) =>
              void pickAttachments({ folder: event.altKey })
            }
            className="grid size-8 shrink-0 place-items-center rounded-md border border-white/10 text-slate-400 hover:bg-white/5 hover:text-slate-200 disabled:opacity-40"
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden>
              <path
                d="M5.5 8.5 9 5a2.1 2.1 0 0 1 3 3l-4.8 4.8a3.2 3.2 0 0 1-4.5-4.5L8 3"
                stroke="currentColor"
                strokeWidth="1.35"
                strokeLinecap="round"
              />
            </svg>
          </button>
          <DarkSelect
            ariaLabel="Mode"
            title={
              autonomy === "act" || autonomy === "automate"
                ? "Apply: shell + writes"
                : "Suggest: plan / inspect only"
            }
            value={
              autonomy === "act" || autonomy === "automate" ? "act" : "propose"
            }
            options={[
              { value: "propose", label: "Suggest" },
              { value: "act", label: "Apply" },
            ]}
            maxLabelChars={10}
            onChange={(next) => {
              if (next === "act") {
                requestApplyAutonomy(
                  autonomy === "automate" ? "automate" : "act",
                );
              } else {
                setAutonomyPersisted("propose");
              }
            }}
          />
          {(autonomy === "act" || autonomy === "automate") && (
            <button
              type="button"
              role="switch"
              aria-checked={autonomy === "automate"}
              title={
                autonomy === "automate"
                  ? "Auto on: verify when the turn finishes"
                  : "Auto off: Apply without auto-verify"
              }
              onClick={() => {
                if (autonomy === "automate") {
                  setAutonomyPersisted("act");
                  setVerifyOnComplete(false);
                } else {
                  // Already on Apply — flip Auto without re-prompting for a goal.
                  setAutonomyPersisted("automate");
                  setVerifyOnComplete(true);
                  if (effort === "low") {
                    setEffort("medium");
                    window.localStorage.setItem(AGENT_EFFORT_KEY, "medium");
                  }
                }
              }}
              className={`flex items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-semibold transition ${
                autonomy === "automate"
                  ? "border-emerald-400/35 bg-emerald-500/15 text-emerald-100"
                  : "border-white/10 bg-white/3 text-slate-400 hover:bg-white/5 hover:text-slate-200"
              }`}
            >
              <span
                className={`relative inline-flex h-3.5 w-6 shrink-0 rounded-full transition ${
                  autonomy === "automate" ? "bg-emerald-400/80" : "bg-slate-600"
                }`}
              >
                <span
                  className={`absolute top-0.5 size-2.5 rounded-full bg-white shadow transition ${
                    autonomy === "automate" ? "left-3" : "left-0.5"
                  }`}
                />
              </span>
              Auto
            </button>
          )}
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
            modelMode={modelMode}
            autoSummary={`Auto · ${autoModelForSlot(provider, slotFromAutonomy(autonomy)).profileId}`}
            onProviderChange={(preset) => {
              setProvider(preset.id);
              setBaseUrl(preset.baseUrl);
            }}
            onModelChange={(next) => {
              setModelMode("pin");
              setModel(next);
            }}
            onSelectAuto={() => setModelMode("auto")}
          />

          <div className="min-w-0 flex-1" />

          <ContextUsageButton
            contextPct={contextPct}
            contextUsed={contextUsed}
            contextLimit={contextLimit}
            segments={contextSegments}
            showSpend={Number(inputCost) > 0 || Number(outputCost) > 0}
            sessionSpendMicros={sessionTokens.costMicros}
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

          {busy && (prompt.trim() || attachments.length > 0) && (
            <button
              type="button"
              onClick={submit}
              disabled={!isTauri() || !provider.trim() || !model.trim()}
              className="grid size-8 shrink-0 place-items-center rounded-lg bg-blue-500 text-sm font-bold text-white hover:bg-blue-400 disabled:opacity-40"
              title="Add to queue"
            >
              ↑
            </button>
          )}
          <button
            type="button"
            onClick={() => {
              if (busy) {
                onCancel?.();
                return;
              }
              submit();
            }}
            disabled={
              !isTauri() ||
              (!busy &&
                ((!prompt.trim() && attachments.length === 0) ||
                  !provider.trim() ||
                  !model.trim()))
            }
            className={`grid size-8 shrink-0 place-items-center rounded-lg text-sm font-bold text-white disabled:opacity-40 ${
              busy
                ? "bg-rose-500 hover:bg-rose-400"
                : "bg-blue-500 hover:bg-blue-400"
            }`}
            title={
              !isTauri()
                ? "Desktop only"
                : busy
                  ? "Stop (Esc)"
                  : "Send"
            }
          >
            {busy ? (
              <span
                className="block size-2.5 rounded-[2px] bg-white"
                aria-hidden
              />
            ) : (
              "↑"
            )}
          </button>
        </div>
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
  onOpenAnalytics,
}: {
  audit: AuditReport;
  handoffs: HandoffHistoryItem[];
  onRefresh: () => void;
  onOpenSettings: () => void;
  onOpenAnalytics?: () => void;
}) {
  const [ignoreBusy, setIgnoreBusy] = useState(false);
  const [ignoreMessage, setIgnoreMessage] = useState<string | null>(null);
  const [spend, setSpend] = useState<{
    daily_usd: number;
    used_usd: number;
    reserved_usd: number;
    remaining_usd: number;
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
      window.localStorage.getItem("ade_session_cap_usd") || "0",
    );
    const dailyCap = Number(window.localStorage.getItem("ade_daily_cap_usd") || "0");
    void invoke<{
      daily_usd: number;
      used_usd: number;
      reserved_usd: number;
      remaining_usd: number;
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
    <div className="space-y-4">
      <Panel
        title="Trust"
        subtitle={`${audit.score}/${audit.score_max} readiness score`}
        actions={
          onOpenAnalytics ? (
            <button
              type="button"
              onClick={onOpenAnalytics}
              className="rounded-lg border border-white/10 bg-white/4 px-2.5 py-1 text-[11px] text-slate-300 hover:bg-white/8"
            >
              Cost trend in Analytics →
            </button>
          ) : undefined
        }
      >
        <p className="mb-4 text-[12px] leading-5 text-slate-500">
          Is this workspace safe to act in, and what did the agent actually do?
          Cost trend, model attribution, and reserve accuracy live in Analytics.
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

        {/* Cap headroom is a safety signal; the analytical view lives in Analytics. */}
        <div className="mb-5 rounded-xl border border-white/8 bg-white/2 p-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="min-w-0">
              <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-500">
                Budget headroom
              </div>
              <div className="mt-0.5 text-[12px] text-slate-400">
                {spend
                  ? `${usd(spend.remaining_usd)} left of the ${usd(spend.daily_cap_usd)} daily cap`
                  : "No spend data for this period yet"}
              </div>
              {spend && (
                <div className="mt-1 flex flex-wrap gap-3 font-mono text-[10px] text-slate-600">
                  <span>used {usd(spend.used_usd)}</span>
                  <span>reserved {usd(spend.reserved_usd)}</span>
                  <span>period {spend.period_key}</span>
                </div>
              )}
            </div>
            <div className="flex shrink-0 items-center gap-1.5">
              {onOpenAnalytics && (
                <button
                  type="button"
                  onClick={onOpenAnalytics}
                  className="rounded-md border border-blue-400/25 bg-blue-500/10 px-2.5 py-1.5 text-[11px] font-semibold text-blue-100 hover:bg-blue-500/20"
                >
                  Analytics
                </button>
              )}
              <button
                type="button"
                onClick={onOpenSettings}
                className="inline-flex items-center gap-1.5 rounded-md border border-white/10 px-2.5 py-1.5 text-[11px] font-semibold text-slate-300 hover:bg-white/5"
              >
                <GearIcon className="size-3" />
                Caps
              </button>
            </div>
          </div>
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
      title="Test project"
      subtitle="Runs build / lint / test gates for this folder — so you know it still works after changes."
    >
      {results.length === 0 ? (
        <div className="py-20 text-center">
          <div className="text-sm text-slate-400">No tests run yet</div>
          <p className="mx-auto mt-2 max-w-sm text-[11px] leading-5 text-slate-600">
            Recommended once after setup, and again after Apply changes. This is not the chat —
            it only checks your project sensors.
          </p>
          <button
            onClick={onRun}
            disabled={busy}
            className="mt-4 rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold disabled:opacity-50"
          >
            {busy ? "Testing…" : "Run project tests"}
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-white/7 bg-white/2 px-3 py-2 text-[11px] text-slate-400">
            <span>
              {passed}/{results.length} tests clear
            </span>
            <button
              type="button"
              onClick={onRun}
              disabled={busy}
              className="rounded-md border border-white/10 px-2 py-1 text-[10px] font-semibold text-slate-300 hover:bg-white/6 disabled:opacity-50"
            >
              {busy ? "Testing…" : "Re-run"}
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
            {busy ? "Testing…" : "Test again"}
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

function SpendUsageStrip({
  compact = false,
  className = "",
}: {
  compact?: boolean;
  className?: string;
}) {
  const [spend, setSpend] = useState<{
    daily_usd: number;
    used_usd: number;
    reserved_usd: number;
    remaining_usd: number;
    daily_cap_usd: number;
    session_cap_usd: number;
    period_key: string;
  } | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    const sessionCap = Number(
      window.localStorage.getItem("ade_session_cap_usd") || "0",
    );
    const dailyCap = Number(window.localStorage.getItem("ade_daily_cap_usd") || "0");
    let cancelled = false;
    void invoke<{
      daily_usd: number;
      used_usd: number;
      reserved_usd: number;
      remaining_usd: number;
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
        title={`period ${spend.period_key} · used ${usd(spend.used_usd)} · reserved ${usd(spend.reserved_usd)}`}
      >
        Used {usd(spend.used_usd)}
        {" · "}
        rem {usd(spend.remaining_usd)}
        {" · "}
        cap {usd(spend.daily_cap_usd)}
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
            Today&apos;s spend
          </div>
          <div className="mt-0.5 text-[12px] text-slate-400">
            Used, reserved, and what you have left today
          </div>
        </div>
        <div
          className={`font-mono text-[11px] tabular-nums ${
            overDaily ? "text-amber-200" : "text-slate-300"
          }`}
        >
          Used {usd(spend.used_usd)}
        </div>
      </div>
      <div className="mt-2 flex flex-wrap gap-3 font-mono text-[10px] text-slate-500">
        <span>reserved {usd(spend.reserved_usd)}</span>
        <span>remaining {usd(spend.remaining_usd)}</span>
        <span>daily cap {usd(spend.daily_cap_usd)}</span>
        <span>session cap {usd(spend.session_cap_usd)}</span>
        <span>period {spend.period_key}</span>
      </div>
    </div>
  );
}

function ContextUsageButton({
  contextPct,
  contextUsed,
  contextLimit,
  segments,
  showSpend,
  sessionSpendMicros,
  effort,
  showEffort,
  onEffort,
  onSaveGoal,
  goalBusy,
}: {
  contextPct: number;
  contextUsed: number;
  contextLimit: number;
  segments: { label: string; tokens: number; color: string }[];
  showSpend: boolean;
  sessionSpendMicros: number;
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

  const segmentTotal = Math.max(
    1,
    segments.reduce((sum, row) => sum + row.tokens, 0),
  );
  const usedForBar = Math.min(contextLimit, Math.max(contextUsed, segmentTotal));

  useLayoutEffect(() => {
    if (!open || !rootRef.current) return;
    const place = () => {
      const rect = rootRef.current!.getBoundingClientRect();
      const width = 300;
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
        title="Context usage"
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
          className="rounded-xl border border-white/12 bg-[#0f141b] p-3 shadow-xl shadow-black/40"
          onClick={(event) => event.stopPropagation()}
        >
          <div className="mb-2 flex items-start justify-between gap-2">
            <div className="text-[12px] font-semibold text-slate-100">Context Usage</div>
            <button
              type="button"
              aria-label="Close"
              onClick={() => setOpen(false)}
              className="rounded px-1 text-[12px] text-slate-500 hover:bg-white/5 hover:text-slate-300"
            >
              ×
            </button>
          </div>
          <div className="mb-2 flex items-baseline justify-between gap-2">
            <div
              className={`text-[12px] font-semibold ${
                contextPct >= 85 ? "text-amber-200" : "text-slate-200"
              }`}
            >
              {contextPct}% Full
            </div>
            <div className="text-[11px] tabular-nums text-slate-500">
              ~{formatTokenCount(usedForBar)} / {formatTokenCount(contextLimit)} Tokens
            </div>
          </div>
          <div className="mb-3 flex h-1.5 overflow-hidden rounded-full bg-slate-800">
            {segments
              .filter((row) => row.tokens > 0)
              .map((row) => (
                <div
                  key={row.label}
                  title={`${row.label}: ${formatTokenCount(row.tokens)}`}
                  className="h-full"
                  style={{
                    width: `${Math.max(1, (row.tokens / contextLimit) * 100)}%`,
                    backgroundColor: row.color,
                  }}
                />
              ))}
          </div>
          <div className="space-y-1.5">
            {segments.map((row) => (
              <div
                key={row.label}
                className="flex items-center justify-between gap-2 text-[11px]"
              >
                <div className="flex min-w-0 items-center gap-2 text-slate-400">
                  <span
                    className="size-2 shrink-0 rounded-[2px]"
                    style={{ backgroundColor: row.color }}
                  />
                  <span className="truncate">{row.label}</span>
                </div>
                <span className="shrink-0 tabular-nums text-slate-300">
                  {formatTokenCount(row.tokens)}
                </span>
              </div>
            ))}
          </div>
          {showEffort && (
            <div className="mt-3 border-t border-white/8 pt-3">
              <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-slate-600">
                Effort · turn budget
              </div>
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
          {showSpend && (
            <div className="mt-2 text-[10px] text-slate-500">
              Session spend ≈ {usd(sessionSpendMicros / 1_000_000)}
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
  return String(Math.max(0, Math.round(tokens)));
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
