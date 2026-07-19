use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ade", about = "Agentic Development Environment CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run AUDIT phase — discover and score the environment
    Audit {
        #[arg(short, long)]
        mode: Option<String>,
    },
    /// Run PLAN phase — create a phased plan with gates
    Plan,
    /// Run EXECUTE phase — apply approved plan phases (requires --approve)
    Execute {
        /// Explicit human approval to mutate owned_paths from the plan
        #[arg(long)]
        approve: bool,
        /// Recipe used when writing a missing AGENTS.md
        #[arg(long, default_value = "rust-api-turso")]
        recipe: String,
        /// Limit EXECUTE to these plan phase ids (repeatable)
        #[arg(long = "phase")]
        phases: Vec<String>,
    },
    /// Initialize a project with a stack recipe (writes AGENTS.md)
    Init {
        #[arg(short, long)]
        recipe: Option<String>,
        /// Overwrite an existing AGENTS.md
        #[arg(long)]
        force: bool,
        /// Project display name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,
    },
    /// List built-in stack recipes
    Recipes,
    /// Run G0-G5 verification
    Verify {
        #[arg(short, long)]
        gate: Option<String>,
        /// Run every gate from G0 through the selected gate, stopping on failure
        #[arg(long)]
        through: bool,
    },
    /// Manage BYOK provider keys in the OS credential vault
    Keys {
        #[command(subcommand)]
        action: KeysAction,
    },
    /// Inspect MCP (Model Context Protocol) servers
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
    /// Validate foundation wiring; optionally run one explicitly capped live LLM turn
    Smoke {
        /// Credential-vault profile to probe for BYOK keys
        #[arg(long)]
        profile: Option<String>,
        /// Make one provider request after the offline foundation checks pass
        #[arg(long)]
        live: bool,
        /// Provider id used to locate the key in the OS credential vault
        #[arg(long, default_value = "openai")]
        provider: String,
        /// OpenAI-compatible API base URL
        #[arg(long, default_value = "https://api.openai.com/v1")]
        base_url: String,
        /// Exact model id (required with --live)
        #[arg(long, requires = "live")]
        model: Option<String>,
        /// Provider price per million input tokens in USD (required for cost enforcement)
        #[arg(long, default_value_t = 0.0)]
        input_cost_per_mtok: f64,
        /// Provider price per million output tokens in USD (required for cost enforcement)
        #[arg(long, default_value_t = 0.0)]
        output_cost_per_mtok: f64,
        /// Maximum input/context tokens permitted for the smoke request
        #[arg(long, default_value_t = 8_192)]
        context_limit: u64,
        /// Maximum output tokens sent to the provider
        #[arg(long, default_value_t = 16)]
        output_limit: u64,
        /// Hard maximum estimated cost for the live smoke request in USD
        #[arg(long, default_value_t = 0.05)]
        max_cost_usd: f64,
    },
    /// Manage workspaces
    Workspace {
        #[command(subcommand)]
        action: Option<WorkspaceAction>,
    },
    /// Show usage and analytics
    Analytics,
    /// Manage git worktrees for parallel agents
    Worktree {
        #[command(subcommand)]
        action: WorktreeAction,
    },
    /// Manage durable path leases for multi-agent ownership
    Lease {
        #[command(subcommand)]
        action: LeaseAction,
    },
    /// Coordinate durable lease-backed tasks across agents
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Run an automatic AgentTurn worker against the task queue
    Worker {
        #[command(subcommand)]
        action: WorkerAction,
    },
    /// Discover and invoke sandboxed capability-free WASM plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Serve the local ADE HTTP API (reads always; coordination writes when token scopes allow)
    Serve {
        /// Loopback socket address (non-loopback binds are refused)
        #[arg(long, default_value = "127.0.0.1:3210")]
        bind: String,
    },
    /// Manage the ADE daemon (workspace-local process or OS service install)
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Run one streamed BYOK agent turn
    Agent {
        /// User prompt for this turn
        prompt: String,
        /// Provider id used to locate the key in the OS credential vault
        #[arg(long, default_value = "openai")]
        provider: String,
        /// OpenAI-compatible API base URL
        #[arg(long, default_value = "https://api.openai.com/v1")]
        base_url: String,
        /// Exact model id; ADE never silently substitutes another model
        #[arg(long)]
        model: String,
        /// Provider price per million input tokens (USD)
        #[arg(long, default_value_t = 0.0)]
        input_cost_per_mtok: f64,
        /// Provider price per million output tokens (USD)
        #[arg(long, default_value_t = 0.0)]
        output_cost_per_mtok: f64,
        /// Model context window; required non-zero when prices are set
        #[arg(long, default_value_t = 128_000)]
        context_limit: u64,
        /// Model max output tokens; required non-zero when prices are set
        #[arg(long, default_value_t = 16_384)]
        output_limit: u64,
        /// Credential-vault profile (defaults to the active ADE environment)
        #[arg(long)]
        profile: Option<String>,
        /// Agent UUID whose active writable leases define tool write scope
        #[arg(long)]
        lease_agent: Option<String>,
    },
}

