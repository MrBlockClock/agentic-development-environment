//! One-shot shell for agent host tool `shell::run_command`.
//!
//! Suggest/Propose: inspect-only commands (list/read/print).
//! Apply/Act/Automate: full commands minus a dangerous deny list.
//! Optional `cwd` may target the workspace or paths under the user profile
//! (e.g. Desktop) — not an interactive PTY.

use ade_core::error::AdeError;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

const DEFAULT_TIMEOUT_SECS: u64 = 60;
const MAX_TIMEOUT_SECS: u64 = 300;
const MAX_OUTPUT_CHARS: usize = 24_000;

/// Options for [`run_command`].
#[derive(Debug, Clone, Default)]
pub struct ShellOptions<'a> {
    /// When true, only inspect/list/read-style commands are allowed.
    pub inspect_only: bool,
    /// Optional working directory (absolute, `~`, `$env:USERPROFILE\…`, or workspace-relative).
    pub cwd: Option<&'a str>,
}

/// Deny high-blast-radius commands. Autonomy Act/Automate is not a blank check.
pub fn dangerous_command_reason(command: &str) -> Option<&'static str> {
    let lower = command.to_ascii_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();

    const DENY: &[(&str, &str)] = &[
        ("rm-rf/", "recursive delete of filesystem root"),
        ("rm-rf/*", "recursive delete of filesystem root"),
        ("formatc:", "disk format"),
        ("format d:", "disk format"),
        ("mkfs.", "filesystem format"),
        (":(){:", "fork bomb"),
        ("shutdown", "system shutdown/reboot"),
        ("restart-computer", "system shutdown/reboot"),
        ("stop-computer", "system shutdown/reboot"),
        (
            "remove-item-recurse-forcec:\\",
            "recursive delete of drive root",
        ),
        ("del/s/qc:\\", "recursive delete of drive root"),
        ("rd/s/qc:\\", "recursive delete of drive root"),
        ("cipher/w:", "secure wipe"),
        ("diskpart", "disk partitioning"),
    ];

    for (needle, reason) in DENY {
        let needle_compact: String = needle.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.contains(&needle_compact) || lower.contains(needle) {
            return Some(reason);
        }
    }

    // Broad recursive wipe patterns with drive roots / home.
    if (lower.contains("rm -rf") || lower.contains("rm -fr"))
        && (lower.contains(" /") || lower.contains(" ~") || lower.contains(" $home"))
    {
        return Some("recursive delete outside a project path");
    }
    if lower.contains("remove-item")
        && lower.contains("-recurse")
        && (lower.contains("c:\\") || lower.contains("$env:userprofile") || lower.contains("~"))
    {
        return Some("recursive delete of a sensitive root");
    }

    None
}

/// Why an inspect-only (Suggest) shell call is blocked, if any.
pub fn inspect_block_reason(command: &str) -> Option<&'static str> {
    let lower = command.to_ascii_lowercase();

    // Output redirects mutate the filesystem.
    if lower.contains('>') {
        return Some("output redirects are not allowed in Suggest shell; switch to Apply");
    }

    const MUTATE: &[&str] = &[
        "move-item",
        "copy-item",
        "remove-item",
        "rename-item",
        "new-item",
        "set-content",
        "add-content",
        "clear-content",
        "out-file",
        "set-item",
        "clear-item",
        "invoke-webrequest",
        "invoke-restmethod",
        "start-process",
        "stop-process",
        "kill ",
        "mkdir ",
        " md ",
        "rmdir",
        "rm -",
        " rm ",
        "del ",
        "erase ",
        "ren ",
        "rename ",
        "copy ",
        "move ",
        "mv ",
        "cp ",
        "touch ",
        "chmod ",
        "chown ",
        "tee ",
        "install ",
        "uninstall",
        "npm install",
        "npm uninstall",
        "pip install",
        "cargo install",
        "git commit",
        "git push",
        "git reset",
        "git checkout",
        "git merge",
        "git rebase",
        "git clean",
    ];
    for needle in MUTATE {
        if lower.contains(needle) {
            return Some("mutating shell is Apply-only; Suggest shell is inspect/list/read");
        }
    }
    if lower.contains("find") && (lower.contains("-delete") || lower.contains("-exec")) {
        return Some("find -delete/-exec is Apply-only");
    }

    for segment in split_shell_segments(&lower) {
        let first = first_token(segment);
        if first.is_empty() {
            continue;
        }
        if !is_inspect_first_token(first) {
            return Some(
                "command not on Suggest inspect allowlist; switch to Apply for full shell",
            );
        }
    }

    None
}

/// True when Suggest/Propose may run this command (inspect-only).
pub fn is_inspect_command(command: &str) -> bool {
    inspect_block_reason(command).is_none() && !command.trim().is_empty()
}

