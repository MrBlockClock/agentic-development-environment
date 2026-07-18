use crate::error::AdeError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RECIPE_SCHEMA: &str = "ade.recipe/v1";

/// A stack recipe used by `ade init` to scaffold a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackRecipe {
    pub id: String,
    pub name: String,
    pub description: String,
    pub runtimes: Vec<String>,
    /// Display pins shown in AGENTS.md (e.g. Rust → stable).
    #[serde(default)]
    pub toolchain: BTreeMap<String, String>,
    pub commands: RecipeCommands,
    /// How G5 evidence should be gathered for this stack.
    #[serde(default)]
    pub g5: RecipeG5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeCommands {
    pub build: Option<RecipeCommand>,
    pub lint: Option<RecipeCommand>,
    pub format: Option<RecipeCommand>,
    pub test: Option<RecipeCommand>,
}

/// A verification action is either one shell-free command or an ordered list.
/// Ordered steps avoid embedding shell-specific `&&` / `||` operators.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RecipeCommand {
    Single(String),
    Steps(Vec<String>),
}

impl RecipeCommand {
    pub fn steps(&self) -> Vec<&str> {
        match self {
            Self::Single(command) => vec![command],
            Self::Steps(commands) => commands.iter().map(String::as_str).collect(),
        }
    }
}

fn cmd(command: &str) -> Option<RecipeCommand> {
    Some(RecipeCommand::Single(command.into()))
}

