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
    /// Run EXECUTE phase — apply approved plan phases
    Execute,
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
            println!("Wrote {}", out.display());
        }
        Commands::Execute => {
            println!("EXECUTE phase — coming soon");
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
        Commands::Verify { gate } => {
            let g = gate.as_deref().unwrap_or("G0");
            println!("Running verify gate: {}", g);
            println!("Coming soon: G0-G5 verify runner");
        }
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
