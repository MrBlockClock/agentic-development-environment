//! Discover and select Global + workspace skills for agent prompt injection.
//!
//! Progressive disclosure (Ideal I3):
//! - T1: skill catalog (names + short descriptions) always in the system prompt
//! - T2: full bodies for `always_apply`, keyword match, or host `activate_skill`

use ade_core::error::AdeError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A skill loaded from Global guidance or `.ade/skills/<name>/SKILL.md`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub always_apply: bool,
    pub body: String,
    pub source: String,
    /// `global` or `workspace`.
    #[serde(default = "default_workspace_scope")]
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
}

fn default_workspace_scope() -> String {
    "workspace".into()
}

/// Lists and selects skills under Global + workspace `.ade/skills` trees.
#[derive(Debug, Clone, Default)]
pub struct SkillLoader {
    root: PathBuf,
}

impl SkillLoader {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.root.join(".ade").join("skills")
    }

    pub fn load_all(&self) -> Result<Vec<SkillDefinition>, AdeError> {
        let mut by_name: BTreeMap<String, SkillDefinition> = BTreeMap::new();

        for skill in load_skills_from_dir(&ade_core::guidance::global_skills_dir(), None, "global")?
        {
            merge_skill_into(&mut by_name, skill);
        }
        for skill in load_skills_from_dir(&self.skills_dir(), Some(&self.root), "workspace")? {
            merge_skill_into(&mut by_name, skill);
        }

        let active = ade_core::guidance::read_active_profile_id().and_then(|id| {
            ade_core::guidance::load_profiles(&self.root)
                .ok()?
                .into_iter()
                .find(|p| p.id == id)
        });

        let mut skills: Vec<SkillDefinition> = by_name.into_values().collect();
        skills.retain(|skill| {
            ade_core::guidance::pack_allowed(skill.pack.as_deref(), active.as_ref(), false)
        });
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    /// Load one skill by exact name (directory / frontmatter name).
    /// On activate, also append T3 `references/*.md` (capped) under the skill dir.
    pub fn activate(&self, name: &str) -> Result<SkillDefinition, AdeError> {
        let needle = name.trim();
        if needle.is_empty() {
            return Err(AdeError::Config("skill name cannot be empty".into()));
        }
        let skills = self.load_all()?;
        let mut skill = skills
            .into_iter()
            .find(|skill| skill.name.eq_ignore_ascii_case(needle))
            .ok_or_else(|| {
                AdeError::NotFound(format!(
                    "skill '{needle}' not found under Global or .ade/skills"
                ))
            })?;
        if let Some(dir) = resolve_skill_dir(&self.root, &skill) {
            let refs = load_skill_references(&dir)?;
            if !refs.is_empty() {
                skill.body.push_str("\n\n## References (T3)\n\n");
                skill.body.push_str(&refs);
            }
        }
        Ok(skill)
    }

    /// T1 catalog plus T2 bodies for always_apply / keyword matches.
    pub fn prompt_context(&self, user_prompt: &str, max_tokens: u32) -> Result<String, AdeError> {
        let skills = self.load_all()?;
        Ok(select_skills_prompt(&skills, user_prompt, max_tokens))
    }
}

fn merge_skill_into(by_name: &mut BTreeMap<String, SkillDefinition>, incoming: SkillDefinition) {
    let key = incoming.name.to_ascii_lowercase();
    match by_name.get_mut(&key) {
        None => {
            by_name.insert(key, incoming);
        }
        Some(existing) => {
            let always = existing.always_apply || incoming.always_apply;
            if incoming.scope == "workspace" {
                *existing = incoming;
                existing.always_apply = always;
            } else {
                existing.always_apply = always;
            }
        }
    }
}

fn resolve_skill_dir(workspace: &Path, skill: &SkillDefinition) -> Option<PathBuf> {
    if skill.scope == "global" {
        Some(ade_core::guidance::global_skills_dir().join(&skill.name))
    } else {
        Path::new(&skill.source).parent().map(|p| workspace.join(p))
    }
}

fn load_skills_from_dir(
    dir: &Path,
    strip_root: Option<&Path>,
    scope: &str,
) -> Result<Vec<SkillDefinition>, AdeError> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut dirs = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    let mut skills = Vec::new();
    for skill_dir in dirs {
        let skill_md = skill_dir.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let content = std::fs::read_to_string(&skill_md)?;
        if let Some(skill) = parse_skill(strip_root, &skill_md, &content, scope) {
            skills.push(skill);
        }
    }
    Ok(skills)
}