fn steps(commands: &[&str]) -> Option<RecipeCommand> {
    Some(RecipeCommand::Steps(
        commands.iter().map(|command| (*command).into()).collect(),
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecipeG5 {
    Playwright,
    HttpContract,
    BinarySmoke,
    PlaytestChecklist,
    ReproducibilityNote,
    DeviceChecklist,
    InstallSmoke,
    HardwareSignoff,
    UpstreamTests,
    PlanChecklist,
    ParityProbes,
    #[default]
    None,
}

fn recipe(
    id: &str,
    name: &str,
    description: &str,
    runtimes: &[&str],
    toolchain: &[(&str, &str)],
    commands: RecipeCommands,
    g5: RecipeG5,
) -> StackRecipe {
    StackRecipe {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        runtimes: runtimes.iter().map(|item| (*item).into()).collect(),
        toolchain: toolchain
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect(),
        commands,
        g5,
    }
}

/// Built-in recipe catalog — the 13 ADE stack recipes.
pub fn builtin_recipes() -> Vec<StackRecipe> {
    vec![
        recipe(
            "business-saas",
            "Business SaaS",
            "Multi-tenant SaaS (API + web) with Playwright login evidence",
            &["Local (Windows) or WSL2"],
            &[("Rust", "stable (rust-toolchain.toml)"), ("Node", "v22.x")],
            RecipeCommands {
                build: steps(&["cargo build", "npm run build"]),
                lint: cmd("cargo clippy -- -D warnings"),
                format: cmd("cargo fmt --check"),
                test: steps(&["cargo test", "npm test"]),
            },
            RecipeG5::Playwright,
        ),
        recipe(
            "business-regulated",
            "Business Regulated",
            "Compliance-heavy SaaS with authz proof and Playwright evidence",
            &["Local (Windows) or WSL2"],
            &[("Rust", "stable (rust-toolchain.toml)"), ("Node", "v22.x")],
            RecipeCommands {
                build: steps(&["cargo build", "npm run build"]),
                lint: cmd("cargo clippy -- -D warnings"),
                format: cmd("cargo fmt --check"),
                test: steps(&["cargo test", "npm test"]),
            },
            RecipeG5::Playwright,
        ),
        recipe(
            "rust-systems",
            "Rust Systems",
            "CLIs, libraries, and services with binary smoke evidence",
            &["Local Rust"],
            &[("Rust", "stable (rust-toolchain.toml)")],
            RecipeCommands {
                build: cmd("cargo build --release"),
                lint: cmd("cargo clippy -- -D warnings"),
                format: cmd("cargo fmt --check"),
                test: cmd("cargo test"),
            },
            RecipeG5::BinarySmoke,
        ),
        recipe(
            "rust-api-turso",
            "Rust API + Turso",
            "Axum + Turso/libSQL service with HTTP contract evidence",
            &["Local (Windows) or WSL2"],
            &[("Rust", "stable (rust-toolchain.toml)")],
            RecipeCommands {
                build: cmd("cargo build"),
                lint: cmd("cargo clippy -- -D warnings"),
                format: cmd("cargo fmt --check"),
                test: cmd("cargo test"),
            },
            RecipeG5::HttpContract,
        ),
        recipe(
            "godot-rust-game",
            "Godot + Rust Game",
            "Game development with Rust + GDScript and playtest checklist",
            &["Local Godot + Rust"],
            &[("Rust", "stable (rust-toolchain.toml)"), ("Godot", "4.x")],
            RecipeCommands {
                build: cmd("cargo build"),
                lint: cmd("cargo clippy -- -D warnings"),
                format: cmd("cargo fmt --check"),
                test: cmd("cargo test"),
            },
            RecipeG5::PlaytestChecklist,
        ),
        recipe(
            "python-data-ai",
            "Python Data / AI",
            "Data and ML workflows with reproducibility notes",
            &["Local Python"],
            &[("Python", "3.12+")],
            RecipeCommands {
                build: cmd("python -m pip install -e ."),
                lint: cmd("ruff check ."),
                format: cmd("ruff format --check ."),
                test: cmd("pytest"),
            },
            RecipeG5::ReproducibilityNote,
        ),
        recipe(
            "mobile-app",
            "Mobile App",
            "iOS/Android via RN/Flutter/KMP with device checklist evidence",
            &["Local mobile toolchain"],
            &[("Node", "v22.x")],
            RecipeCommands {
                build: cmd("npm run build"),
                lint: cmd("npm run lint"),
                format: cmd("npm run format --if-present"),
                test: cmd("npm test"),
            },
            RecipeG5::DeviceChecklist,
        ),
        recipe(
            "tauri-desktop",
            "Tauri Desktop",
            "Rust + web desktop app with install smoke evidence",
            &["Local (Windows) or WSL2"],
            &[("Rust", "stable (rust-toolchain.toml)"), ("Node", "v22.x")],
            RecipeCommands {
                build: steps(&["npm run build", "cargo build -p ade-desktop-app"]),
                lint: cmd("cargo clippy -- -D warnings"),
                format: cmd("cargo fmt --check"),
                test: cmd("cargo test"),
            },
            RecipeG5::InstallSmoke,
        ),
        recipe(
            "web-playwright-quality",
            "Web Playwright Quality",
            "Web app where Playwright G5 evidence is mandatory",
            &["Local Node"],
            &[("Node", "v22.x")],
            RecipeCommands {
                build: cmd("npm run build"),
                lint: cmd("npm run lint"),
                format: cmd("npm run format --if-present"),
                test: cmd("npm test"),
            },
            RecipeG5::Playwright,
        ),
        recipe(
            "embedded-hil",
            "Embedded HIL",
            "Firmware with human hardware-in-the-loop sign-off",
            &["Local embedded toolchain"],
            &[("Rust", "stable (rust-toolchain.toml)")],
            RecipeCommands {
                build: cmd("cargo build"),
                lint: cmd("cargo clippy -- -D warnings"),
                format: cmd("cargo fmt --check"),
                test: cmd("cargo test"),
            },
            RecipeG5::HardwareSignoff,
        ),
        recipe(
            "oss-fork-maintainer",
            "OSS Fork Maintainer",
            "Upstream fork maintenance with upstream test evidence",
            &["Local"],
            &[],
            RecipeCommands {
                build: steps(&["cargo build", "npm run build"]),
                lint: steps(&["cargo clippy -- -D warnings", "npm run lint"]),
                format: steps(&["cargo fmt --check", "npm run format --if-present"]),
                test: steps(&["cargo test", "npm test"]),
            },
            RecipeG5::UpstreamTests,
        ),
        recipe(
            "ade-plan-heavy",
            "ADE Plan Heavy",
            "Architecture-first work with plan quality checklist evidence",
            &["Local"],
            &[],
            RecipeCommands {
                build: None,
                lint: None,
                format: None,
                test: None,
            },
            RecipeG5::PlanChecklist,
        ),
        recipe(
            "multi-ade-shop",
            "Multi-ADE Shop",
            "Multi-agent team workflows with parity probes",
            &["Local (Windows) or WSL2"],
            &[("Rust", "stable (rust-toolchain.toml)")],
            RecipeCommands {
                build: cmd("cargo build"),
                lint: cmd("cargo clippy -- -D warnings"),
                format: cmd("cargo fmt --check"),
                test: cmd("cargo test"),
            },
            RecipeG5::ParityProbes,
        ),
        // Compatibility alias used by older docs/UI copy.
        recipe(
            "node-web",
            "Node Web App",
            "TypeScript/Vite web app with npm verify ladder (alias of web-playwright-quality)",
            &["Local Node"],
            &[("Node", "v22.x")],
            RecipeCommands {
                build: cmd("npm run build"),
                lint: cmd("npm run lint"),
                format: cmd("npm run format --if-present"),
                test: cmd("npm test"),
            },
            RecipeG5::Playwright,
        ),
    ]
}

pub fn builtin_recipe(id: &str) -> Result<StackRecipe, AdeError> {
    let key = id.trim().to_ascii_lowercase();
    let alias = match key.as_str() {
        "node-web" => "web-playwright-quality",
        other => other,
    };
    builtin_recipes()
        .into_iter()
        .find(|recipe| recipe.id == key || recipe.id == alias)
        .ok_or_else(|| {
            let known = builtin_recipes()
                .iter()
                .map(|recipe| recipe.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            AdeError::NotFound(format!("unknown recipe '{id}' (known: {known})"))
        })
}

/// Canonical 13 recipe ids (excludes compatibility aliases).
pub fn canonical_recipe_ids() -> Vec<&'static str> {
    vec![
        "business-saas",
        "business-regulated",
        "rust-systems",
        "rust-api-turso",
        "godot-rust-game",
        "python-data-ai",
        "mobile-app",
        "tauri-desktop",
        "web-playwright-quality",
        "embedded-hil",
        "oss-fork-maintainer",
        "ade-plan-heavy",
        "multi-ade-shop",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rust_recipe() {
        let recipe = builtin_recipe("rust-api-turso").unwrap();
        assert_eq!(
            recipe.commands.test.as_ref().unwrap().steps(),
            vec!["cargo test"]
        );
        assert_eq!(recipe.g5, RecipeG5::HttpContract);
    }

    #[test]
    fn catalog_covers_thirteen_canonical_recipes() {
        assert_eq!(canonical_recipe_ids().len(), 13);
        for id in canonical_recipe_ids() {
            assert!(builtin_recipe(id).is_ok(), "{id}");
        }
    }

    #[test]
    fn rejects_unknown() {
        assert!(builtin_recipe("nope").is_err());
    }

    #[test]
    fn recipe_commands_do_not_embed_shell_specific_chaining() {
        for recipe in builtin_recipes() {
            for command in [
                recipe.commands.build,
                recipe.commands.lint,
                recipe.commands.format,
                recipe.commands.test,
            ]
            .into_iter()
            .flatten()
            {
                for step in command.steps() {
                    assert!(!step.contains("&&"), "{}: {step}", recipe.id);
                    assert!(!step.contains("||"), "{}: {step}", recipe.id);
                }
            }
        }
    }
}
