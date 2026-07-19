import { Channel } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke, isTauri } from "./ipc";
import {
  RecipeWizard,
  type ScaffoldResult,
  type ScaffoldFilePlan,
  type StackRecipe,
} from "./components/RecipeWizard";

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
};

type PlanReport = {
  phases: PlanPhase[];
  requires_human: string[];
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

type DashboardSnapshot = {
  workspace_root: string;
  audit: AuditReport;
  plan: PlanReport;
  handoff: HandoffMetrics;
  leases: PathLease[];
  tasks: AgentTask[];
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
  | { type: "tool_call"; server: string; tool: string; arguments: unknown }
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

const navItems = [
  ["Overview", "⌂"],
  ["Agent", "✦"],
  ["Keys", "◈"],
  ["Audit", "◎"],
  ["Plan", "◇"],
  ["Verify", "✓"],
  ["MCP", "⬡"],
  ["Recipes", "▦"],
] as const;

function App() {
  const [dashboard, setDashboard] = useState<DashboardSnapshot | null>(null);
  const [activeView, setActiveView] = useState("Overview");
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

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const snapshot = await invoke<DashboardSnapshot>("get_dashboard");
      setDashboard({ ...snapshot, tasks: snapshot.tasks ?? [] });
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

  const runVerify = async () => {
    setVerifying(true);
    setError(null);
    try {
      const results = await invoke<VerifyResult[]>("run_verify", {
        gate,
        through: true,
      });
      setVerifyResults(results);
      setActiveView("Verify");
    } catch (reason) {
      setError(String(reason));
    } finally {
      setVerifying(false);
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
  }) => {
    if (!isTauri) {
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
        onEvent,
      });
    } catch (reason) {
      setError(String(reason));
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
      <aside className="flex w-58 shrink-0 flex-col border-r border-white/7 bg-[#0b0f16]/95 px-3 py-4">
        <div className="flex items-center gap-3 px-3 pb-7">
          <div className="grid size-9 place-items-center rounded-xl border border-blue-400/30 bg-blue-500/12 text-sm font-black text-blue-300">
            A
          </div>
          <div>
            <div className="text-sm font-semibold tracking-wide">ADE</div>
            <div className="text-[10px] uppercase tracking-[0.18em] text-slate-500">
              Development Environment
            </div>
          </div>
        </div>

        <nav className="space-y-1">
          {navItems.map(([label, icon]) => (
            <button
              key={label}
              onClick={() => setActiveView(label)}
              className={`flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm transition ${
                activeView === label
                  ? "bg-blue-500/12 text-blue-200"
                  : "text-slate-400 hover:bg-white/4 hover:text-slate-200"
              }`}
            >
              <span className="w-4 text-center text-base">{icon}</span>
              {label}
            </button>
          ))}
        </nav>

        <div className="mt-7 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-slate-600">
          System
        </div>
        <div className="mt-3 space-y-1 text-xs text-slate-500">
          <div className="flex items-center gap-2 px-3 py-2">
            <span className="size-1.5 rounded-full bg-emerald-400" />
            Local runtime
          </div>
          <div className="flex items-center gap-2 px-3 py-2">
            <span className="size-1.5 rounded-full bg-blue-400" />
            Key vault ready
          </div>
          <div className="flex items-center gap-2 px-3 py-2">
            <span
              className={`size-1.5 rounded-full ${
                mcpServers.length > 0 ? "bg-emerald-400" : "bg-violet-400"
              }`}
            />
            {mcpServers.length > 0
              ? `${mcpServers.length} MCP server${mcpServers.length === 1 ? "" : "s"}`
              : "MCP host idle"}
          </div>
        </div>

        <div className="mt-auto rounded-xl border border-white/7 bg-white/2.5 p-3">
          <div className="text-[10px] uppercase tracking-wider text-slate-600">
            Phase router
          </div>
          <div className="mt-2 flex items-center gap-1.5 text-[11px]">
            <span className="rounded bg-emerald-400/10 px-1.5 py-1 text-emerald-300">
              AUDIT
            </span>
            <span className="text-slate-700">→</span>
            <span className="rounded bg-blue-400/10 px-1.5 py-1 text-blue-300">
              PLAN
            </span>
            <span className="text-slate-700">→</span>
            <span className="rounded bg-violet-400/10 px-1.5 py-1 text-violet-300">
              EXECUTE
            </span>
          </div>
        </div>
      </aside>

      <main className="thin-scrollbar min-w-0 flex-1 overflow-y-auto">
        <header className="sticky top-0 z-10 flex h-16 items-center justify-between border-b border-white/7 bg-[#080b11]/90 px-7 backdrop-blur-xl">
          <div>
            <h1 className="text-sm font-semibold">{activeView}</h1>
            <p className="mt-0.5 max-w-[62vw] truncate text-[11px] text-slate-500">
              {dashboard?.workspace_root ?? "Locating workspace…"}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <select
              value={gate}
              onChange={(event) => setGate(event.target.value)}
              className="rounded-lg border border-white/10 bg-[#101620] px-2.5 py-2 text-xs text-slate-300"
            >
              {["G0", "G1", "G2", "G3", "G4", "G5"].map((item) => (
                <option key={item}>{item}</option>
              ))}
            </select>
            <button
              onClick={() => void runVerify()}
              disabled={verifying}
              className="rounded-lg bg-blue-500 px-3.5 py-2 text-xs font-semibold text-white transition hover:bg-blue-400 disabled:opacity-50"
            >
              {verifying ? "Verifying…" : `Run through ${gate}`}
            </button>
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

        <div className="mx-auto max-w-[1400px] p-7">
          {error && (
            <div className="mb-5 rounded-xl border border-red-400/20 bg-red-400/7 px-4 py-3 text-xs text-red-200">
              {error}
            </div>
          )}

          {loading && !dashboard ? (
            <LoadingState />
          ) : dashboard ? (
            <>
              {activeView === "Overview" && (
                <Overview
                  dashboard={dashboard}
                  scorePercent={scorePercent}
                  verifyResults={verifyResults}
                  executing={executing}
                  onExecute={() => void executePlan()}
                />
              )}
              {activeView === "Agent" && (
                <AgentView
                  events={agentEvents}
                  busy={agentBusy}
                  connectedTools={mcpTools.length}
                  onRun={(input) => void runAgentTurn(input)}
                />
              )}
              {activeView === "Keys" && <KeysView />}
              {activeView === "Audit" && <AuditView audit={dashboard.audit} />}
              {activeView === "Plan" && (
                <PlanView
                  plan={dashboard.plan}
                  executing={executing}
                  onExecute={() => void executePlan()}
                />
              )}
              {activeView === "Verify" && (
                <VerifyView results={verifyResults} onRun={() => void runVerify()} />
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

function Overview({
  dashboard,
  scorePercent,
  verifyResults,
  executing,
  onExecute,
}: {
  dashboard: DashboardSnapshot;
  scorePercent: number;
  verifyResults: VerifyResult[];
  executing: boolean;
  onExecute: () => void;
}) {
  const passed = verifyResults.filter((result) => result.passed).length;
  return (
    <div className="space-y-5">
      <section className="grid grid-cols-7 gap-4">
        <MetricCard label="Readiness score" value={`${scorePercent}%`} accent="blue" />
        <MetricCard
          label="Audit blockers"
          value={String(dashboard.audit.blockers.length)}
          accent={dashboard.audit.blockers.length ? "red" : "green"}
        />
        <MetricCard
          label="Planned phases"
          value={String(dashboard.plan.phases.length)}
          accent="violet"
        />
        <MetricCard
          label="Verify gates"
          value={verifyResults.length ? `${passed}/${verifyResults.length}` : "Not run"}
          accent={verifyResults.length && passed === verifyResults.length ? "green" : "slate"}
        />
        <MetricCard
          label="Handoff capsules"
          value={String(dashboard.handoff.capsule_count)}
          accent={dashboard.handoff.invalid_capsule_count ? "red" : "green"}
        />
        <MetricCard
          label="Active leases"
          value={String(dashboard.leases.length)}
          accent={dashboard.leases.length ? "violet" : "slate"}
        />
        <MetricCard
          label="Open tasks"
          value={String(
            dashboard.tasks.filter(
              (task) => !["completed", "failed", "cancelled"].includes(task.status),
            ).length,
          )}
          accent={dashboard.tasks.some((task) => task.status === "running") ? "blue" : "slate"}
        />
      </section>

      <section className="grid grid-cols-[1.4fr_1fr] gap-5">
        <Panel title="Environment health" subtitle="Live L0–L11 audit assessment">
          <div className="flex gap-7">
            <div
              className="score-ring grid size-32 shrink-0 place-items-center rounded-full p-2"
              style={
                {
                  "--score-angle": `${scorePercent * 3.6}deg`,
                } as React.CSSProperties
              }
            >
              <div className="grid size-full place-items-center rounded-full bg-[#0d121a] text-center">
                <div>
                  <div className="text-2xl font-semibold">{scorePercent}%</div>
                  <div className="text-[10px] text-slate-500">
                    {dashboard.audit.score}/{dashboard.audit.score_max}
                  </div>
                </div>
              </div>
            </div>
            <div className="min-w-0 flex-1 space-y-3">
              {dashboard.audit.findings.slice(0, 6).map((finding) => (
                <FindingBar key={finding.layer} finding={finding} />
              ))}
            </div>
          </div>
        </Panel>

        <Panel title="Current plan" subtitle="Human-gated execution scope">
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

      <Panel
        title="Multi-agent ownership"
        subtitle="Active durable leases; observe leases never grant write scope"
      >
        {dashboard.leases.length === 0 ? (
          <p className="text-xs text-slate-500">
            No active leases. Agent turns remain read-only unless explicit PLAN owned paths are
            supplied.
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

      <Panel
        title="Coordinated task queue"
        subtitle="Dependency-aware claims acquire durable path leases before agent work starts"
      >
        {dashboard.tasks.length === 0 ? (
          <p className="text-xs text-slate-500">
            No queued tasks. Use <span className="font-mono">ade task enqueue --approve</span> to
            add lease-backed work.
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

      <Panel title="Continuity health" subtitle="Aggregate metadata only; capsule text stays local">
        <div className="grid grid-cols-4 gap-6">
          <div>
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Latest score snapshot
            </div>
            <div className="mt-2 text-lg font-semibold text-slate-200">
              {dashboard.handoff.latest_score_before !== null
                ? dashboard.handoff.latest_score_after !== null
                  ? `${dashboard.handoff.latest_score_before} → ${dashboard.handoff.latest_score_after}`
                  : String(dashboard.handoff.latest_score_before)
                : "Not measured"}
              {dashboard.handoff.latest_score_max !== null
                ? ` / ${dashboard.handoff.latest_score_max}`
                : ""}
            </div>
            {dashboard.handoff.latest_score_delta !== null && (
              <div
                className={`mt-1 text-[10px] ${
                  dashboard.handoff.latest_score_delta >= 0
                    ? "text-emerald-400"
                    : "text-red-400"
                }`}
              >
                {dashboard.handoff.latest_score_delta >= 0 ? "+" : ""}
                {dashboard.handoff.latest_score_delta} points
              </div>
            )}
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Prompt compaction
            </div>
            <div className="mt-2 text-lg font-semibold text-slate-200">
              {dashboard.handoff.latest_context_status
                ? dashboard.handoff.latest_context_status
                : dashboard.handoff.latest_bytes
                  ? `${dashboard.handoff.latest_compaction_percent}%`
                  : "Not measured"}
            </div>
            <div className="mt-1 text-[10px] text-slate-600">
              {dashboard.handoff.latest_context_tokens !== null
                ? `${dashboard.handoff.latest_context_tokens} assembled tokens · `
                : ""}
              {dashboard.handoff.latest_summary_chars} summary chars /{" "}
              {dashboard.handoff.latest_bytes} capsule bytes
            </div>
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Latest state
            </div>
            <div className="mt-2 text-lg font-semibold capitalize text-slate-200">
              {dashboard.handoff.latest_status?.replaceAll("_", " ") ?? "No handoff"}
            </div>
            <div className="mt-1 truncate text-[10px] text-slate-600">
              {dashboard.handoff.latest_created_at ?? "No timestamp"}
            </div>
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Archive health
            </div>
            <div className="mt-2 text-lg font-semibold text-slate-200">
              {dashboard.handoff.invalid_capsule_count === 0 ? "Valid" : "Attention"}
            </div>
            <div className="mt-1 text-[10px] text-slate-600">
              {dashboard.handoff.invalid_capsule_count} invalid ·{" "}
              {dashboard.handoff.total_bytes.toLocaleString()} bytes stored
            </div>
          </div>
        </div>

        {dashboard.handoff.latest_context_sections.length > 0 && (
          <div className="mt-5 space-y-2">
            <div className="text-[10px] uppercase tracking-wider text-slate-600">
              Assembled prompt sections
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
}: {
  events: AgentEvent[];
  busy: boolean;
  connectedTools: number;
  onRun: (input: {
    prompt: string;
    provider: string;
    baseUrl: string;
    model: string;
    inputCostPerMtok: number;
    outputCostPerMtok: number;
    sessionCapUsd: number;
    dailyCapUsd: number;
  }) => void;
}) {
  const [prompt, setPrompt] = useState("");
  const [provider, setProvider] = useState("openai");
  const [baseUrl, setBaseUrl] = useState("https://api.openai.com/v1");
  const [model, setModel] = useState("");
  const [inputCost, setInputCost] = useState("0");
  const [outputCost, setOutputCost] = useState("0");
  const [sessionCap, setSessionCap] = useState("1");
  const [dailyCap, setDailyCap] = useState("10");
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
  const activity = events.filter(
    (event) => event.type === "tool_call" || event.type === "tool_result",
  );

  const submit = () => {
    onRun({
      prompt: prompt.trim(),
      provider: provider.trim(),
      baseUrl: baseUrl.trim(),
      model: model.trim(),
      inputCostPerMtok: Number(inputCost) || 0,
      outputCostPerMtok: Number(outputCost) || 0,
      sessionCapUsd: Number(sessionCap),
      dailyCapUsd: Number(dailyCap),
    });
  };

  return (
    <div className="grid grid-cols-[1fr_310px] gap-5">
      <Panel
        title="Agent session"
        subtitle="BYOK streaming · exact model lock · runtime authority · read-only until PLAN approval"
      >
        <div className="min-h-72 rounded-xl border border-white/7 bg-black/20 p-4">
          {!text && !busy ? (
            <div className="grid min-h-64 place-items-center text-center">
              <div>
                <div className="text-sm text-slate-300">Start a trustworthy agent turn</div>
                <p className="mt-2 max-w-md text-xs leading-5 text-slate-600">
                  The selected model can use connected read tools. Write-capable tools remain
                  blocked until an approved PLAN grants owned paths.
                </p>
              </div>
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
                    → {event.server}/{event.tool}
                  </span>
                ) : (
                  <span className={event.is_error ? "text-red-300" : "text-emerald-300"}>
                    ← {event.is_error ? "error" : "ok"} {event.server}/{event.tool}
                  </span>
                )}
              </div>
            ))}
          </div>
        )}

        {completed && (
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

        <textarea
          value={prompt}
          onChange={(event) => setPrompt(event.target.value)}
          rows={4}
          className="thin-scrollbar mt-4 w-full rounded-xl border border-white/10 bg-[#101620] px-3 py-3 text-sm leading-6 text-slate-200"
          placeholder="What should ADE help you accomplish?"
        />
        <div className="mt-3 flex items-center justify-between">
          <span className="text-[10px] text-slate-600">
            {connectedTools} MCP tool{connectedTools === 1 ? "" : "s"} available
          </span>
          <button
            onClick={submit}
            disabled={busy || !prompt.trim() || !provider.trim() || !model.trim()}
            className="rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
          >
            {busy ? "Running…" : "Run agent turn"}
          </button>
        </div>
      </Panel>

      <Panel title="Provider" subtitle="Key loaded from the local OS vault">
        <div className="space-y-3">
          <Field label="Provider id" value={provider} onChange={setProvider} />
          <Field label="Base URL" value={baseUrl} onChange={setBaseUrl} mono />
          <Field label="Exact model id" value={model} onChange={setModel} mono />
          <div className="grid grid-cols-2 gap-2">
            <Field label="Input $/MTok" value={inputCost} onChange={setInputCost} />
            <Field label="Output $/MTok" value={outputCost} onChange={setOutputCost} />
          </div>
          <div className="grid grid-cols-2 gap-2">
            <Field label="Session cap $" value={sessionCap} onChange={setSessionCap} />
            <Field label="Daily cap $" value={dailyCap} onChange={setDailyCap} />
          </div>
          <div className="rounded-lg border border-blue-400/15 bg-blue-400/5 p-3 text-[10px] leading-5 text-blue-200/70">
            Manage this provider in the Keys view or run{" "}
            <span className="font-mono">ade keys set {provider}</span>. Keys never enter prompts,
            events, or handoff capsules. Spend caps are reserved before every provider round.
          </div>
        </div>
      </Panel>
    </div>
  );
}

function KeysView() {
  const [provider, setProvider] = useState("openai");
  const [profile, setProfile] = useState("local");
  const [secret, setSecret] = useState("");
  const [baseUrl, setBaseUrl] = useState("https://api.openai.com/v1");
  const [model, setModel] = useState("");
  const [inputCostPerMtok, setInputCostPerMtok] = useState("");
  const [outputCostPerMtok, setOutputCostPerMtok] = useState("");
  const [maxCostUsd, setMaxCostUsd] = useState("0.05");
  const [approveLiveCost, setApproveLiveCost] = useState(false);
  const [status, setStatus] = useState<ProviderKeyStatus | null>(null);
  const [smoke, setSmoke] = useState<ProviderKeySmokeResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

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

  const save = async () => {
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
      setMessage(`${result.provider} credential saved to the OS vault.`);
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
    <div className="grid grid-cols-[1fr_340px] gap-5">
      <Panel
        title="Provider credentials"
        subtitle="Stored only in the native OS credential vault"
      >
        <div className="grid max-w-2xl grid-cols-2 gap-3">
          <label className="block">
            <span className="mb-1.5 block text-[10px] font-semibold uppercase tracking-wider text-slate-600">
              Provider
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
          <div className="text-xs font-semibold text-slate-300">Live credential validation</div>
          <p className="mt-1 text-[10px] leading-5 text-slate-500">
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
        </div>

        <div className="mt-4 flex flex-wrap gap-2">
          <button
            onClick={() => void save()}
            disabled={busy || !provider.trim() || !profile.trim() || !secret.trim()}
            className="rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold hover:bg-blue-400 disabled:opacity-50"
          >
            {status?.configured ? "Replace key" : "Save key"}
          </button>
          <button
            onClick={() => void refreshStatus()}
            disabled={busy || !provider.trim() || !profile.trim()}
            className="rounded-lg border border-white/10 px-4 py-2 text-xs text-slate-300 hover:bg-white/5 disabled:opacity-50"
          >
            Check status
          </button>
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
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  mono?: boolean;
}) {
  return (
    <label className="block text-[11px] text-slate-500">
      {label}
      <input
        value={value}
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

function PlanView({
  plan,
  executing,
  onExecute,
}: {
  plan: PlanReport;
  executing: boolean;
  onExecute: () => void;
}) {
  return (
    <Panel title="PLAN report" subtitle={`${plan.phases.length} approved-scope phase(s)`}>
      {plan.phases.length === 0 ? (
        <div className="py-20 text-center text-sm text-slate-500">
          No remediation phases were generated by the current audit.
        </div>
      ) : (
        <div className="space-y-3">
          {plan.phases.map((phase, index) => (
            <div key={phase.id} className="rounded-xl border border-white/7 bg-white/2 p-4">
              <div className="flex items-start gap-4">
                <div className="grid size-7 shrink-0 place-items-center rounded-full bg-blue-400/10 text-xs text-blue-300">
                  {index + 1}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{phase.title}</div>
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {phase.owned_paths.map((path) => (
                      <span key={path} className="rounded bg-white/5 px-2 py-1 font-mono text-[10px] text-slate-400">
                        {path}
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          ))}
          <button
            onClick={onExecute}
            disabled={executing}
            className="rounded-lg bg-violet-500 px-4 py-2.5 text-xs font-semibold hover:bg-violet-400 disabled:opacity-50"
          >
            {executing ? "Executing approved paths…" : "Approve and execute plan"}
          </button>
        </div>
      )}
    </Panel>
  );
}

function VerifyView({ results, onRun }: { results: VerifyResult[]; onRun: () => void }) {
  return (
    <Panel title="Verification evidence" subtitle="Commands, status, and captured output">
      {results.length === 0 ? (
        <div className="py-20 text-center">
          <div className="text-sm text-slate-400">No verification evidence yet</div>
          <button onClick={onRun} className="mt-4 rounded-lg bg-blue-500 px-4 py-2 text-xs font-semibold">
            Run verification
          </button>
        </div>
      ) : (
        <div className="space-y-3">
          {results.map((result) => (
            <div key={result.gate} className="rounded-xl border border-white/7 bg-white/2 p-4">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-sm font-semibold">{result.gate}</span>
                  <span className="ml-3 font-mono text-[11px] text-slate-500">{result.command}</span>
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
                    ? "● PASS"
                    : result.status === "unavailable"
                      ? "● UNAVAILABLE"
                      : "● FAIL"}
                </span>
              </div>
              {(result.stderr || result.stdout) && (
                <pre className="thin-scrollbar mt-3 max-h-44 overflow-auto whitespace-pre-wrap rounded-lg bg-black/25 p-3 text-[10px] leading-5 text-slate-500">
                  {result.stderr || result.stdout}
                </pre>
              )}
            </div>
          ))}
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
              No MCP servers connected yet.
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
  children,
}: {
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <section className="rounded-2xl border border-white/7 bg-[#0d121a]/85 p-5 shadow-[0_12px_45px_rgba(0,0,0,0.15)]">
      <div className="mb-5">
        <h2 className="text-sm font-semibold">{title}</h2>
        <p className="mt-1 text-[11px] text-slate-600">{subtitle}</p>
      </div>
      {children}
    </section>
  );
}

function MetricCard({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent: "blue" | "green" | "red" | "violet" | "slate";
}) {
  const colors = {
    blue: "text-blue-300",
    green: "text-emerald-300",
    red: "text-red-300",
    violet: "text-violet-300",
    slate: "text-slate-300",
  };
  return (
    <div className="rounded-xl border border-white/7 bg-[#0d121a]/80 px-4 py-4">
      <div className="text-[10px] uppercase tracking-[0.14em] text-slate-600">{label}</div>
      <div className={`mt-2 text-xl font-semibold ${colors[accent]}`}>{value}</div>
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