#[derive(Subcommand)]
enum WorktreeAction {
    /// List git worktrees for the current repository
    List {
        /// Emit JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Create a linked worktree on a new branch
    Add {
        /// Filesystem path for the new worktree
        #[arg(long)]
        path: String,
        /// Branch name to create
        #[arg(long)]
        branch: String,
        /// Optional start point (commit/branch)
        #[arg(long)]
        start_point: Option<String>,
        /// Confirm you reviewed this mutating git operation
        #[arg(long)]
        approve: bool,
    },
    /// Remove a linked worktree
    Remove {
        /// Filesystem path of the worktree to remove
        #[arg(long)]
        path: String,
        /// Confirm you reviewed this mutating git operation
        #[arg(long)]
        approve: bool,
        /// Force removal even if the worktree has local changes
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum LeaseAction {
    /// List active path leases
    List {
        /// Emit JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Acquire a path lease for an agent
    Acquire {
        /// Agent id (UUID)
        #[arg(long)]
        agent: String,
        /// Relative workspace path to lease
        #[arg(long)]
        path: String,
        /// observe | cooperative | strong | exclusive
        #[arg(long, default_value = "strong")]
        mode: String,
        /// Lease lifetime in seconds
        #[arg(long, default_value_t = 28_800)]
        ttl_secs: i64,
        /// Confirm this mutating ownership change
        #[arg(long)]
        approve: bool,
    },
    /// Renew (heartbeat) an active lease held by an agent
    Renew {
        id: String,
        /// Agent id (UUID) that holds the lease
        #[arg(long)]
        agent: String,
        /// New lease lifetime in seconds, measured from now
        #[arg(long, default_value_t = 28_800)]
        ttl_secs: i64,
        /// Confirm this mutating ownership change
        #[arg(long)]
        approve: bool,
    },
    /// Release a lease by id
    Release {
        id: String,
        /// Confirm this mutating ownership change
        #[arg(long)]
        approve: bool,
    },
    /// Drop expired leases from the durable registry
    ReleaseStale {
        /// Confirm this mutating ownership change
        #[arg(long)]
        approve: bool,
    },
}

#[derive(Subcommand)]
enum TaskAction {
    /// List all coordinated tasks
    List {
        /// Emit JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Add a task to the durable queue
    Enqueue {
        /// Goal delivered to the assigned agent
        #[arg(long)]
        goal: String,
        /// Relative writable path; repeat for multiple paths
        #[arg(long = "path")]
        owned_paths: Vec<String>,
        /// observe | cooperative | strong | exclusive
        #[arg(long, default_value = "strong")]
        mode: String,
        /// Task id that must complete first; repeat for multiple dependencies
        #[arg(long = "depends-on")]
        depends_on: Vec<String>,
        /// Confirm this mutating queue operation
        #[arg(long)]
        approve: bool,
    },
    /// Atomically claim the oldest dependency-ready task and acquire its leases
    Claim {
        /// Agent id (UUID) claiming the task
        #[arg(long)]
        agent: String,
        /// Claim and lease lifetime in seconds
        #[arg(long, default_value_t = 28_800)]
        ttl_secs: i64,
        /// Confirm this mutating ownership operation
        #[arg(long)]
        approve: bool,
    },
    /// Mark a claimed task as running
    Start {
        id: String,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        approve: bool,
    },
    /// Renew an active task claim and all of its leases
    Heartbeat {
        id: String,
        #[arg(long)]
        agent: String,
        #[arg(long, default_value_t = 28_800)]
        ttl_secs: i64,
        #[arg(long)]
        approve: bool,
    },
    /// Complete a task and release its leases
    Complete {
        id: String,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        approve: bool,
    },
    /// Fail a task and release its leases
    Fail {
        id: String,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        approve: bool,
    },
    /// Cancel a queued or active task
    Cancel {
        id: String,
        #[arg(long)]
        approve: bool,
    },
    /// Requeue expired claims and release any remaining leases
    RequeueExpired {
        #[arg(long)]
        approve: bool,
    },
}

#[derive(Subcommand)]
enum WorkerAction {
    /// Claim and execute lease-backed tasks until interrupted
    Run {
        /// Stable agent UUID used for claims and lease binding
        #[arg(long)]
        agent: String,
        /// Provider id used to locate the key in the OS credential vault
        #[arg(long, default_value = "openai")]
        provider: String,
        /// OpenAI-compatible API base URL
        #[arg(long, default_value = "https://api.openai.com/v1")]
        base_url: String,
        /// Exact model id; ADE never silently substitutes another model
        #[arg(long)]
        model: String,
        /// Provider price per million input tokens (USD)
        #[arg(long, default_value_t = 0.0)]
        input_cost_per_mtok: f64,
        /// Provider price per million output tokens (USD)
        #[arg(long, default_value_t = 0.0)]
        output_cost_per_mtok: f64,
        /// Model context window; required non-zero when prices are set
        #[arg(long, default_value_t = 128_000)]
        context_limit: u64,
        /// Model max output tokens; required non-zero when prices are set
        #[arg(long, default_value_t = 16_384)]
        output_limit: u64,
        /// Credential-vault profile (defaults to the active ADE environment)
        #[arg(long)]
        profile: Option<String>,
        /// Claim and lease lifetime in seconds
        #[arg(long, default_value_t = 28_800)]
        ttl_secs: i64,
        /// Idle poll interval in milliseconds
        #[arg(long, default_value_t = 2_000)]
        poll_ms: u64,
        /// Provision an isolated git worktree per claimed task
        #[arg(long)]
        worktree: bool,
        /// Remove successful worktrees after completion
        #[arg(long)]
        cleanup_worktree: bool,
        /// Confirm this process may automatically claim and mutate ownership
        #[arg(long)]
        approve: bool,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List workspace plugins and validate their manifests
    List {
        /// Emit JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Pin a plugin digest/pubkey into the local trust store
    Trust {
        /// Plugin manifest id
        id: String,
        /// Optional Ed25519 verifying key (hex)
        #[arg(long)]
        pubkey: Option<String>,
        /// Pin the current artifact digest instead of trusting by id alone
        #[arg(long, default_value_t = true)]
        pin_digest: bool,
        /// Confirm this mutating trust-store change
        #[arg(long)]
        approve: bool,
    },
    /// Remove a plugin from the local trust store
    Revoke {
        id: String,
        #[arg(long)]
        approve: bool,
    },
    /// Invoke one enabled trusted WASM plugin with a JSON value
    Invoke {
        /// Plugin manifest id
        id: String,
        /// JSON input passed to the capability-free guest
        #[arg(long, default_value = "{}")]
        input: String,
        /// Confirm execution of third-party WASM code
        #[arg(long)]
        approve: bool,
    },
    /// Connect one enabled trusted MCP plugin through the ADE MCP host
    ConnectMcp {
        /// Plugin manifest id
        id: String,
        /// Confirm spawning this reviewed MCP command
        #[arg(long)]
        approve: bool,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon as a detached background process
    Start {
        /// Loopback socket address (non-loopback binds are refused)
        #[arg(long, default_value = "127.0.0.1:3210")]
        bind: String,
        /// Workspace root (defaults to the current directory)
        #[arg(long)]
        root: Option<String>,
    },
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status {
        /// Emit JSON instead of prose
        #[arg(long)]
        json: bool,
    },
    /// Run the daemon in the foreground (used internally by `daemon start` / OS services)
    Run {
        /// Loopback socket address (non-loopback binds are refused)
        #[arg(long, default_value = "127.0.0.1:3210")]
        bind: String,
        /// Workspace root (defaults to the current directory)
        #[arg(long)]
        root: Option<String>,
    },
    /// Install an OS autostart service (launchd/systemd/Windows SCM)
    Install {
        /// Loopback socket address (non-loopback binds are refused)
        #[arg(long, default_value = "127.0.0.1:3210")]
        bind: String,
        /// Workspace root pinned into the service args (defaults to cwd)
        #[arg(long)]
        root: Option<String>,
        /// Confirm OS service registration
        #[arg(long)]
        approve: bool,
    },
    /// Remove the OS autostart service
    Uninstall {
        /// Confirm OS service removal
        #[arg(long)]
        approve: bool,
    },
    /// Show whether the OS autostart service is installed
    ServiceStatus {
        /// Emit JSON instead of prose
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum WorkspaceAction {
    /// List registered workspaces
    List,
    /// Register a workspace
    Create {
        name: String,
        /// Filesystem root of the workspace (defaults to the current directory)
        #[arg(long)]
        root: Option<String>,
        /// Stack recipe associated with this workspace
        #[arg(long)]
        recipe: Option<String>,
    },
    /// Remove a workspace by id or name
    Delete { id: String },
}

#[derive(Subcommand)]
enum McpAction {
    /// Expose ADE state over MCP stdio (set ADE_MCP_TOKEN to require request metadata auth)
    Serve,
    /// Spawn a server, list its tools, then shut it down
    Tools {
        /// Registry name for the server (1-64 letters, digits, '.', '-', '_')
        #[arg(long)]
        name: String,
        /// Executable to spawn (no shell interpretation)
        #[arg(long)]
        command: String,
        /// Argument passed to the executable (repeatable)
        #[arg(long = "arg", allow_hyphen_values = true)]
        args: Vec<String>,
        /// Confirm you reviewed this exact command and argument list
        #[arg(long)]
        approve: bool,
        /// Seconds to wait for the server to initialize
        #[arg(long, default_value_t = 15)]
        timeout: u64,
    },
    /// Spawn a server, call one tool, print the result, then shut it down
    Call {
        /// Registry name for the server (1-64 letters, digits, '.', '-', '_')
        #[arg(long)]
        name: String,
        /// Executable to spawn (no shell interpretation)
        #[arg(long)]
        command: String,
        /// Argument passed to the executable (repeatable)
        #[arg(long = "arg", allow_hyphen_values = true)]
        args: Vec<String>,
        /// Tool to invoke on the server
        #[arg(long)]
        tool: String,
        /// Tool arguments as a JSON object (default: {})
        #[arg(long = "args-json", default_value = "{}")]
        args_json: String,
        /// Print the raw content blocks as JSON instead of text
        #[arg(long)]
        json: bool,
        /// Confirm you reviewed this exact command and argument list
        #[arg(long)]
        approve: bool,
        /// Seconds to wait for the server to initialize
        #[arg(long, default_value_t = 15)]
        timeout: u64,
    },
}

#[derive(Subcommand)]
enum KeysAction {
    /// Store or replace a provider key (prompted with hidden input)
    Set {
        provider: String,
        #[arg(long)]
        profile: Option<String>,
    },
    /// Check whether a provider key is configured (never prints the key)
    Status {
        provider: String,
        #[arg(long)]
        profile: Option<String>,
    },
    /// Permanently remove a provider key from the OS credential vault
    Delete {
        provider: String,
        #[arg(long)]
        profile: Option<String>,
        /// Confirm permanent credential deletion
        #[arg(long)]
        confirm: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ade_core::config::AdeConfig::load()?;
    // Respect an explicit RUST_LOG, otherwise fall back to the profile default.
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", &config.log_level);
    }
    tracing_subscriber::fmt::init();
    tracing::info!(
        environment = %config.environment,
        data_dir = %config.data_dir.display(),
        "ADE starting"
    );

    let cli = Cli::parse();

    match &cli.command {
        Commands::Audit { mode } => {
            let mode: ade_core::audit::AuditMode = mode
                .as_deref()
                .unwrap_or("evaluate_existing")
                .parse()
                .map_err(anyhow::Error::msg)?;
            let root = std::env::current_dir()?;
            let report = ade_core::audit::AuditRunner::new(root).run(mode);
            println!(
                "AUDIT complete — score {}/{} (mode={}, root={})",
                report.score, report.score_max, report.mode, report.root
            );
            if !report.blockers.is_empty() {
                println!("Blockers:");
                for b in &report.blockers {
                    println!("  - {b}");
                }
            }
            if let Some(summary) = &report.human_summary_markdown {
                println!("\n{summary}");
            }
            let out = config.data_dir.join("last-audit.json");
            std::fs::create_dir_all(&config.data_dir)?;
            std::fs::write(&out, serde_json::to_string_pretty(&report)?)?;
            persist_report(
                &config,
                ade_db::reports::ReportKind::Audit,
                ade_core::audit::AUDIT_SCHEMA,
                &report.root,
                &report,
            )
            .await?;
            record_event(&config, "audit_run", Some(&report.root), None).await;
            println!("Wrote {}", out.display());
        }
        Commands::Plan => {
            // Reuse the last audit if one exists for this root; otherwise run a fresh one.
            let root = std::env::current_dir()?;
            let last_audit_path = config.data_dir.join("last-audit.json");
            let audit: ade_core::audit::AuditReport =
                match std::fs::read_to_string(&last_audit_path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<ade_core::audit::AuditReport>(&s).ok())
                    .filter(|a| a.root == root.display().to_string())
                {
                    Some(report) => {
                        println!("Using last audit from {}", last_audit_path.display());
                        report
                    }
                    None => {
                        println!("No matching audit found — running a fresh AUDIT first");
                        ade_core::audit::AuditRunner::new(&root)
                            .run(ade_core::audit::AuditMode::EvaluateExisting)
                    }
                };

            let plan = ade_core::plan::PlanBuilder::new().build(&audit);
            println!(
                "PLAN complete — {} phase(s) from audit score {}/{}",
                plan.phases.len(),
                plan.score_before,
                plan.score_max
            );
            if let Some(summary) = &plan.human_summary_markdown {
                println!("\n{summary}");
            }
            let out = config.data_dir.join("last-plan.json");
            std::fs::create_dir_all(&config.data_dir)?;
            std::fs::write(&out, serde_json::to_string_pretty(&plan)?)?;
            persist_report(
                &config,
                ade_db::reports::ReportKind::Plan,
                ade_core::plan::PLAN_SCHEMA,
                &plan.audit_root,
                &plan,
            )
            .await?;
            record_event(&config, "plan_run", Some(&plan.audit_root), None).await;
            println!("Wrote {}", out.display());
        }
        Commands::Execute {
            approve,
            recipe,
            phases,
        } => {
            let root = std::env::current_dir()?;
            let last_plan_path = config.data_dir.join("last-plan.json");
            let plan: ade_core::plan::PlanReport = match std::fs::read_to_string(&last_plan_path)
                .ok()
                .and_then(|s| serde_json::from_str::<ade_core::plan::PlanReport>(&s).ok())
                .filter(|p| p.audit_root == root.display().to_string())
            {
                Some(plan) => {
                    println!("Using last plan from {}", last_plan_path.display());
                    plan
                }
                None => {
                    anyhow::bail!(
                        "No matching plan at {} — run `ade plan` first for this root",
                        last_plan_path.display()
                    );
                }
            };

            let opts = ade_core::execute::ExecuteOptions {
                approved: *approve,
                recipe_id: recipe.clone(),
                phase_ids: phases.clone(),
            };
            let report = ade_core::execute::ExecuteRunner::new(&root).run(&plan, &opts)?;
            println!(
                "EXECUTE complete — score {:?} → {:?} / {} (changed {} path(s))",
                report.score_before,
                report.score_after,
                report.score_max,
                report.changed_paths.len()
            );
            if let Some(summary) = &report.human_summary_markdown {
                println!("\n{summary}");
            }
            let out = config.data_dir.join("last-execute.json");
            std::fs::create_dir_all(&config.data_dir)?;
            std::fs::write(&out, serde_json::to_string_pretty(&report)?)?;
            persist_report(
                &config,
                ade_db::reports::ReportKind::Execute,
                ade_core::execute::EXECUTE_SCHEMA,
                &root.display().to_string(),
                &report,
            )
            .await?;
            record_event(
                &config,
                "execute_run",
                Some(&root.display().to_string()),
                Some(recipe),
            )
            .await;
            println!("Wrote {}", out.display());
            let mut capsule = ade_core::handoff::HandoffCapsule::from_execute(
                "Continue approved ADE plan",
                &report,
            );
            capsule.branch = current_branch(&root);
            let handoff_id =
                ade_agents::handoff::HandoffManager::new(&root).save_capsule(&capsule)?;
            println!("Wrote handoff capsule {handoff_id}");
        }
        Commands::Init {
            recipe,
            force,
            name,
        } => {
            let recipe_id = recipe.as_deref().unwrap_or("business-saas");
            let recipe = ade_core::recipe::builtin_recipe(recipe_id)?;
            let root = std::env::current_dir()?;
            let project_name = name.clone().unwrap_or_else(|| {
                root.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("project")
                    .to_string()
            });
            let ctx = ade_core::agents_contract::AgentsContractContext::new(project_name)
                .with_root(root.display().to_string());
            let result = ade_core::scaffold::RecipeScaffold::apply(&root, &recipe, &ctx, *force)?;
            println!(
                "Initialized recipe '{}' — agents={}",
                result.recipe_id, result.agents_path
            );
            if result.recovered_interrupted {
                println!("Recovered an interrupted scaffold transaction before apply");
            }
            for file in &result.files {
                let action = match file.action {
                    ade_core::scaffold::ScaffoldAction::Create => "create",
                    ade_core::scaffold::ScaffoldAction::Update => "update",
                    ade_core::scaffold::ScaffoldAction::Preserve => "preserve",
                };
                println!("  {action:<8} {}", file.relative);
            }
        }
        Commands::Recipes => {
            println!("Built-in stack recipes:");
            for r in ade_core::recipe::builtin_recipes() {
                println!("  {:<18} {} — {}", r.id, r.name, r.description);
            }
        }
        Commands::Verify { gate, through } => {
            let gate: ade_core::verify::VerifyGate = gate
                .as_deref()
                .unwrap_or("G0")
                .parse()
                .map_err(anyhow::Error::msg)?;
            let runner = ade_workflow::verify::VerifyRunner::new();
            let results = if *through {
                runner.run_through(gate).await
            } else {
                vec![runner.run_gate(gate).await]
            };

            for result in &results {
                let label = if result.passed {
                    "PASS"
                } else {
                    match result.status {
                        ade_core::verify::VerifyStatus::Unavailable => "UNAVAILABLE",
                        ade_core::verify::VerifyStatus::Skipped => "SKIPPED",
                        _ => "FAIL",
                    }
                };
                println!("{} {} — {}", label, result.gate, result.command);
                if let Some(stdout) = &result.stdout {
                    if !stdout.trim().is_empty() {
                        println!("{stdout}");
                    }
                }
                if let Some(stderr) = &result.stderr {
                    if !stderr.trim().is_empty() {
                        eprintln!("{stderr}");
                    }
                }
            }

            let out = config.data_dir.join("last-verify.json");
            std::fs::create_dir_all(&config.data_dir)?;
            std::fs::write(&out, serde_json::to_string_pretty(&results)?)?;
            let root = std::env::current_dir()?;
            persist_report(
                &config,
                ade_db::reports::ReportKind::Verify,
                ade_core::verify::VERIFY_SCHEMA,
                &root.display().to_string(),
                &results,
            )
            .await?;
            record_event(
                &config,
                "verify_run",
                Some(&root.display().to_string()),
                Some(gate.id()),
            )
            .await;
            println!("Wrote {}", out.display());
            let manager = ade_agents::handoff::HandoffManager::new(&root);
            let mut capsule = manager.load_latest().unwrap_or_else(|_| {
                ade_core::handoff::HandoffCapsule::new(
                    "Continue after workspace verification",
                    "evaluate_existing",
                )
            });
            capsule.branch = current_branch(&root);
            capsule.apply_verify_results(&results);
            let handoff_id = manager.save_capsule(&capsule)?;
            println!("Wrote handoff capsule {handoff_id}");

            if results.iter().any(|result| !result.passed) {
                anyhow::bail!("verification failed");
            }
        }
        Commands::Keys { action } => match action {
            KeysAction::Set { provider, profile } => {
                let profile = profile
                    .clone()
                    .unwrap_or_else(|| config.environment.to_string());
                let vault = ade_db::secrets::SecretsVault::for_profile(&profile)?;
                let secret = rpassword::prompt_password(format!(
                    "Enter API key for {provider} ({profile}; input hidden): "
                ))?;
                vault.set(provider, &secret)?;
                println!("Stored {provider} key in the OS credential vault ({profile})");
            }
            KeysAction::Status { provider, profile } => {
                let profile = profile
                    .clone()
                    .unwrap_or_else(|| config.environment.to_string());
                let vault = ade_db::secrets::SecretsVault::for_profile(&profile)?;
                println!(
                    "{} key is {} for profile {}",
                    provider,
                    if vault.contains(provider)? {
                        "configured"
                    } else {
                        "not configured"
                    },
                    profile
                );
            }
            KeysAction::Delete {
                provider,
                profile,
                confirm,
            } => {
                if !confirm {
                    anyhow::bail!(
                        "credential deletion is permanent; rerun with --confirm to proceed"
                    );
                }
                let profile = profile
                    .clone()
                    .unwrap_or_else(|| config.environment.to_string());
                let vault = ade_db::secrets::SecretsVault::for_profile(&profile)?;
                if vault.delete(provider)? {
                    println!("Deleted {provider} key from profile {profile}");
                } else {
                    println!("No {provider} key was configured for profile {profile}");
                }
            }
        },
        Commands::Mcp { action } => match action {
            McpAction::Serve => {
                let root = std::env::current_dir()?;
                let mut server = ade_agents::mcp_server::AdeMcpServer::new(root);
                if let Ok(token) = std::env::var("ADE_MCP_TOKEN") {
                    server = server.with_auth_token(token);
                }
                server.serve_stdio()?;
            }
            McpAction::Tools {
                name,
                command,
                args,
                approve,
                timeout,
            } => {
                let host = ade_agents::mcp::McpHost::with_timeout(std::time::Duration::from_secs(
                    *timeout,
                ));
                host.connect_server(ade_agents::mcp::McpServerConfig {
                    name: name.clone(),
                    command: command.clone(),
                    args: args.clone(),
                    approved: *approve,
                })
                .await?;
                let tools = host.list_tools().await?;
                if tools.is_empty() {
                    println!("{name} exposes no tools");
                } else {
                    println!("Tools exposed by {name}:");
                    for tool in &tools {
                        println!("  {:<28} {}", tool.name, tool.description);
                    }
                }
                host.disconnect_server(name).await?;
            }
            McpAction::Call {
                name,
                command,
                args,
                tool,
                args_json,
                json,
                approve,
                timeout,
            } => {
                let arguments: serde_json::Value = serde_json::from_str(args_json)
                    .map_err(|error| anyhow::anyhow!("--args-json is not valid JSON: {error}"))?;
                let host = ade_agents::mcp::McpHost::with_timeout(std::time::Duration::from_secs(
                    *timeout,
                ));
                host.connect_server(ade_agents::mcp::McpServerConfig {
                    name: name.clone(),
                    command: command.clone(),
                    args: args.clone(),
                    approved: *approve,
                })
                .await?;
                let root = std::env::current_dir()?;
                ade_agents::authority::AuthorityEnforcer::load(&root, Vec::<String>::new())?
                    .authorize_human_tool(name, tool, &arguments)?;
                let result = host.call_tool(name, tool, arguments).await;
                host.disconnect_server(name).await?;
                let result = result?;

                if *json || result.text.is_empty() {
                    println!("{}", serde_json::to_string_pretty(&result.content)?);
                } else {
                    println!("{}", result.text);
                }
                record_event(&config, "mcp_call", None, Some(&format!("{name}/{tool}"))).await;
                if result.is_error {
                    anyhow::bail!("tool '{tool}' reported an error");
                }
            }
        },
        Commands::Workspace { action } => {
            let store = ade_db::workspace::WorkspaceStore::new(open_database(&config).await?);
            match action {
                Some(WorkspaceAction::List) | None => {
                    let workspaces = store.list().await?;
                    if workspaces.is_empty() {
                        println!("No workspaces registered — use `ade workspace create <name>`");
                    } else {
                        println!("Workspaces:");
                        for workspace in &workspaces {
                            println!(
                                "  {:<24} {}  root={}  recipe={}",
                                workspace.name,
                                workspace.id,
                                workspace.root_path.as_deref().unwrap_or("-"),
                                workspace.recipe_id.as_deref().unwrap_or("-"),
                            );
                        }
                    }
                }
                Some(WorkspaceAction::Create { name, root, recipe }) => {
                    let default_root = std::env::current_dir()?.display().to_string();
                    let root = root.clone().unwrap_or(default_root);
                    let workspace = store.create(name, Some(&root), recipe.as_deref()).await?;
                    println!(
                        "Created workspace '{}' ({}) at {}",
                        workspace.name, workspace.id, root
                    );
                }
                Some(WorkspaceAction::Delete { id }) => {
                    if store.delete(id).await? {
                        println!("Deleted workspace '{id}'");
                    } else {
                        anyhow::bail!("no workspace matches '{id}'");
                    }
                }
            }
        }
        Commands::Analytics => {
            let store = ade_db::analytics::AnalyticsStore::new(open_database(&config).await?);
            let summary = store.summary().await?;
            if summary.is_empty() {
                println!("No events recorded yet — run audit/plan/execute/verify first");
            } else {
                println!("{:<16} {:>8}  Last seen (UTC)", "Event", "Count");
                for entry in &summary {
                    println!(
                        "{:<16} {:>8}  {}",
                        entry.event_type,
                        entry.count,
                        entry.last_seen.format("%Y-%m-%d %H:%M:%S")
                    );
                }
                println!();
                println!("Recent events:");
                for event in store.recent(5).await? {
                    println!(
                        "  {}  {:<16} {}",
                        event.created_at.format("%H:%M:%S"),
                        event.event_type,
                        event.detail.unwrap_or_default()
                    );
                }
            }
        }
        Commands::Worktree { action } => {
            let root = std::env::current_dir()?;
            let manager = ade_workflow::parallel::WorktreeManager::new(&root);
            match action {
                WorktreeAction::List { json } => {
                    let items = manager.list()?;
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&items)?);
                    } else if items.is_empty() {
                        println!("No worktrees found");
                    } else {
                        for item in &items {
                            println!(
                                "{:<48}  {}",
                                item.path,
                                item.branch
                                    .as_deref()
                                    .unwrap_or(item.head.as_deref().unwrap_or("-"))
                            );
                        }
                    }
                }
                WorktreeAction::Add {
                    path,
                    branch,
                    start_point,
                    approve,
                } => {
                    if !approve {
                        anyhow::bail!("worktree add mutates git state; rerun with --approve");
                    }
                    let info =
                        manager.add(std::path::Path::new(path), branch, start_point.as_deref())?;
                    println!(
                        "Added worktree {} on {}",
                        info.path,
                        info.branch.as_deref().unwrap_or("-")
                    );
                }
                WorktreeAction::Remove {
                    path,
                    approve,
                    force,
                } => {
                    if !approve {
                        anyhow::bail!("worktree remove mutates git state; rerun with --approve");
                    }
                    manager.remove(std::path::Path::new(path), *force)?;
                    println!("Removed worktree {path}");
                }
            }
        }
        Commands::Lease { action } => {
            let root = std::env::current_dir()?;
            let manager = ade_workflow::parallel::LeaseManager::new(&root);
            match action {
                LeaseAction::List { json } => {
                    let leases = manager.list()?;
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&leases)?);
                    } else if leases.is_empty() {
                        println!("No active path leases");
                    } else {
                        println!("{:<38} {:<12} {:<10} PATH", "ID", "MODE", "PROTECTED");
                        for lease in &leases {
                            println!(
                                "{:<38} {:<12} {:<10} {}  agent={}",
                                lease.id,
                                lease.mode.as_str(),
                                if lease.protected { "yes" } else { "no" },
                                lease.path,
                                lease.agent_id
                            );
                        }
                    }
                }
                LeaseAction::Acquire {
                    agent,
                    path,
                    mode,
                    ttl_secs,
                    approve,
                } => {
                    if !approve {
                        anyhow::bail!("lease acquire mutates ownership; rerun with --approve");
                    }
                    let agent_id = uuid::Uuid::parse_str(agent)
                        .map_err(|error| anyhow::anyhow!("invalid --agent uuid: {error}"))?;
                    let mode = ade_workflow::parallel::LeaseMode::parse(mode)?;
                    let ttl = chrono::Duration::seconds(*ttl_secs);
                    let lease = manager.acquire(agent_id, path, mode, ttl)?;
                    println!(
                        "Acquired {} lease {} on {} until {}",
                        lease.mode.as_str(),
                        lease.id,
                        lease.path,
                        lease.expires_at.to_rfc3339()
                    );
                }
                LeaseAction::Renew {
                    id,
                    agent,
                    ttl_secs,
                    approve,
                } => {
                    if !approve {
                        anyhow::bail!("lease renew mutates ownership; rerun with --approve");
                    }
                    let agent_id = uuid::Uuid::parse_str(agent)
                        .map_err(|error| anyhow::anyhow!("invalid --agent uuid: {error}"))?;
                    let ttl = chrono::Duration::seconds(*ttl_secs);
                    let lease = manager.renew(agent_id, id, ttl)?;
                    println!(
                        "Renewed {} lease {} on {} until {}",
                        lease.mode.as_str(),
                        lease.id,
                        lease.path,
                        lease.expires_at.to_rfc3339()
                    );
                }
                LeaseAction::Release { id, approve } => {
                    if !approve {
                        anyhow::bail!("lease release mutates ownership; rerun with --approve");
                    }
                    if manager.release(id)? {
                        println!("Released lease {id}");
                    } else {
                        anyhow::bail!("no lease matches '{id}'");
                    }
                }
                LeaseAction::ReleaseStale { approve } => {
                    if !approve {
                        anyhow::bail!(
                            "lease release-stale mutates ownership; rerun with --approve"
                        );
                    }
                    let removed = manager.release_stale()?;
                    println!("Released {removed} stale lease(s)");
                }
            }
        }
        Commands::Task { action } => {
            use ade_workflow::tasks::{EnqueueTask, TaskCoordinator};

            let root = std::env::current_dir()?;
            let coordinator = TaskCoordinator::new(&root);
            match action {
                TaskAction::List { json } => {
                    let tasks = coordinator.list()?;
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&tasks)?);
                    } else if tasks.is_empty() {
                        println!("No coordinated tasks");
                    } else {
                        println!("{:<38} {:<10} {:<38} GOAL", "ID", "STATUS", "AGENT");
                        for task in tasks {
                            println!(
                                "{:<38} {:<10} {:<38} {}",
                                task.id,
                                format!("{:?}", task.status).to_ascii_lowercase(),
                                task.agent_id
                                    .map(|id| id.to_string())
                                    .unwrap_or_else(|| "-".into()),
                                task.goal
                            );
                        }
                    }
                }
                TaskAction::Enqueue {
                    goal,
                    owned_paths,
                    mode,
                    depends_on,
                    approve,
                } => {
                    require_approval(*approve, "task enqueue")?;
                    let task = coordinator.enqueue(EnqueueTask {
                        goal: goal.clone(),
                        owned_paths: owned_paths.clone(),
                        lease_mode: ade_workflow::parallel::LeaseMode::parse(mode)?,
                        depends_on: depends_on.clone(),
                    })?;
                    println!("Enqueued task {}: {}", task.id, task.goal);
                }
                TaskAction::Claim {
                    agent,
                    ttl_secs,
                    approve,
                } => {
                    require_approval(*approve, "task claim")?;
                    let agent_id = parse_agent_id(agent)?;
                    match coordinator.claim(agent_id, chrono::Duration::seconds(*ttl_secs))? {
                        Some(task) => {
                            println!("{}", serde_json::to_string_pretty(&task)?);
                        }
                        None => println!("No dependency-ready task available"),
                    }
                }
                TaskAction::Start { id, agent, approve } => {
                    require_approval(*approve, "task start")?;
                    let task = coordinator.start(id, parse_agent_id(agent)?)?;
                    println!("Started task {}", task.id);
                }
                TaskAction::Heartbeat {
                    id,
                    agent,
                    ttl_secs,
                    approve,
                } => {
                    require_approval(*approve, "task heartbeat")?;
                    let task = coordinator.heartbeat(
                        id,
                        parse_agent_id(agent)?,
                        chrono::Duration::seconds(*ttl_secs),
                    )?;
                    println!(
                        "Renewed task {} until {}",
                        task.id,
                        task.expires_at
                            .map(|expiry| expiry.to_rfc3339())
                            .unwrap_or_else(|| "-".into())
                    );
                }
                TaskAction::Complete { id, agent, approve } => {
                    require_approval(*approve, "task complete")?;
                    coordinator.complete(id, parse_agent_id(agent)?)?;
                    println!("Completed task {id} and released its leases");
                }
                TaskAction::Fail {
                    id,
                    agent,
                    reason,
                    approve,
                } => {
                    require_approval(*approve, "task fail")?;
                    coordinator.fail(id, parse_agent_id(agent)?, reason)?;
                    println!("Failed task {id} and released its leases");
                }
                TaskAction::Cancel { id, approve } => {
                    require_approval(*approve, "task cancel")?;
                    coordinator.cancel(id)?;
                    println!("Cancelled task {id} and released its leases");
                }
                TaskAction::RequeueExpired { approve } => {
                    require_approval(*approve, "task requeue-expired")?;
                    let count = coordinator.requeue_expired()?;
                    println!("Requeued {count} expired task claim(s)");
                }
            }
        }
        Commands::Worker { action } => match action {
            WorkerAction::Run {
                agent,
                provider,
                base_url,
                model,
                input_cost_per_mtok,
                output_cost_per_mtok,
                context_limit,
                output_limit,
                profile,
                ttl_secs,
                poll_ms,
                worktree,
                cleanup_worktree,
                approve,
            } => {
                require_approval(*approve, "worker run")?;
                let root = std::env::current_dir()?;
                let profile = profile
                    .clone()
                    .unwrap_or_else(|| config.environment.to_string());
                let worker =
                    ade_service::worker::AgentTurnWorker::new(ade_service::worker::WorkerConfig {
                        workspace_root: root,
                        agent_id: parse_agent_id(agent)?,
                        provider: provider.clone(),
                        base_url: base_url.clone(),
                        model: model.clone(),
                        input_cost_per_mtok: ade_core::money::Money::try_from_usd_f64(
                            *input_cost_per_mtok,
                        )?,
                        output_cost_per_mtok: ade_core::money::Money::try_from_usd_f64(
                            *output_cost_per_mtok,
                        )?,
                        context_limit: *context_limit,
                        output_limit: *output_limit,
                        profile,
                        ttl_secs: *ttl_secs,
                        poll_interval: std::time::Duration::from_millis(*poll_ms),
                        provision_worktree: *worktree,
                        cleanup_worktree: *cleanup_worktree,
                    });
                println!(
                    "ADE worker running as {} (worktree={} cleanup={})",
                    agent, worktree, cleanup_worktree
                );
                worker.run().await?;
            }
        },
        Commands::Plugin { action } => {
            use ade_plugins::manifest::PluginKind;
            use ade_plugins::mcp_ext::McpPluginLoader;
            use ade_plugins::trust::{sha256_hex, verify_artifact, PluginTrustStore, TrustEntry};

            let root = std::env::current_dir()?;
            let registry = ade_plugins::registry::PluginRegistry::from_workspace(&root);
            let trust_store = PluginTrustStore::from_workspace(&root);
            match action {
                PluginAction::List { json } => {
                    let plugins = registry.discover()?;
                    let trusted = trust_store.list()?;
                    if *json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "plugins": plugins,
                                "trust": trusted,
                            }))?
                        );
                    } else if plugins.is_empty() {
                        println!("No plugins found under .ade/plugins");
                    } else {
                        println!(
                            "{:<28} {:<8} {:<8} {:<8} ARTIFACT",
                            "ID", "KIND", "ENABLED", "TRUSTED"
                        );
                        for plugin in plugins {
                            let trusted = trusted
                                .iter()
                                .any(|entry| entry.plugin_id == plugin.manifest.id);
                            let artifact = match plugin.manifest.kind {
                                PluginKind::Wasm => plugin
                                    .module_path
                                    .as_ref()
                                    .map(|path| path.display().to_string())
                                    .unwrap_or_else(|| "-".into()),
                                PluginKind::Mcp => plugin
                                    .manifest
                                    .mcp
                                    .as_ref()
                                    .map(|mcp| format!("{} {:?}", mcp.command, mcp.args))
                                    .unwrap_or_else(|| "-".into()),
                            };
                            println!(
                                "{:<28} {:<8} {:<8} {:<8} {}",
                                plugin.manifest.id,
                                format!("{:?}", plugin.manifest.kind).to_ascii_lowercase(),
                                if plugin.manifest.enabled { "yes" } else { "no" },
                                if trusted { "yes" } else { "no" },
                                artifact
                            );
                        }
                    }
                }
                PluginAction::Trust {
                    id,
                    pubkey,
                    pin_digest,
                    approve,
                } => {
                    require_approval(*approve, "plugin trust")?;
                    let plugins = registry.discover()?;
                    let plugin = plugins
                        .iter()
                        .find(|plugin| plugin.manifest.id == *id)
                        .ok_or_else(|| anyhow::anyhow!("plugin '{id}' was not discovered"))?;
                    let digest = if *pin_digest {
                        Some(sha256_hex(
                            &plugin.manifest.artifact_bytes(&plugin.manifest_path)?,
                        ))
                    } else {
                        None
                    };
                    let entry = trust_store.trust(TrustEntry {
                        plugin_id: id.clone(),
                        digest,
                        pubkey: pubkey.clone(),
                    })?;
                    println!(
                        "Trusted plugin {} (digest={})",
                        entry.plugin_id,
                        entry.digest.as_deref().unwrap_or("unpinned")
                    );
                }
                PluginAction::Revoke { id, approve } => {
                    require_approval(*approve, "plugin revoke")?;
                    if trust_store.revoke(id)? {
                        println!("Revoked trust for plugin {id}");
                    } else {
                        anyhow::bail!("no trust entry for '{id}'");
                    }
                }
                PluginAction::Invoke { id, input, approve } => {
                    require_approval(*approve, "plugin invoke")?;
                    let plugins = registry.discover()?;
                    let plugin = plugins
                        .iter()
                        .find(|plugin| plugin.manifest.id == *id)
                        .ok_or_else(|| anyhow::anyhow!("plugin '{id}' was not discovered"))?;
                    let input: serde_json::Value = serde_json::from_str(input)
                        .map_err(|error| anyhow::anyhow!("invalid --input JSON: {error}"))?;
                    let mut host = ade_plugins::wasm::WasmPluginHost::new()?;
                    host.load(plugin, &trust_store)?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&host.invoke(id, &input)?)?
                    );
                }
                PluginAction::ConnectMcp { id, approve } => {
                    require_approval(*approve, "plugin connect-mcp")?;
                    let plugins = registry.discover()?;
                    let plugin = plugins
                        .iter()
                        .find(|plugin| plugin.manifest.id == *id)
                        .ok_or_else(|| anyhow::anyhow!("plugin '{id}' was not discovered"))?;
                    let trust = trust_store.get(id)?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "plugin '{id}' is not trusted; run `ade plugin trust {id} --approve`"
                        )
                    })?;
                    let artifact = plugin.manifest.artifact_bytes(&plugin.manifest_path)?;
                    verify_artifact(
                        &plugin.manifest.id,
                        &plugin.manifest.version,
                        &artifact,
                        plugin.manifest.digest.as_deref(),
                        plugin.manifest.signature.as_deref(),
                        &trust,
                    )?;
                    let config = McpPluginLoader::new().load(plugin)?;
                    let host = ade_agents::mcp::McpHost::new();
                    host.connect_server(ade_agents::mcp::McpServerConfig {
                        name: config.name.clone(),
                        command: config.command.clone(),
                        args: config.args.clone(),
                        approved: true,
                    })
                    .await?;
                    println!(
                        "Connected trusted MCP plugin {} via {} {:?}",
                        config.name, config.command, config.args
                    );
                }
            }
        }
        Commands::Serve { bind } => {
            let address = parse_loopback_bind(bind)?;
            let root = std::env::current_dir()?;
            let (auth_token, auth_scopes) = api_auth_from_env()?;
            let service =
                ade_service::runtime::BoundService::bind(ade_service::runtime::ServiceConfig {
                    workspace_root: root,
                    bind: address,
                    auth_token,
                    auth_scopes,
                })
                .await?;
            let local = service.local_addr();
            println!(
                "ADE API listening on http://{} (auth={})",
                local,
                if service.auth_required() {
                    "bearer token required"
                } else {
                    "loopback reads open; writes require ADE_API_TOKEN"
                }
            );
            service
                .serve(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await?;
        }
        Commands::Daemon { action } => match action {
            DaemonAction::Start { bind, root } => {
                let workspace = resolve_workspace_root(root.as_deref())?;
                let lifecycle = ade_service::lifecycle::DaemonLifecycle::new(&workspace);
                let address = parse_loopback_bind(bind)?;
                let (token, _) = api_auth_from_env()?;
                let state = lifecycle.start_detached(address, token.as_deref())?;
                println!(
                    "ADE daemon started (pid {}) on http://{} (auth={}). Logs: {}",
                    state.pid,
                    state.bind,
                    if state.auth_required {
                        "bearer token required"
                    } else {
                        "loopback reads open; writes require ADE_API_TOKEN"
                    },
                    lifecycle.log_path().display()
                );
            }
            DaemonAction::Stop => {
                let workspace = std::env::current_dir()?;
                let lifecycle = ade_service::lifecycle::DaemonLifecycle::new(&workspace);
                let pid = lifecycle.stop()?;
                println!("ADE daemon stopped (pid {pid})");
            }
            DaemonAction::Status { json } => {
                let workspace = std::env::current_dir()?;
                let lifecycle = ade_service::lifecycle::DaemonLifecycle::new(&workspace);
                let status = lifecycle.status();
                if *json {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                } else if status.running {
                    println!(
                        "ADE daemon running (pid {}) on http://{} since {} (auth={})",
                        status.pid.unwrap_or_default(),
                        status.bind.as_deref().unwrap_or("unknown"),
                        status.started_at.as_deref().unwrap_or("unknown"),
                        match status.auth_required {
                            Some(true) => "bearer token required",
                            Some(false) => "loopback reads open; writes require ADE_API_TOKEN",
                            None => "unknown",
                        }
                    );
                } else {
                    println!("ADE daemon is not running. Logs: {}", status.log_path);
                }
            }
            DaemonAction::Run { bind, root } => {
                let workspace = resolve_workspace_root(root.as_deref())?;
                let lifecycle = ade_service::lifecycle::DaemonLifecycle::new(&workspace);
                let address = parse_loopback_bind(bind)?;
                let (token, auth_scopes) = api_auth_from_env()?;
                let scheduler_root = workspace.clone();
                tokio::spawn(async move {
                    ade_service::scheduler::Scheduler::new(scheduler_root)
                        .run()
                        .await;
                });
                let service =
                    ade_service::runtime::BoundService::bind(ade_service::runtime::ServiceConfig {
                        workspace_root: workspace.clone(),
                        bind: address,
                        auth_token: token,
                        auth_scopes,
                    })
                    .await?;
                let local = service.local_addr();
                lifecycle.mark_running(local, service.auth_required())?;
                println!(
                    "ADE daemon serving on http://{} (auth={})",
                    local,
                    if service.auth_required() {
                        "bearer token required"
                    } else {
                        "loopback reads open; writes require ADE_API_TOKEN"
                    }
                );
                let outcome = service
                    .serve(async {
                        let _ = tokio::signal::ctrl_c().await;
                    })
                    .await;
                lifecycle.mark_stopped();
                outcome?;
            }
            DaemonAction::Install {
                bind,
                root,
                approve,
            } => {
                require_approval(*approve, "daemon install")?;
                let workspace = resolve_workspace_root(root.as_deref())?;
                let address = parse_loopback_bind(bind)?;
                let daemon = ade_service::daemon::AdeDaemon::for_workspace(&workspace, address);
                daemon.install_service().map_err(|error| {
                    anyhow::anyhow!("failed to install OS service '{}': {error}", daemon.name())
                })?;
                println!(
                    "Installed OS service '{}' for workspace {} on {}",
                    daemon.name(),
                    daemon.workspace_root().display(),
                    daemon.bind()
                );
                println!(
                        "Set ADE_API_TOKEN (and optional ADE_API_SCOPES) in the service environment before starting it."
                    );
            }
            DaemonAction::Uninstall { approve } => {
                require_approval(*approve, "daemon uninstall")?;
                let workspace = std::env::current_dir()?;
                let daemon = ade_service::daemon::AdeDaemon::for_workspace(
                    &workspace,
                    std::net::SocketAddr::from(([127, 0, 0, 1], 3210)),
                );
                daemon.uninstall_service().map_err(|error| {
                    anyhow::anyhow!(
                        "failed to uninstall OS service '{}': {error}",
                        daemon.name()
                    )
                })?;
                println!("Uninstalled OS service '{}'", daemon.name());
            }
            DaemonAction::ServiceStatus { json } => {
                let workspace = std::env::current_dir()?;
                let daemon = ade_service::daemon::AdeDaemon::for_workspace(
                    &workspace,
                    std::net::SocketAddr::from(([127, 0, 0, 1], 3210)),
                );
                let installed = daemon.is_service_installed();
                if *json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "name": daemon.name(),
                            "installed": installed,
                            "workspace_root": daemon.workspace_root().display().to_string(),
                        })
                    );
                } else if installed {
                    println!(
                        "OS service '{}' is installed (workspace {})",
                        daemon.name(),
                        daemon.workspace_root().display()
                    );
                } else {
                    println!("OS service '{}' is not installed", daemon.name());
                }
            }
        },
        Commands::Smoke {
            profile,
            live,
            provider,
            base_url,
            model,
            input_cost_per_mtok,
            output_cost_per_mtok,
            context_limit,
            output_limit,
            max_cost_usd,
        } => {
            let profile = profile
                .clone()
                .unwrap_or_else(|| config.environment.to_string());
            let root = std::env::current_dir()?;
            let report = ade_agents::smoke::run_foundation_smoke(&root, &profile).await?;
            for check in &report.checks {
                println!(
                    "{} {:<24} {}",
                    if check.ok { "ok" } else { "FAIL" },
                    check.name,
                    check.detail
                );
            }
            if !report.ok {
                anyhow::bail!("foundation smoke failed");
            }
            println!("foundation smoke passed");
            if *live {
                let model = model
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("--model is required with --live"))?;
                let live_report =
                    ade_agents::smoke::run_live_agent_smoke(ade_agents::smoke::LiveSmokeSpec {
                        workspace_root: root,
                        profile,
                        provider: provider.clone(),
                        base_url: base_url.clone(),
                        model,
                        input_cost_per_mtok: ade_core::money::Money::try_from_usd_f64(
                            *input_cost_per_mtok,
                        )?,
                        output_cost_per_mtok: ade_core::money::Money::try_from_usd_f64(
                            *output_cost_per_mtok,
                        )?,
                        context_limit: *context_limit,
                        output_limit: *output_limit,
                        max_cost: ade_core::money::Money::try_from_usd_f64(*max_cost_usd)?,
                    })
                    .await?;
                let cost =
                    ade_core::money::Money::from_micros(live_report.cost_micros).format_usd();
                println!(
                    "live smoke {:?}: {} ({} input + {} output tokens, ${cost})",
                    live_report.status,
                    live_report.detail,
                    live_report.input_tokens,
                    live_report.output_tokens,
                );
                if live_report.status == ade_agents::smoke::LiveSmokeStatus::Failed {
                    anyhow::bail!("live provider smoke failed");
                }
            }
        }
        Commands::Agent {
            prompt,
            provider,
            base_url,
            model,
            input_cost_per_mtok,
            output_cost_per_mtok,
            context_limit,
            output_limit,
            profile,
            lease_agent,
        } => {
            use std::io::Write;

            let profile = profile
                .clone()
                .unwrap_or_else(|| config.environment.to_string());
            let root = std::env::current_dir()?;
            let input_cost = ade_core::money::Money::try_from_usd_f64(*input_cost_per_mtok)?;
            let output_cost = ade_core::money::Money::try_from_usd_f64(*output_cost_per_mtok)?;
            let ledger = ade_db::usage_ledger::UsageLedgerStore::new(open_database(&config).await?);
            let mut builder =
                ade_agents::turn::AgentTurnBuilder::new(ade_agents::turn::AgentTurnSpec {
                    prompt: prompt.clone(),
                    provider: provider.clone(),
                    base_url: base_url.clone(),
                    model: model.clone(),
                    input_cost_per_mtok: input_cost,
                    output_cost_per_mtok: output_cost,
                    context_limit: *context_limit,
                    output_limit: *output_limit,
                    profile,
                    workspace_root: root.clone(),
                    owned_paths: vec![],
                    handoff_chars: 1_500,
                })
                .ledger(ledger);
            if let Some(agent) = lease_agent {
                let agent_id = uuid::Uuid::parse_str(agent)
                    .map_err(|error| anyhow::anyhow!("invalid --lease-agent uuid: {error}"))?;
                builder = builder.lease_agent(agent_id);
            }
            let service = builder.prepare().await?;
            if !service.effective_owned_paths().is_empty() {
                eprintln!(
                    "lease-bound write scope: {}",
                    service.effective_owned_paths().join(", ")
                );
            }
            let mut events = service.start();
            let mut failure = None;
            let mut final_result = None;
            while let Some(event) = events.recv().await {
                match event {
                    ade_agents::session::AgentEvent::TextDelta { text } => {
                        print!("{text}");
                        std::io::stdout().flush()?;
                    }
                    ade_agents::session::AgentEvent::ToolCall { server, tool, .. } => {
                        eprintln!("\n→ tool {server}/{tool}");
                    }
                    ade_agents::session::AgentEvent::ToolResult {
                        server,
                        tool,
                        is_error,
                        ..
                    } => {
                        eprintln!(
                            "← {} {server}/{tool}",
                            if is_error { "error" } else { "ok" }
                        );
                    }
                    ade_agents::session::AgentEvent::SpendWarning {
                        scope,
                        period_key,
                        projected_micros,
                        soft_cap_micros,
                    } => {
                        eprintln!(
                            "\n! spend soft warning {scope}/{period_key}: ${} > soft ${}",
                            ade_core::money::Money::from_micros(projected_micros).format_usd(),
                            ade_core::money::Money::from_micros(soft_cap_micros).format_usd()
                        );
                    }
                    ade_agents::session::AgentEvent::Completed { result } => {
                        final_result = Some(result);
                    }
                    ade_agents::session::AgentEvent::Failed { error }
                    | ade_agents::session::AgentEvent::Cancelled { reason: error } => {
                        failure = Some(error);
                    }
                    _ => {}
                }
            }
            if let Some(error) = failure {
                anyhow::bail!("{error}");
            }
            let result = final_result
                .ok_or_else(|| anyhow::anyhow!("provider stream ended without a result"))?;
            let cost = ade_core::money::Money::from_micros(result.cost_micros);
            println!(
                "\n\n{} / {} · {} input + {} output tokens · ${}",
                result.provider,
                result.model,
                result.usage.input_tokens,
                result.usage.output_tokens,
                cost.format_usd()
            );
        }
    }

    Ok(())
}