fn split_shell_segments(command: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = command.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // |  ;  &&  ||
        if bytes[i] == b'|' && (i + 1 >= bytes.len() || bytes[i + 1] != b'|') {
            parts.push(command[start..i].trim());
            start = i + 1;
            i += 1;
            continue;
        }
        if bytes[i] == b';' {
            parts.push(command[start..i].trim());
            start = i + 1;
            i += 1;
            continue;
        }
        if i + 1 < bytes.len()
            && ((bytes[i] == b'&' && bytes[i + 1] == b'&')
                || (bytes[i] == b'|' && bytes[i + 1] == b'|'))
        {
            parts.push(command[start..i].trim());
            start = i + 2;
            i += 2;
            continue;
        }
        i += 1;
    }
    parts.push(command[start..].trim());
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}

fn first_token(segment: &str) -> &str {
    let trimmed = segment.trim().trim_start_matches('@');
    // Skip env assignments like FOO=bar cmd
    let mut rest = trimmed;
    loop {
        let Some((head, tail)) = rest.split_once(char::is_whitespace) else {
            return strip_path_prefix(rest);
        };
        if head.contains('=') && !head.starts_with('-') {
            rest = tail.trim_start();
            continue;
        }
        return strip_path_prefix(head);
    }
}

fn strip_path_prefix(token: &str) -> &str {
    let token = token.trim_matches(|c| c == '"' || c == '\'');
    Path::new(token)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(token)
}

fn is_inspect_first_token(token: &str) -> bool {
    let t = token.to_ascii_lowercase();
    const ALLOW: &[&str] = &[
        // PowerShell
        "get-childitem",
        "gci",
        "ls",
        "dir",
        "get-location",
        "gl",
        "pwd",
        "get-item",
        "gi",
        "test-path",
        "resolve-path",
        "get-content",
        "gc",
        "cat",
        "type",
        "select-object",
        "where-object",
        "foreach-object",
        "sort-object",
        "measure-object",
        "format-table",
        "format-list",
        "out-string",
        "write-output",
        "echo",
        "write-host",
        "select-string",
        "convertto-json",
        "convertfrom-json",
        "get-date",
        "whoami",
        "hostname",
        "get-filehash",
        "measure-command",
        // Unix / common
        "head",
        "tail",
        "wc",
        "file",
        "stat",
        "which",
        "command",
        "true",
        "false",
        "test",
        "[",
        "basename",
        "dirname",
        "realpath",
        "readlink",
        "tree",
        "du",
        "df",
        "find",
        "rg",
        "grep",
        "awk",
        "sed",
        "cut",
        "sort",
        "uniq",
        "tr",
        "printf",
        "env",
        "printenv",
        "uname",
        "date",
        "id",
        "groups",
    ];
    ALLOW.iter().any(|a| *a == t)
}

pub fn looks_like_ade_rebuild(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    lower.contains("cargo")
        && lower.contains("build")
        && (lower.contains("ade-desktop")
            || lower.contains("ade-desktop-app")
            || lower.contains("-p ade"))
}

/// Soft hint when rebuilding while ADE binaries may be locked (Windows).
pub fn rebuild_lock_hint(command: &str) -> Option<String> {
    if !looks_like_ade_rebuild(command) {
        return None;
    }
    #[cfg(windows)]
    {
        for name in ["ade-desktop-app.exe", "ade.exe"] {
            if process_appears_running(name) {
                return Some(format!(
                    "note: {name} appears running — cargo build may hit access denied / os error 5. Stop ADE or exclude that package."
                ));
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
    None
}

#[cfg(windows)]
fn process_appears_running(image_name: &str) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {image_name}"), "/NH"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|stdout| {
            stdout
                .to_ascii_lowercase()
                .contains(&image_name.to_ascii_lowercase())
        })
}

/// Resolve cwd: workspace default, workspace-relative, `~` / `$env:USERPROFILE`, or absolute under profile/workspace.
pub fn resolve_cwd(workspace_root: &Path, cwd: Option<&str>) -> Result<PathBuf, AdeError> {
    let Some(raw) = cwd.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(workspace_root.to_path_buf());
    };

    let expanded = expand_user_path(raw);
    let candidate = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        workspace_root.join(&expanded)
    };

    let canonical = candidate
        .canonicalize()
        .map_err(|e| AdeError::Config(format!("cwd '{raw}' not found: {e}")))?;

    let workspace_canon = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());

    if path_is_within(&canonical, &workspace_canon) {
        return Ok(canonical);
    }

    if let Some(home) = user_home_dir() {
        let home_canon = home.canonicalize().unwrap_or(home);
        if path_is_within(&canonical, &home_canon) {
            return Ok(canonical);
        }
    }

    Err(AdeError::Authorization(
        "cwd must be inside the workspace or your user profile (e.g. Desktop)".into(),
    ))
}

