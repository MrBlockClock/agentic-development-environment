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
    /// Manage workspaces
    Workspace {
        #[command(subcommand)]
        action: Option<WorkspaceAction>,
    },
    /// Show usage and analytics
    Analytics,
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
            let path = ade_core::agents_contract::AgentsContractGenerator::write(
                &root, &recipe, &ctx, *force,
            )?;
            println!(
                "Initialized recipe '{}' — wrote {}",
                recipe.id,
                path.display()
            );
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
                println!(
                    "{} {} — {}",
                    if result.passed { "PASS" } else { "FAIL" },
                    result.gate,
                    result.command
                );
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