async fn open_database(config: &ade_core::config::AdeConfig) -> anyhow::Result<turso::Connection> {
    let db_config = ade_db::repo::DbConfig::from_ade_config(config);
    let database = ade_db::repo::AdeDatabase::open(&db_config).await?;
    Ok(database.connect()?)
}

async fn record_event(
    config: &ade_core::config::AdeConfig,
    event_type: &str,
    workspace_root: Option<&str>,
    detail: Option<&str>,
) {
    // Analytics must never break a phase run; log and continue on failure.
    let result: anyhow::Result<()> = async {
        let store = ade_db::analytics::AnalyticsStore::new(open_database(config).await?);
        store.record(event_type, workspace_root, detail).await?;
        Ok(())
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, event_type, "failed to record analytics event");
    }
}

async fn persist_report<T: serde::Serialize>(
    config: &ade_core::config::AdeConfig,
    kind: ade_db::reports::ReportKind,
    schema: &str,
    workspace_root: &str,
    report: &T,
) -> anyhow::Result<()> {
    let db_config = ade_db::repo::DbConfig::from_ade_config(config);
    let database = ade_db::repo::AdeDatabase::open(&db_config).await?;
    let store = ade_db::reports::ReportStore::new(database.connect()?);
    store.save(kind, schema, workspace_root, report).await?;
    Ok(())
}