fn expand_user_path(raw: &str) -> String {
    let mut s = raw.to_string();
    if let Some(home) = user_home_dir() {
        let home_str = home.to_string_lossy().into_owned();
        if s.starts_with("~/") || s.starts_with("~\\") {
            s = format!("{home_str}{}", &s[1..]);
        } else if s == "~" {
            s = home_str.clone();
        }
        for needle in [
            "$env:USERPROFILE",
            "$env:userprofile",
            "%USERPROFILE%",
            "%userprofile%",
        ] {
            let lower = s.to_ascii_lowercase();
            let n = needle.to_ascii_lowercase();
            if let Some(idx) = lower.find(&n) {
                s = format!("{}{}{}", &s[..idx], home_str, &s[idx + needle.len()..]);
            }
        }
        for needle in ["$HOME", "$home"] {
            if let Some(idx) = s.find(needle) {
                s = format!("{}{}{}", &s[..idx], home_str, &s[idx + needle.len()..]);
            }
        }
    }
    s
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

pub async fn run_command(
    workspace_root: &Path,
    command: &str,
    timeout_secs: Option<u64>,
    options: ShellOptions<'_>,
) -> Result<String, AdeError> {
    let command = command.trim();
    if command.is_empty() {
        return Err(AdeError::Config("command is required".into()));
    }
    if let Some(reason) = dangerous_command_reason(command) {
        return Err(AdeError::Authorization(format!(
            "command blocked ({reason}): refuse high-blast-radius shell"
        )));
    }
    if options.inspect_only {
        if let Some(reason) = inspect_block_reason(command) {
            return Err(AdeError::Authorization(format!(
                "{reason}. Use Apply mode for mkdir/move/write shell."
            )));
        }
    }

    let cwd = resolve_cwd(workspace_root, options.cwd)?;
    let secs = timeout_secs
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
        .clamp(1, MAX_TIMEOUT_SECS);

    let mut hint = String::new();
    if let Some(rebuild) = rebuild_lock_hint(command) {
        hint = format!("{rebuild}\n\n");
    }
    if options.inspect_only {
        hint.push_str(&format!("mode=inspect cwd={}\n", cwd.display()));
    }

    #[cfg(windows)]
    let mut child = Command::new("powershell.exe");
    #[cfg(windows)]
    {
        child
            .args(["-NoProfile", "-NonInteractive", "-Command", command])
            .current_dir(&cwd)
            .kill_on_drop(true);
    }

    #[cfg(not(windows))]
    let mut child = Command::new("sh");
    #[cfg(not(windows))]
    {
        child
            .args(["-lc", command])
            .current_dir(&cwd)
            .kill_on_drop(true);
    }

    let joined = timeout(Duration::from_secs(secs), child.output())
        .await
        .map_err(|_| AdeError::Other(format!("command timed out after {secs}s")))?
        .map_err(|error| AdeError::Other(format!("spawn failed: {error}")))?;

    let stdout = String::from_utf8_lossy(&joined.stdout);
    let stderr = String::from_utf8_lossy(&joined.stderr);
    let code = joined.status.code();
    let mut body = String::new();
    body.push_str(&hint);
    body.push_str(&format!(
        "exit {}\n",
        code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
    ));
    if !stdout.is_empty() {
        body.push_str("--- stdout ---\n");
        body.push_str(&stdout);
        if !stdout.ends_with('\n') {
            body.push('\n');
        }
    }
    if !stderr.is_empty() {
        body.push_str("--- stderr ---\n");
        body.push_str(&stderr);
        if !stderr.ends_with('\n') {
            body.push('\n');
        }
    }
    if stdout.is_empty() && stderr.is_empty() {
        body.push_str("(no output)\n");
    }

    Ok(truncate_chars(&body, MAX_OUTPUT_CHARS))
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max).collect();
    format!("{clipped}\n\n…[truncated at {max} chars]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_rm_rf_root() {
        assert!(dangerous_command_reason("rm -rf /").is_some());
        assert!(dangerous_command_reason("echo hi").is_none());
    }

    #[test]
    fn detects_ade_rebuild() {
        assert!(looks_like_ade_rebuild("cargo build -p ade-desktop-app"));
        assert!(!looks_like_ade_rebuild("cargo test -p ade-core"));
    }

    #[test]
    fn inspect_allows_listing() {
        assert!(is_inspect_command(
            "Get-ChildItem $env:USERPROFILE\\Desktop"
        ));
        assert!(is_inspect_command("ls -la"));
        assert!(is_inspect_command("pwd"));
        assert!(is_inspect_command(
            "Get-ChildItem | Select-Object Name, Length"
        ));
    }

    #[test]
    fn inspect_blocks_mutations() {
        assert!(!is_inspect_command("Move-Item a.pdf PDFs\\"));
        assert!(!is_inspect_command("New-Item -ItemType Directory PDFs"));
        assert!(!is_inspect_command("echo hi > out.txt"));
        assert!(!is_inspect_command("cargo build -p ade-core"));
    }

    #[test]
    fn resolve_cwd_defaults_to_workspace() {
        let root = std::env::temp_dir();
        let resolved = resolve_cwd(&root, None).unwrap();
        assert_eq!(
            resolved.canonicalize().unwrap_or(resolved.clone()),
            root.canonicalize().unwrap_or(root)
        );
    }

    #[test]
    fn resolve_cwd_accepts_home_tilde() {
        let Some(home) = user_home_dir() else {
            return;
        };
        let root = std::env::temp_dir();
        let resolved = resolve_cwd(&root, Some("~")).unwrap();
        assert_eq!(
            resolved.canonicalize().unwrap_or(resolved.clone()),
            home.canonicalize().unwrap_or(home)
        );
    }
}
