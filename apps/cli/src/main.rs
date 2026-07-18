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
    List,
    Create { name: String },
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
        },
        Commands::Workspace { action } => match action {
            Some(WorkspaceAction::List) => println!("Workspaces: (none)"),
            Some(WorkspaceAction::Create { name }) => {
                println!("Creating workspace: {}", name);
            }
            Some(WorkspaceAction::Delete { id }) => {
                println!("Deleting workspace: {}", id);
            }
            None => println!("Workspace subcommand required"),
        },
        Commands::Analytics => {
            println!("Analytics dashboard — coming soon");
        }
    }

    Ok(())
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
