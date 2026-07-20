use crate::error::AdeError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const RECIPE_SCHEMA: &str = "ade.recipe/v1";

/// Browse facet: classic well-known vs modern defaults vs frontier stacks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecipeEra {
    Classic,
    #[default]
    Modern,
    Frontier,
}

/// Hints used by Stack Fit scoring (see `recipe_fit`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RecipeFitHints {
    #[serde(default)]
    pub intents: Vec<String>,
    #[serde(default)]
    pub runtimes: Vec<String>,
    #[serde(default)]
    pub ui_surfaces: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub compliance: Vec<String>,
    #[serde(default)]
    pub hosts: Vec<String>,
}

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
    #[serde(default)]
    pub era: RecipeEra,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub fit: RecipeFitHints,
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

fn fit(
    intents: &[&str],
    runtimes: &[&str],
    ui_surfaces: &[&str],
    evidence: &[&str],
    compliance: &[&str],
    hosts: &[&str],
) -> RecipeFitHints {
    RecipeFitHints {
        intents: intents.iter().map(|s| (*s).into()).collect(),
        runtimes: runtimes.iter().map(|s| (*s).into()).collect(),
        ui_surfaces: ui_surfaces.iter().map(|s| (*s).into()).collect(),
        evidence: evidence.iter().map(|s| (*s).into()).collect(),
        compliance: compliance.iter().map(|s| (*s).into()).collect(),
        hosts: hosts.iter().map(|s| (*s).into()).collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn recipe(
    id: &str,
    name: &str,
    description: &str,
    runtimes: &[&str],
    toolchain: &[(&str, &str)],
    commands: RecipeCommands,
    g5: RecipeG5,
    era: RecipeEra,
    domain: &str,
    tags: &[&str],
    fit_hints: RecipeFitHints,
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
        era,
        domain: domain.into(),
        tags: tags.iter().map(|t| (*t).into()).collect(),
        fit: fit_hints,
    }
}

/// Built-in recipe catalog — the 13 ADE stack recipes (+ compatibility alias).
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
            RecipeEra::Modern,
            "saas",
            &["saas", "multi-tenant", "playwright", "rust", "node"],
            fit(
                &["product"],
                &["rust", "node", "mixed"],
                &["web"],
                &["playwright"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Classic,
            "saas",
            &["saas", "compliance", "regulated", "authz", "playwright"],
            fit(
                &["product"],
                &["rust", "node", "mixed"],
                &["web"],
                &["playwright"],
                &["regulated"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Classic,
            "systems",
            &["rust", "cli", "library", "binary"],
            fit(
                &["lib", "product"],
                &["rust"],
                &["none"],
                &["binary"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Modern,
            "systems",
            &["rust", "axum", "turso", "api", "http"],
            fit(
                &["product", "lib"],
                &["rust"],
                &["none", "web"],
                &["http"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Frontier,
            "game",
            &["godot", "game", "rust", "gdscript"],
            fit(
                &["product"],
                &["rust"],
                &["game"],
                &["any"],
                &["none"],
                &["windows", "macos", "linux"],
            ),
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
            RecipeEra::Modern,
            "data-ai",
            &["python", "ml", "data", "ruff", "pytest"],
            fit(
                &["lib", "product"],
                &["python"],
                &["none"],
                &["any"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Modern,
            "mobile",
            &["mobile", "react-native", "flutter", "device"],
            fit(
                &["product"],
                &["node", "any"],
                &["mobile"],
                &["device"],
                &["none"],
                &["macos", "linux", "windows"],
            ),
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
            RecipeEra::Frontier,
            "desktop",
            &["tauri", "desktop", "rust", "node"],
            fit(
                &["product"],
                &["rust", "node", "mixed"],
                &["desktop"],
                &["binary"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Classic,
            "saas",
            &["web", "node", "playwright", "typescript"],
            fit(
                &["product"],
                &["node"],
                &["web"],
                &["playwright"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Classic,
            "embedded",
            &["embedded", "firmware", "hil", "rust"],
            fit(
                &["product", "lib"],
                &["rust"],
                &["none"],
                &["hil"],
                &["none"],
                &["linux", "macos", "windows"],
            ),
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
            RecipeEra::Classic,
            "oss",
            &["oss", "fork", "upstream", "maintainer"],
            fit(
                &["ops", "lib"],
                &["any", "mixed", "rust", "node"],
                &["none", "web"],
                &["any"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Frontier,
            "ade",
            &["plan", "architecture", "ade"],
            fit(
                &["ops"],
                &["any"],
                &["none"],
                &["plan"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
            RecipeEra::Frontier,
            "ade",
            &["multi-agent", "parity", "team", "ade"],
            fit(
                &["ops", "product"],
                &["rust", "any"],
                &["none", "desktop"],
                &["any"],
                &["none"],
                &["windows", "wsl"],
            ),
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
            RecipeEra::Classic,
            "saas",
            &["web", "node", "alias"],
            fit(
                &["product"],
                &["node"],
                &["web"],
                &["playwright"],
                &["none"],
                &["windows", "wsl", "macos", "linux"],
            ),
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
        assert_eq!(recipe.domain, "systems");
        assert_eq!(recipe.era, RecipeEra::Modern);
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

    #[test]
    fn recipes_have_fit_metadata() {
        for recipe in builtin_recipes() {
            if recipe.id == "node-web" {
                continue;
            }
            assert!(!recipe.domain.is_empty(), "{}", recipe.id);
            assert!(!recipe.fit.runtimes.is_empty(), "{}", recipe.id);
        }
    }
}
