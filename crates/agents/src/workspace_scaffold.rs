//! Create named ADE workspaces on disk (no Tauri). Desktop attaches via HostIntent.

use ade_core::error::AdeError;
use chrono::Utc;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Sanitize a folder name (no separators / traversal).
pub fn sanitize_workspace_name(raw: &str) -> Result<String, AdeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AdeError::Config("workspace name is required".into()));
    }
    if trimmed.len() > 64 {
        return Err(AdeError::Config(
            "workspace name must be 64 characters or fewer".into(),
        ));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(AdeError::Config(
            "workspace name cannot be '.' or '..'".into(),
        ));
    }
    if trimmed.contains(['/', '\\', ':']) || trimmed.contains("..") {
        return Err(AdeError::Config(
            "workspace name cannot contain path separators or '..'".into(),
        ));
    }
    let cleaned: String = trimmed
        .chars()
        .map(|ch| match ch {
            '<' | '>' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.');
    if cleaned.is_empty() {
        return Err(AdeError::Config(
            "workspace name is empty after sanitizing".into(),
        ));
    }
    Ok(cleaned.to_string())
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn user_desktop_dir() -> Option<PathBuf> {
    let home = user_home_dir()?;
    let desktop = home.join("Desktop");
    if desktop.is_dir() {
        Some(desktop)
    } else {
        None
    }
}

fn default_workspace_parent() -> Option<PathBuf> {
    user_desktop_dir().or_else(user_home_dir)
}

fn minimal_agents_md(project_name: &str, root: &Path) -> String {
    format!(
        r#"# {project_name} — Agent Contract

Created by ADE (`workspace__create_named`).

## Authority Order

1. Law/security/human direction
2. CI, tests, schemas
3. This AGENTS.md
4. `.ade/rules/` scoped rules
5. Task/issue acceptance criteria
6. Provider/adapter files
7. Chat memory

## Golden Path

- **Root:** `{root}`
- Prefer Apply + `fs__write_file` for project files — do not dump full apps as chat blueprints.
- Open local servers with `browser__open` and an explicit `http://localhost:PORT` URL.

## Notes

- Environment Audit diagnoses setup gaps for this folder.
- Home is where you ask ADE to work in this environment.
"#,
        project_name = project_name,
        root = root.display()
    )
}

fn ensure_workspace_identity(root: &Path, project_name: &str) -> Result<(), AdeError> {
    let ade_dir = root.join(".ade");
    std::fs::create_dir_all(&ade_dir)
        .map_err(|error| AdeError::Other(format!("create .ade: {error}")))?;
    let identity_path = ade_dir.join("workspace.json");
    if !identity_path.is_file() {
        let payload = serde_json::json!({
            "schema": "ade.workspace/v1",
            "id": Uuid::new_v4().to_string(),
            "name": project_name,
            "created_at": Utc::now().to_rfc3339(),
            "created_by": "agent",
        });
        std::fs::write(
            &identity_path,
            serde_json::to_string_pretty(&payload)
                .map_err(|error| AdeError::Other(error.to_string()))?,
        )
        .map_err(|error| AdeError::Other(format!("write workspace.json: {error}")))?;
    }
    Ok(())
}

/// Create `{parent}/{name}` with AGENTS.md + `.ade/` identity. Returns canonical path.
pub fn create_named_workspace_on_disk(
    name: &str,
    parent: Option<&str>,
) -> Result<PathBuf, AdeError> {
    let name = sanitize_workspace_name(name)?;
    let parent_path = match parent.map(str::trim).filter(|value| !value.is_empty()) {
        Some(raw) => PathBuf::from(raw),
        None => default_workspace_parent().ok_or_else(|| {
            AdeError::Config("could not resolve Desktop/home parent folder".into())
        })?,
    };
    if !parent_path.exists() {
        std::fs::create_dir_all(&parent_path)
            .map_err(|error| AdeError::Other(format!("create parent folder: {error}")))?;
    }
    if !parent_path.is_dir() {
        return Err(AdeError::Config(format!(
            "parent is not a directory: {}",
            parent_path.display()
        )));
    }
    let root = parent_path.join(&name);
    if root.exists() {
        if root.join("AGENTS.md").is_file() {
            return Ok(root.canonicalize().unwrap_or(root));
        }
        if !root.is_dir() {
            return Err(AdeError::Config(format!(
                "path exists and is not a folder: {}",
                root.display()
            )));
        }
    } else {
        std::fs::create_dir_all(&root)
            .map_err(|error| AdeError::Other(format!("create folder: {error}")))?;
    }
    let agents = root.join("AGENTS.md");
    if !agents.is_file() {
        std::fs::write(&agents, minimal_agents_md(&name, &root))
            .map_err(|error| AdeError::Other(format!("write AGENTS.md: {error}")))?;
    }
    ensure_workspace_identity(&root, &name)?;
    Ok(root.canonicalize().unwrap_or(root))
}

/// Normalize browser open URLs: localhost / loopback → http.
pub fn normalize_browser_open_url(raw: &str) -> Result<String, AdeError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AdeError::Config("url is required".into()));
    }
    if trimmed.eq_ignore_ascii_case("about:blank") {
        return Ok("about:blank".into());
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        let local = trimmed.starts_with("localhost")
            || trimmed.starts_with("127.0.0.1")
            || trimmed.starts_with("[::1]");
        if local {
            format!("http://{trimmed}")
        } else if trimmed.contains('.') {
            format!("https://{trimmed}")
        } else {
            return Err(AdeError::Config(
                "url must be http(s) or localhost:PORT".into(),
            ));
        }
    };
    let lower = with_scheme.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://") || lower == "about:blank") {
        return Err(AdeError::Config(
            "only http, https, and about:blank are allowed".into(),
        ));
    }
    Ok(with_scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_paths() {
        assert!(sanitize_workspace_name("").is_err());
        assert!(sanitize_workspace_name("a/b").is_err());
        assert_eq!(sanitize_workspace_name(" Demo ").unwrap(), "Demo");
    }

    #[test]
    fn localhost_defaults_to_http() {
        assert_eq!(
            normalize_browser_open_url("localhost:3000").unwrap(),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_browser_open_url("http://localhost:3000").unwrap(),
            "http://localhost:3000"
        );
    }

    #[test]
    fn create_named_writes_agents() {
        let parent = std::env::temp_dir().join(format!("ade-ws-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&parent).unwrap();
        let root =
            create_named_workspace_on_disk("DemoApp", Some(parent.to_str().unwrap())).unwrap();
        assert!(root.join("AGENTS.md").is_file());
        assert!(root.join(".ade").join("workspace.json").is_file());
        let _ = std::fs::remove_dir_all(parent);
    }
}
