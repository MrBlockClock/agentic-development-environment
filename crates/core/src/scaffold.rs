use crate::agents_contract::{AgentsContractContext, AgentsContractGenerator};
use crate::error::AdeError;
use crate::ignore::merge_ignore_content;
use crate::recipe::StackRecipe;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCAFFOLD_JOURNAL_SCHEMA: &str = "ade.scaffold.journal/v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScaffoldAction {
    Create,
    Update,
    Preserve,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScaffoldFilePlan {
    pub relative: String,
    pub action: ScaffoldAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScaffoldFileChange {
    pub relative: String,
    pub action: ScaffoldAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScaffoldResult {
    pub recipe_id: String,
    pub project_name: String,
    pub agents_path: String,
    pub recovered_interrupted: bool,
    pub files: Vec<ScaffoldFileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum JournalStatus {
    Preparing,
    Applying,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupEntry {
    relative: String,
    existed: bool,
    backup_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlannedWrite {
    relative: String,
    content: String,
    action: ScaffoldAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScaffoldJournal {
    schema: String,
    id: String,
    status: JournalStatus,
    recipe_id: String,
    backups: Vec<BackupEntry>,
    planned: Vec<PlannedWrite>,
}

/// Transactional recipe bootstrap with rollback journal under `.ade/scaffold`.
pub struct RecipeScaffold;

impl RecipeScaffold {
    /// Recover any interrupted scaffold transaction before planning/applying.
    pub fn recover_interrupted(root: impl AsRef<Path>) -> Result<bool, AdeError> {
        let root = root.as_ref();
        let Some(journal_path) = journal_path_if_present(root)? else {
            return Ok(false);
        };
        let payload = std::fs::read_to_string(&journal_path)?;
        let journal: ScaffoldJournal = serde_json::from_str(&payload)?;
        if journal.schema != SCAFFOLD_JOURNAL_SCHEMA {
            return Err(AdeError::Other(
                "unsupported scaffold journal schema".into(),
            ));
        }
        match journal.status {
            JournalStatus::Completed => {
                cleanup_transaction(root, &journal.id)?;
                Ok(false)
            }
            JournalStatus::Preparing | JournalStatus::Applying => {
                rollback_journal(root, &journal)?;
                cleanup_transaction(root, &journal.id)?;
                Ok(true)
            }
        }
    }

    /// Dry-run the file set that would be created/updated/preserved.
    pub fn plan(
        root: impl AsRef<Path>,
        recipe: &StackRecipe,
        ctx: &AgentsContractContext,
        force: bool,
    ) -> Result<Vec<ScaffoldFilePlan>, AdeError> {
        let root = root.as_ref();
        Self::recover_interrupted(root)?;
        let planned = build_plan(root, recipe, ctx, force)?;
        Ok(planned
            .into_iter()
            .map(|item| ScaffoldFilePlan {
                relative: item.relative,
                action: item.action,
            })
            .collect())
    }

    /// Apply the scaffold transactionally. Restores prior state on failure.
    pub fn apply(
        root: impl AsRef<Path>,
        recipe: &StackRecipe,
        ctx: &AgentsContractContext,
        force: bool,
    ) -> Result<ScaffoldResult, AdeError> {
        Self::apply_inner(root.as_ref(), recipe, ctx, force, None)
    }

    /// Test helper: optionally fault after N successful file writes.
    #[cfg(test)]
    pub fn apply_with_fault_after(
        root: impl AsRef<Path>,
        recipe: &StackRecipe,
        ctx: &AgentsContractContext,
        force: bool,
        fault_after: usize,
    ) -> Result<ScaffoldResult, AdeError> {
        Self::apply_inner(root.as_ref(), recipe, ctx, force, Some(fault_after))
    }

    fn apply_inner(
        root: &Path,
        recipe: &StackRecipe,
        ctx: &AgentsContractContext,
        force: bool,
        fault_after: Option<usize>,
    ) -> Result<ScaffoldResult, AdeError> {
        let recovered_interrupted = Self::recover_interrupted(root)?;
        let plan = build_plan(root, recipe, ctx, force)?;
        let writes = plan
            .iter()
            .filter_map(|item| {
                item.content.as_ref().map(|content| PlannedWrite {
                    relative: item.relative.clone(),
                    content: content.clone(),
                    action: item.action,
                })
            })
            .collect::<Vec<_>>();
        let result_files = plan
            .into_iter()
            .map(|item| ScaffoldFileChange {
                relative: item.relative,
                action: item.action,
            })
            .collect::<Vec<_>>();

        let tx_id = uuid::Uuid::new_v4().to_string();
        let tx_dir = scaffold_dir(root).join(&tx_id);
        let backup_dir = tx_dir.join("backups");
        std::fs::create_dir_all(&backup_dir)?;

        let mut backups = Vec::new();
        for write in &writes {
            let path = root.join(&write.relative);
            if path.exists() {
                let backup_name = write.relative.replace(['/', '\\'], "__");
                std::fs::copy(&path, backup_dir.join(&backup_name))?;
                backups.push(BackupEntry {
                    relative: write.relative.clone(),
                    existed: true,
                    backup_name: Some(backup_name),
                });
            } else {
                backups.push(BackupEntry {
                    relative: write.relative.clone(),
                    existed: false,
                    backup_name: None,
                });
            }
        }

        let mut journal = ScaffoldJournal {
            schema: SCAFFOLD_JOURNAL_SCHEMA.into(),
            id: tx_id.clone(),
            status: JournalStatus::Preparing,
            recipe_id: recipe.id.clone(),
            backups,
            planned: writes.clone(),
        };
        write_journal(root, &journal)?;

        journal.status = JournalStatus::Applying;
        write_journal(root, &journal)?;

        let apply_result = (|| -> Result<(), AdeError> {
            for (index, write) in writes.iter().enumerate() {
                if let Some(limit) = fault_after {
                    if index >= limit {
                        return Err(AdeError::Other(
                            "injected scaffold fault for recovery testing".into(),
                        ));
                    }
                }
                write_atomic(root, &write.relative, write.content.as_bytes())?;
            }
            Ok(())
        })();

        if let Err(error) = apply_result {
            let _ = rollback_journal(root, &journal);
            let _ = cleanup_transaction(root, &tx_id);
            return Err(error);
        }

        journal.status = JournalStatus::Completed;
        write_journal(root, &journal)?;
        cleanup_transaction(root, &tx_id)?;

        Ok(ScaffoldResult {
            recipe_id: recipe.id.clone(),
            project_name: ctx.project_name.clone(),
            agents_path: root.join("AGENTS.md").display().to_string(),
            recovered_interrupted,
            files: result_files,
        })
    }
}

#[derive(Debug, Clone)]
struct PlanItem {
    relative: String,
    action: ScaffoldAction,
    content: Option<String>,
}

fn build_plan(
    root: &Path,
    recipe: &StackRecipe,
    ctx: &AgentsContractContext,
    force: bool,
) -> Result<Vec<PlanItem>, AdeError> {
    let agents_path = root.join("AGENTS.md");
    if agents_path.exists() && !force {
        return Err(AdeError::Other(format!(
            "AGENTS.md already exists at {} (pass --force to overwrite)",
            agents_path.display()
        )));
    }

    let mut items = Vec::new();
    let agents_content = AgentsContractGenerator::render(recipe, ctx);
    items.push(PlanItem {
        relative: "AGENTS.md".into(),
        action: if agents_path.exists() {
            ScaffoldAction::Update
        } else {
            ScaffoldAction::Create
        },
        content: Some(agents_content),
    });

    for relative in [".gitignore", ".cursorignore"] {
        let path = root.join(relative);
        let existing = if path.exists() {
            std::fs::read_to_string(&path)?
        } else {
            String::new()
        };
        let (body, changed) = merge_ignore_content(&existing);
        if !path.exists() {
            items.push(PlanItem {
                relative: relative.into(),
                action: ScaffoldAction::Create,
                content: Some(body),
            });
        } else if changed {
            items.push(PlanItem {
                relative: relative.into(),
                action: ScaffoldAction::Update,
                content: Some(body),
            });
        } else {
            items.push(PlanItem {
                relative: relative.into(),
                action: ScaffoldAction::Preserve,
                content: None,
            });
        }
    }

    if recipe.toolchain.contains_key("Rust") {
        let pin = root.join("rust-toolchain.toml");
        if pin.exists() {
            items.push(PlanItem {
                relative: "rust-toolchain.toml".into(),
                action: ScaffoldAction::Preserve,
                content: None,
            });
        } else {
            items.push(PlanItem {
                relative: "rust-toolchain.toml".into(),
                action: ScaffoldAction::Create,
                content: Some(
                    "[toolchain]\nchannel = \"stable\"\ncomponents = [\"rustfmt\", \"clippy\"]\n"
                        .into(),
                ),
            });
        }
    }

    if recipe.toolchain.contains_key("Node") {
        let nvmrc = root.join(".nvmrc");
        if nvmrc.exists() {
            items.push(PlanItem {
                relative: ".nvmrc".into(),
                action: ScaffoldAction::Preserve,
                content: None,
            });
        } else {
            items.push(PlanItem {
                relative: ".nvmrc".into(),
                action: ScaffoldAction::Create,
                content: Some("22\n".into()),
            });
        }
    }

    items.extend(recipe_g5_scaffold_items(root, recipe));

    Ok(items)
}

fn recipe_g5_scaffold_items(root: &Path, recipe: &StackRecipe) -> Vec<PlanItem> {
    use crate::recipe::RecipeG5;

    let mut items = Vec::new();
    let recipe_json = serde_json::json!({
        "schema": "ade.recipe.meta/v1",
        "id": recipe.id,
        "g5": recipe.g5,
    });
    let recipe_meta = root.join(".ade").join("recipe.json");
    items.push(PlanItem {
        relative: ".ade/recipe.json".into(),
        action: if recipe_meta.exists() {
            ScaffoldAction::Update
        } else {
            ScaffoldAction::Create
        },
        content: Some(format!(
            "{}\n",
            serde_json::to_string_pretty(&recipe_json).unwrap_or_else(|_| "{}".into())
        )),
    });

    match recipe.g5 {
        RecipeG5::Playwright => {
            push_script_pair(
                &mut items,
                root,
                "# G5 Playwright evidence\nnpx playwright test --reporter=line\n",
                "npx playwright test --reporter=line\n",
            );
        }
        RecipeG5::BinarySmoke
        | RecipeG5::HttpContract
        | RecipeG5::UpstreamTests
        | RecipeG5::InstallSmoke
        | RecipeG5::ParityProbes => {
            push_script_pair(
                &mut items,
                root,
                "# G5 cargo evidence\ncargo test --workspace -- --nocapture\n",
                "cargo test --workspace -- --nocapture\n",
            );
        }
        RecipeG5::PlaytestChecklist
        | RecipeG5::ReproducibilityNote
        | RecipeG5::DeviceChecklist
        | RecipeG5::HardwareSignoff
        | RecipeG5::PlanChecklist => {
            let checklist = root.join("scripts").join("g5-checklist.md");
            let body = format!(
                "# G5 checklist ({})\n\n- [ ] Human reviewed acceptance criteria\n- [ ] Evidence attached or signed off\n- [ ] Replace this checklist with scripts/g5-evidence.* when automation exists\n",
                recipe.id
            );
            items.push(PlanItem {
                relative: "scripts/g5-checklist.md".into(),
                action: if checklist.exists() {
                    ScaffoldAction::Preserve
                } else {
                    ScaffoldAction::Create
                },
                content: if checklist.exists() { None } else { Some(body) },
            });
        }
        RecipeG5::None => {}
    }

    items
}

fn push_script_pair(items: &mut Vec<PlanItem>, root: &Path, ps1: &str, sh: &str) {
    for (relative, body) in [
        ("scripts/g5-evidence.ps1", ps1),
        ("scripts/g5-evidence.sh", sh),
    ] {
        let path = root.join(relative);
        if path.exists() {
            items.push(PlanItem {
                relative: relative.into(),
                action: ScaffoldAction::Preserve,
                content: None,
            });
        } else {
            items.push(PlanItem {
                relative: relative.into(),
                action: ScaffoldAction::Create,
                content: Some(body.into()),
            });
        }
    }
}

#[cfg(test)]
fn build_writes(
    root: &Path,
    recipe: &StackRecipe,
    ctx: &AgentsContractContext,
    force: bool,
) -> Result<Vec<PlannedWrite>, AdeError> {
    Ok(build_plan(root, recipe, ctx, force)?
        .into_iter()
        .filter_map(|item| {
            item.content.map(|content| PlannedWrite {
                relative: item.relative,
                content,
                action: item.action,
            })
        })
        .collect())
}

fn scaffold_dir(root: &Path) -> PathBuf {
    root.join(".ade").join("scaffold")
}

fn journal_path(root: &Path) -> PathBuf {
    scaffold_dir(root).join("journal.json")
}

fn journal_path_if_present(root: &Path) -> Result<Option<PathBuf>, AdeError> {
    let path = journal_path(root);
    if path.is_file() {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

fn write_journal(root: &Path, journal: &ScaffoldJournal) -> Result<(), AdeError> {
    let directory = scaffold_dir(root);
    std::fs::create_dir_all(&directory)?;
    let payload = serde_json::to_vec_pretty(journal)?;
    write_bytes_atomic(&journal_path(root), &payload)
}

fn write_atomic(root: &Path, relative: &str, payload: &[u8]) -> Result<(), AdeError> {
    ensure_safe_relative(relative)?;
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_bytes_atomic(&path, payload)
}

fn write_bytes_atomic(path: &Path, payload: &[u8]) -> Result<(), AdeError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ade-write");
    let temporary = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{file_name}.ade.tmp"));
    std::fs::write(&temporary, payload)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn rollback_journal(root: &Path, journal: &ScaffoldJournal) -> Result<(), AdeError> {
    let backup_dir = scaffold_dir(root).join(&journal.id).join("backups");
    for entry in &journal.backups {
        ensure_safe_relative(&entry.relative)?;
        let target = root.join(&entry.relative);
        if entry.existed {
            let Some(backup_name) = &entry.backup_name else {
                continue;
            };
            let backup = backup_dir.join(backup_name);
            if backup.is_file() {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&backup, &target)?;
            }
        } else if target.exists() {
            std::fs::remove_file(&target)?;
        }
    }
    Ok(())
}

fn cleanup_transaction(root: &Path, tx_id: &str) -> Result<(), AdeError> {
    let tx_dir = scaffold_dir(root).join(tx_id);
    if tx_dir.exists() {
        let _ = std::fs::remove_dir_all(&tx_dir);
    }
    let journal = journal_path(root);
    if journal.exists() {
        std::fs::remove_file(journal)?;
    }
    Ok(())
}

fn ensure_safe_relative(relative: &str) -> Result<(), AdeError> {
    let normalized = relative.replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains("..")
        || Path::new(&normalized).is_absolute()
    {
        return Err(AdeError::Authorization(format!(
            "refusing unsafe scaffold path '{relative}'"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::builtin_recipe;
    use std::fs;

    fn fixture() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ade-scaffold-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn applies_agents_ignores_and_toolchain_pins() {
        let root = fixture();
        let recipe = builtin_recipe("business-saas").unwrap();
        let ctx = AgentsContractContext::new("demo").with_root(root.display().to_string());
        let result = RecipeScaffold::apply(&root, &recipe, &ctx, false).unwrap();
        assert!(root.join("AGENTS.md").is_file());
        assert!(root.join(".gitignore").is_file());
        assert!(root.join(".cursorignore").is_file());
        assert!(root.join("rust-toolchain.toml").is_file());
        assert!(root.join(".nvmrc").is_file());
        assert!(root.join(".ade").join("recipe.json").is_file());
        assert!(root.join("scripts").join("g5-evidence.ps1").is_file());
        assert!(!journal_path(&root).exists());
        assert!(result
            .files
            .iter()
            .any(|file| file.relative == "AGENTS.md" && file.action == ScaffoldAction::Create));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_overwrite_without_force_and_preserves_existing_pins() {
        let root = fixture();
        fs::write(root.join("AGENTS.md"), "# keep\n").unwrap();
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"nightly\"\n",
        )
        .unwrap();
        let recipe = builtin_recipe("rust-api-turso").unwrap();
        let ctx = AgentsContractContext::new("demo");
        assert!(RecipeScaffold::apply(&root, &recipe, &ctx, false).is_err());

        fs::remove_file(root.join("AGENTS.md")).unwrap();
        let plan = RecipeScaffold::plan(&root, &recipe, &ctx, false).unwrap();
        assert!(plan.iter().any(|item| {
            item.relative == "rust-toolchain.toml" && item.action == ScaffoldAction::Preserve
        }));
        RecipeScaffold::apply(&root, &recipe, &ctx, false).unwrap();
        let pin = fs::read_to_string(root.join("rust-toolchain.toml")).unwrap();
        assert!(pin.contains("nightly"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fault_injection_rolls_back_partial_writes() {
        let root = fixture();
        fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();
        let before = fs::read_to_string(root.join(".gitignore")).unwrap();
        let recipe = builtin_recipe("rust-systems").unwrap();
        let ctx = AgentsContractContext::new("demo");
        let err =
            RecipeScaffold::apply_with_fault_after(&root, &recipe, &ctx, false, 1).unwrap_err();
        assert!(err.to_string().contains("injected scaffold fault"));
        assert!(!root.join("AGENTS.md").exists());
        assert_eq!(fs::read_to_string(root.join(".gitignore")).unwrap(), before);
        assert!(!journal_path(&root).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovers_interrupted_journal_on_next_run() {
        let root = fixture();
        fs::write(root.join(".gitignore"), "target/\n").unwrap();
        let recipe = builtin_recipe("rust-systems").unwrap();
        let ctx = AgentsContractContext::new("demo");
        let writes = build_writes(&root, &recipe, &ctx, false).unwrap();
        let tx_id = uuid::Uuid::new_v4().to_string();
        let backup_dir = scaffold_dir(&root).join(&tx_id).join("backups");
        fs::create_dir_all(&backup_dir).unwrap();
        fs::copy(root.join(".gitignore"), backup_dir.join(".gitignore")).unwrap();
        // Simulate a partial write that should be rolled back.
        fs::write(root.join("AGENTS.md"), "partial\n").unwrap();
        fs::write(root.join(".gitignore"), "corrupted\n").unwrap();
        let journal = ScaffoldJournal {
            schema: SCAFFOLD_JOURNAL_SCHEMA.into(),
            id: tx_id,
            status: JournalStatus::Applying,
            recipe_id: recipe.id.clone(),
            backups: vec![
                BackupEntry {
                    relative: "AGENTS.md".into(),
                    existed: false,
                    backup_name: None,
                },
                BackupEntry {
                    relative: ".gitignore".into(),
                    existed: true,
                    backup_name: Some(".gitignore".into()),
                },
            ],
            planned: writes,
        };
        write_journal(&root, &journal).unwrap();

        assert!(RecipeScaffold::recover_interrupted(&root).unwrap());
        assert!(!root.join("AGENTS.md").exists());
        assert_eq!(
            fs::read_to_string(root.join(".gitignore")).unwrap(),
            "target/\n"
        );
        assert!(!journal_path(&root).exists());
        let _ = fs::remove_dir_all(root);
    }
}
