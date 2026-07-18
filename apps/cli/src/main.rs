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
    /// Initialize a project with a stack recipe
    Init {
        #[arg(short, long)]
        recipe: Option<String>,
    },
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
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match &cli.command {
        Commands::Audit { mode } => {
            let m = mode.as_deref().unwrap_or("evaluate_existing");
            println!("AUDIT phase — mode: {}", m);
            println!("Coming soon: full AUDIT implementation");
        }
        Commands::Plan => {
            println!("PLAN phase — coming soon");
        }
        Commands::Execute => {
            println!("EXECUTE phase — coming soon");
        }
        Commands::Init { recipe } => {
            let r = recipe.as_deref().unwrap_or("business-saas");
            println!("Initializing project with recipe: {}", r);
            println!("Coming soon: recipe wizard");
        }
        Commands::Verify { gate } => {
            let g = gate.as_deref().unwrap_or("G0");
            println!("Running verify gate: {}", g);
            println!("Coming soon: G0-G5 verify runner");
        }
        Commands::Workspace { action } => {
            match action {
                Some(WorkspaceAction::List) => println!("Workspaces: (none)"),
                Some(WorkspaceAction::Create { name }) => {
                    println!("Creating workspace: {}", name);
                }
                Some(WorkspaceAction::Delete { id }) => {
                    println!("Deleting workspace: {}", id);
                }
                None => println!("Workspace subcommand required"),
            }
        }
        Commands::Analytics => {
            println!("Analytics dashboard — coming soon");
        }
    }

    Ok(())
}
