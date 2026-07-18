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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeCommands {
    pub build: Option<String>,
    pub lint: Option<String>,
    pub format: Option<String>,
    pub test: Option<String>,
}

/// Built-in recipe catalog used by Phase 1 scaffolding.
pub fn builtin_recipes() -> Vec<StackRecipe> {
    vec![
        StackRecipe {
            id: "rust-api-turso".into(),
            name: "Rust API + Turso".into(),
            description: "Axum + Turso/libSQL service with Cargo verify ladder".into(),
            runtimes: vec!["Local (Windows) or WSL2".into()],
            toolchain: BTreeMap::from([("Rust".into(), "stable (rust-toolchain.toml)".into())]),
            commands: RecipeCommands {
                build: Some("cargo build".into()),
                lint: Some("cargo clippy -- -D warnings".into()),
                format: Some("cargo fmt --check".into()),
                test: Some("cargo test".into()),
            },
        },
        StackRecipe {
            id: "business-saas".into(),
            name: "Business SaaS".into(),
            description: "Full-stack business SaaS (API + web) starter profile".into(),
            runtimes: vec!["Local (Windows) or WSL2".into()],
            toolchain: BTreeMap::from([
                ("Rust".into(), "stable (rust-toolchain.toml)".into()),
                ("Node".into(), "v22.x".into()),
            ]),
            commands: RecipeCommands {
                build: Some("cargo build && npm run build".into()),
                lint: Some("cargo clippy -- -D warnings".into()),
                format: Some("cargo fmt --check".into()),
                test: Some("cargo test".into()),
            },
        },
        StackRecipe {
            id: "node-web".into(),
            name: "Node Web App".into(),
            description: "TypeScript/Vite web app with npm verify ladder".into(),
            runtimes: vec!["Local Node".into()],
            toolchain: BTreeMap::from([("Node".into(), "v22.x".into())]),
            commands: RecipeCommands {
                build: Some("npm run build".into()),
                lint: Some("npm run lint".into()),
                format: Some("npm run format --if-present".into()),
                test: Some("npm test".into()),
            },
        },
    ]
}

pub fn builtin_recipe(id: &str) -> Result<StackRecipe, AdeError> {
    let key = id.trim().to_ascii_lowercase();
    builtin_recipes()
        .into_iter()
        .find(|r| r.id == key)
        .ok_or_else(|| {
            let known = builtin_recipes()
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            AdeError::NotFound(format!("unknown recipe '{id}' (known: {known})"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rust_recipe() {
        let r = builtin_recipe("rust-api-turso").unwrap();
        assert_eq!(r.commands.test.as_deref(), Some("cargo test"));
    }

    #[test]
    fn rejects_unknown() {
        assert!(builtin_recipe("nope").is_err());
    }
}