pub fn catalog_prompt(skills: &[SkillDefinition]) -> String {
    let catalog = skills
        .iter()
        .map(|skill| {
            format!(
                "- {} [{}]: {}",
                skill.name,
                skill.scope,
                truncate_chars(&skill.description, 160)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "AVAILABLE SKILLS (Global + .ade/skills) — T1 catalog. Call ade__activate_skill with {{\"name\"}} to load a full body when needed beyond match/always-on:\n{catalog}"
    )
}

pub fn skill_body_block(skill: &SkillDefinition) -> String {
    format!(
        "SKILL {} ({})\n{}\n\n{}",
        skill.name, skill.source, skill.description, skill.body
    )
}

/// T3: load up to 4 markdown files from `references/` (each ≤4k chars).
fn load_skill_references(skill_dir: &Path) -> Result<String, AdeError> {
    let refs_dir = skill_dir.join("references");
    if !refs_dir.is_dir() {
        return Ok(String::new());
    }
    let mut files = std::fs::read_dir(&refs_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .collect::<Vec<_>>();
    files.sort();
    files.truncate(4);
    let mut out = String::new();
    for path in files {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ref.md");
        let raw = std::fs::read_to_string(&path)?;
        let body = truncate_chars(raw.trim(), 4_000);
        out.push_str(&format!("### {name}\n{body}\n\n"));
    }
    Ok(out)
}

fn parse_skill(
    strip_root: Option<&Path>,
    path: &Path,
    content: &str,
    scope: &str,
) -> Option<SkillDefinition> {
    let (header, body) = split_frontmatter(content);
    let name = header_value(header, "name")
        .filter(|value| !value.is_empty())
        .or_else(|| {
            path.parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name.to_string_lossy().into_owned())
        })?;
    let description = header_value(header, "description").unwrap_or_default();
    let always_apply = header.lines().any(|line| {
        let key = line
            .split_once(':')
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase()));
        matches!(
            key.as_ref().map(|(k, v)| (k.as_str(), v.as_str())),
            Some((
                "alwaysapply" | "always_apply" | "ade-always" | "ade_always",
                "true"
            ))
        )
    });
    let source = if let Some(root) = strip_root {
        path.strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
            .replace('\\', "/")
    } else {
        format!("global/skills/{name}/SKILL.md")
    };
    Some(SkillDefinition {
        name,
        description,
        always_apply,
        body: body.trim().to_string(),
        source,
        scope: scope.into(),
        pack: ade_core::guidance::frontmatter_pack(content),
    })
}

fn split_frontmatter(content: &str) -> (&str, &str) {
    let Some(rest) = content.strip_prefix("---") else {
        return ("", content);
    };
    match rest.split_once("\n---") {
        Some((header, body)) => (header, body.trim_start_matches('\n')),
        None => ("", content),
    }
}

fn header_value(header: &str, key: &str) -> Option<String> {
    let mut lines = header.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix(&format!("{key}:"))
            .or_else(|| trimmed.strip_prefix(&format!("{key} :")))
        else {
            continue;
        };
        let rest = rest.trim();
        if rest == ">" || rest == "|-" || rest == "|" || rest == ">-" {
            let mut block = String::new();
            while let Some(next) = lines.peek() {
                let candidate = *next;
                if candidate.starts_with(' ') || candidate.starts_with('\t') {
                    lines.next();
                    if !block.is_empty() {
                        block.push(' ');
                    }
                    block.push_str(candidate.trim());
                } else if candidate.trim().is_empty() {
                    lines.next();
                } else {
                    break;
                }
            }
            return Some(block);
        }
        return Some(rest.trim_matches('"').to_string());
    }
    None
}

fn select_skills_prompt(skills: &[SkillDefinition], user_prompt: &str, max_tokens: u32) -> String {
    if skills.is_empty() || max_tokens == 0 {
        return String::new();
    }

    let prompt_lower = user_prompt.to_ascii_lowercase();
    let mut selected: Vec<&SkillDefinition> = skills
        .iter()
        .filter(|skill| skill.always_apply || skill_matches(skill, &prompt_lower))
        .collect();

    // Prefer always-on first, then matches; drop duplicates.
    selected.sort_by_key(|skill| (!skill.always_apply, skill.name.as_str()));
    selected.dedup_by_key(|skill| skill.name.as_str());

    let catalog = catalog_prompt(skills);
    let catalog_tokens = estimate_tokens(&catalog);
    let mut parts = vec![catalog];
    let mut used = catalog_tokens;

    for skill in selected {
        let block = skill_body_block(skill);
        let block_tokens = estimate_tokens(&block);
        if used.saturating_add(block_tokens) > max_tokens {
            break;
        }
        used = used.saturating_add(block_tokens);
        parts.push(block);
    }

    let mut text = parts.join("\n\n");
    let max_chars = (max_tokens as usize).saturating_mul(4);
    if text.chars().count() > max_chars {
        text = text.chars().take(max_chars.saturating_sub(3)).collect();
        text.push_str("...");
    }
    text
}