fn parse_loopback_bind(bind: &str) -> anyhow::Result<std::net::SocketAddr> {
    let address: std::net::SocketAddr = bind
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid --bind address: {error}"))?;
    if !address.ip().is_loopback() {
        anyhow::bail!(
            "ADE local API refuses non-loopback bind {address}; use a reverse proxy with explicit policy"
        );
    }
    Ok(address)
}

fn api_auth_from_env() -> anyhow::Result<(
    Option<String>,
    std::collections::HashSet<ade_service::runtime::ApiScope>,
)> {
    ade_service::runtime::ServiceConfig::auth_from_env().map_err(Into::into)
}

fn resolve_workspace_root(root: Option<&str>) -> anyhow::Result<std::path::PathBuf> {
    match root {
        Some(path) => {
            let path = std::path::PathBuf::from(path);
            if !path.is_dir() {
                anyhow::bail!("workspace root does not exist: {}", path.display());
            }
            Ok(path.canonicalize().unwrap_or(path))
        }
        None => Ok(std::env::current_dir()?),
    }
}

fn parse_agent_id(value: &str) -> anyhow::Result<uuid::Uuid> {
    uuid::Uuid::parse_str(value).map_err(|error| anyhow::anyhow!("invalid --agent uuid: {error}"))
}

fn require_approval(approved: bool, operation: &str) -> anyhow::Result<()> {
    if !approved {
        anyhow::bail!("{operation} mutates coordination state; rerun with --approve");
    }
    Ok(())
}

fn current_branch(root: &std::path::Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty())
}
