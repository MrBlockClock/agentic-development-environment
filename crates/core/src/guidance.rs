//! Global + workspace guidance paths, profiles, and machine audit.

use crate::error::AdeError;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

/// Machine ADE home (no `ADE_ENV` subdir) for persistent personal guidance.
pub fn ade_machine_home() -> PathBuf {
    if let Some(dir) = non_empty("LOCALAPPDATA") {
        return PathBuf::from(dir).join("ade");
    }
    if let Some(dir) = non_empty("APPDATA") {
        return PathBuf::from(dir).join("ade");
    }
    if let Some(dir) = non_empty("XDG_DATA_HOME") {
        return PathBuf::from(dir).join("ade");
    }
    if let Some(home) = non_empty("HOME") {
        return PathBuf::from(home).join(".local/share/ade");
    }
    PathBuf::from("./data/ade")
}

pub fn guidance_root() -> PathBuf {
    ade_machine_home().join("guidance")
}

pub fn global_rules_dir() -> PathBuf {
    guidance_root().join("rules")
}

pub fn global_skills_dir() -> PathBuf {
    guidance_root().join("skills")
}

pub fn global_profiles_dir() -> PathBuf {
    guidance_root().join("profiles")
}

pub fn global_audit_path() -> PathBuf {
    guidance_root().join("audit").join("latest.json")
}

pub fn active_profile_path() -> PathBuf {
    guidance_root().join("active-profile.txt")
}

fn non_empty(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.trim().is_empty())
}