fn estimate_tokens(text: &str) -> u32 {
    let chars = text.chars().count() as u32;
    chars.div_ceil(4).max(if text.is_empty() { 0 } else { 1 })
}

fn skill_matches(skill: &SkillDefinition, prompt_lower: &str) -> bool {
    if prompt_lower.contains(&skill.name.replace('-', " "))
        || prompt_lower.contains(&skill.name)
        || prompt_lower.contains(&skill.name.replace('-', "_"))
    {
        return true;
    }
    let haystack = format!("{} {}", skill.name, skill.description).to_ascii_lowercase();
    let tokens = prompt_lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| token.len() >= 4)
        .collect::<Vec<_>>();
    let mut hits = 0u32;
    for token in tokens {
        if haystack.contains(token) {
            hits += 1;
        }
    }
    hits >= 2
}

fn truncate_chars(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max.saturating_sub(3)).collect();
    format!("{clipped}...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("ade-skills-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join(".ade/skills/verify-ladder")).unwrap();
        fs::write(
            root.join(".ade/skills/verify-ladder/SKILL.md"),
            "---\nname: verify-ladder\ndescription: Run ADE verify gates G0-G5 when verifying work.\n---\n# Verify\nRun ade verify.\n",
        )
        .unwrap();
        fs::create_dir_all(root.join(".ade/skills/verify-ladder/references")).unwrap();
        fs::write(
            root.join(".ade/skills/verify-ladder/references/gates.md"),
            "G0 probe · G3 unit tests · never self-certify.\n",
        )
        .unwrap();
        fs::create_dir_all(root.join(".ade/skills/accidental-data-loss-prevention")).unwrap();
        fs::write(
            root.join(".ade/skills/accidental-data-loss-prevention/SKILL.md"),
            "---\nname: accidental-data-loss-prevention\ndescription: Stop before destructive deletes.\nalwaysApply: true\n---\n# Stop\nAsk first.\n",
        )
        .unwrap();
        fs::create_dir_all(root.join(".ade/skills/rust-workspace-ade")).unwrap();
        fs::write(
            root.join(".ade/skills/rust-workspace-ade/SKILL.md"),
            "---\nname: rust-workspace-ade\ndescription: Cargo workspace tips for ADE.\n---\n# Rust\nUse cargo check.\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn loads_skills_and_always_applies() {
        let root = fixture();
        let loader = SkillLoader::new(&root);
        let skills = loader.load_all().unwrap();
        assert_eq!(skills.len(), 3);
        let prompt = loader
            .prompt_context("please help verifying the build", 4_000)
            .unwrap();
        assert!(prompt.contains("AVAILABLE SKILLS"));
        assert!(prompt.contains("accidental-data-loss-prevention"));
        assert!(prompt.contains("SKILL verify-ladder") || prompt.contains("verify-ladder"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_only_when_no_match() {
        let root = fixture();
        let loader = SkillLoader::new(&root);
        let prompt = loader.prompt_context("say hello politely", 4_000).unwrap();
        assert!(prompt.contains("AVAILABLE SKILLS"));
        assert!(prompt.contains("T1 catalog"));
        assert!(prompt.contains("SKILL accidental-data-loss-prevention"));
        assert!(!prompt.contains("SKILL rust-workspace-ade"));
        assert!(!prompt.contains("Use cargo check"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn match_injects_body() {
        let root = fixture();
        let loader = SkillLoader::new(&root);
        let prompt = loader
            .prompt_context("help with the rust workspace ade cargo checks", 4_000)
            .unwrap();
        assert!(prompt.contains("SKILL rust-workspace-ade"));
        assert!(prompt.contains("Use cargo check"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn activate_returns_skill_or_errors() {
        let root = fixture();
        let loader = SkillLoader::new(&root);
        let skill = loader.activate("verify-ladder").unwrap();
        assert_eq!(skill.name, "verify-ladder");
        assert!(skill.body.contains("ade verify"));
        assert!(skill.body.contains("References (T3)"));
        assert!(skill.body.contains("G0 probe"));
        let err = loader.activate("missing-skill").unwrap_err().to_string();
        assert!(err.contains("not found"));
        let empty = loader.activate("  ").unwrap_err().to_string();
        assert!(empty.contains("empty"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn empty_skills_dir_is_ok() {
        let root = std::env::temp_dir().join(format!("ade-skills-empty-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let loader = SkillLoader::new(&root);
        assert!(loader.load_all().unwrap().is_empty());
        assert!(loader.prompt_context("hi", 100).unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