/// Ensure global guidance directories exist.
pub fn ensure_guidance_dirs() -> Result<(), AdeError> {
    for dir in [
        global_rules_dir(),
        global_skills_dir(),
        global_profiles_dir(),
        guidance_root().join("audit"),
    ] {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuidanceProfile {
    pub id: String,
    /// Pack ids that should remain when this profile is active.
    pub packs: Vec<String>,
}

/// Load profiles from workspace `.ade/profiles` then global (workspace wins on id).
pub fn load_profiles(workspace: &Path) -> Result<Vec<GuidanceProfile>, AdeError> {
    let mut by_id = std::collections::BTreeMap::new();
    for dir in [
        global_profiles_dir(),
        workspace.join(".ade").join("profiles"),
    ] {
        if !dir.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)?.filter_map(Result::ok) {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "toml" && ext != "json" {
                continue;
            }
            if let Some(profile) = parse_profile_file(&path)? {
                by_id.insert(profile.id.clone(), profile);
            }
        }
    }
    Ok(by_id.into_values().collect())
}

pub fn read_active_profile_id() -> Option<String> {
    let path = active_profile_path();
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn write_active_profile_id(id: Option<&str>) -> Result<(), AdeError> {
    ensure_guidance_dirs()?;
    let path = active_profile_path();
    match id {
        Some(id) if !id.trim().is_empty() => {
            std::fs::write(path, format!("{}\n", id.trim()))?;
        }
        _ => {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

fn parse_profile_file(path: &Path) -> Result<Option<GuidanceProfile>, AdeError> {
    let id = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if id.is_empty() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    if path.extension().is_some_and(|e| e == "json") {
        #[derive(Deserialize)]
        struct Body {
            packs: Option<Vec<String>>,
        }
        let body: Body = serde_json::from_str(&raw).map_err(|e| AdeError::Config(e.to_string()))?;
        return Ok(Some(GuidanceProfile {
            id,
            packs: body.packs.unwrap_or_default(),
        }));
    }
    // Minimal TOML: packs = ["a", "b"]
    let packs = parse_packs_toml(&raw);
    Ok(Some(GuidanceProfile { id, packs }))
}

fn parse_packs_toml(raw: &str) -> Vec<String> {
    for line in raw.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("packs")
            .map(str::trim)
            .and_then(|s| s.strip_prefix('='))
            .map(str::trim)
        else {
            continue;
        };
        return rest
            .trim_matches(['[', ']'])
            .split(',')
            .map(|item| item.trim().trim_matches(['"', '\'']).to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    Vec::new()
}

/// Extract optional `pack:` from markdown frontmatter.
pub fn frontmatter_pack(content: &str) -> Option<String> {
    let rest = content.strip_prefix("---")?;
    let header = rest.split_once("---").map(|(h, _)| h).unwrap_or("");
    for line in header.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("pack:")
            .or_else(|| trimmed.strip_prefix("pack :"))
        {
            let pack = rest.trim().trim_matches(['"', '\'']).to_string();
            if !pack.is_empty() {
                return Some(pack);
            }
        }
    }
    None
}

/// Keep item if no active profile, or pack is untagged, or pack is in profile.
/// Deny rules with a pack still pass when `force_keep_denies` and the item is a deny.
pub fn pack_allowed(pack: Option<&str>, active: Option<&GuidanceProfile>, is_deny: bool) -> bool {
    let Some(profile) = active else {
        return true;
    };
    if profile.packs.is_empty() {
        return true;
    }
    match pack {
        None => true, // untagged always loads
        Some(p) if profile.packs.iter().any(|x| x == p) => true,
        Some(_) if is_deny => true, // deny union safety: keep tagged denies too when profile filters
        Some(_) => false,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAuditReport {
    pub schema: String,
    pub ok: bool,
    pub checks: Vec<GlobalAuditCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAuditCheck {
    pub id: String,
    pub label: String,
    pub passed: bool,
    pub detail: String,
}

pub const GLOBAL_AUDIT_SCHEMA: &str = "ade.global-audit/v1";

/// Machine / ADE-install health (not workspace L0–L11).
pub fn run_global_audit(preferred_workspace: Option<&Path>) -> GlobalAuditReport {
    let _ = ensure_guidance_dirs();
    let mut checks = Vec::new();

    let rules = global_rules_dir();
    checks.push(GlobalAuditCheck {
        id: "guidance_rules".into(),
        label: "Global rules directory".into(),
        passed: rules.is_dir(),
        detail: rules.display().to_string(),
    });

    let skills = global_skills_dir();
    checks.push(GlobalAuditCheck {
        id: "guidance_skills".into(),
        label: "Global skills directory".into(),
        passed: skills.is_dir(),
        detail: skills.display().to_string(),
    });

    let ws_ptr = ade_machine_home().join("workspace-root.txt");
    let ptr_ok = ws_ptr.is_file() || preferred_workspace.is_some_and(|p| p.exists());
    checks.push(GlobalAuditCheck {
        id: "workspace_pointer".into(),
        label: "Preferred workspace pointer".into(),
        passed: ptr_ok,
        detail: if ws_ptr.is_file() {
            std::fs::read_to_string(&ws_ptr)
                .unwrap_or_else(|_| ws_ptr.display().to_string())
                .trim()
                .to_string()
        } else if let Some(p) = preferred_workspace {
            p.display().to_string()
        } else {
            "No workspace-root.txt yet".into()
        },
    });

    let data_dir = crate::config::AdeConfig::load()
        .map(|c| c.data_dir)
        .unwrap_or_else(|_| ade_machine_home().join("local"));
    checks.push(GlobalAuditCheck {
        id: "data_dir".into(),
        label: "ADE data directory".into(),
        passed: true,
        detail: data_dir.display().to_string(),
    });

    let turso = crate::config::AdeConfig::load()
        .ok()
        .and_then(|c| c.turso_url)
        .filter(|s| !s.is_empty());
    checks.push(GlobalAuditCheck {
        id: "turso".into(),
        label: "Turso / SQL URL configured".into(),
        passed: turso.is_some(),
        detail: turso.unwrap_or_else(|| "Not set (JSON/.ade state still works)".into()),
    });

    let rule_count = std::fs::read_dir(&rules)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "mdc"))
                .count()
        })
        .unwrap_or(0);
    checks.push(GlobalAuditCheck {
        id: "global_rule_count".into(),
        label: "Global rule files".into(),
        passed: true,
        detail: format!("{rule_count} .mdc"),
    });

    let ok = checks.iter().filter(|c| c.id != "turso").all(|c| c.passed);
    let report = GlobalAuditReport {
        schema: GLOBAL_AUDIT_SCHEMA.into(),
        ok,
        checks,
    };
    if let Ok(()) = std::fs::create_dir_all(guidance_root().join("audit")) {
        let _ = std::fs::write(
            global_audit_path(),
            serde_json::to_string_pretty(&report).unwrap_or_default(),
        );
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_packs_line() {
        let packs = parse_packs_toml("packs = [\"a\", \"b\"]\n");
        assert_eq!(packs, vec!["a", "b"]);
    }

    #[test]
    fn pack_filter_keeps_untagged() {
        let profile = GuidanceProfile {
            id: "daily".into(),
            packs: vec!["money".into()],
        };
        assert!(pack_allowed(None, Some(&profile), false));
        assert!(pack_allowed(Some("money"), Some(&profile), false));
        assert!(!pack_allowed(Some("other"), Some(&profile), false));
        assert!(pack_allowed(Some("other"), Some(&profile), true));
    }
}
