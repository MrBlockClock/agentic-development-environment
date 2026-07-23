use ade_core::authority::AuthorityLevel;
use ade_core::error::AdeError;
use ade_core::ignore::SensitivePathPolicy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct AuthorityEnforcer {
    root: Option<PathBuf>,
    contract: String,
    owned_paths: Vec<PathBuf>,
    scoped_rules: Vec<ScopedRule>,
}

impl Default for AuthorityEnforcer {
    fn default() -> Self {
        Self::read_only()
    }
}

impl AuthorityEnforcer {
    /// Safe fallback for sessions that have no approved PLAN scope.
    pub fn read_only() -> Self {
        Self {
            root: None,
            contract: String::new(),
            owned_paths: vec![],
            scoped_rules: vec![],
        }
    }

    pub fn load(
        root: impl Into<PathBuf>,
        owned_paths: impl IntoIterator<Item = String>,
    ) -> Result<Self, AdeError> {
        let root = root.into().canonicalize().map_err(|error| {
            AdeError::Authorization(format!("cannot resolve workspace root: {error}"))
        })?;
        let contract_path = root.join("AGENTS.md");
        let contract = std::fs::read_to_string(&contract_path).map_err(|error| {
            AdeError::Authorization(format!(
                "canonical AGENTS.md is required before agent tools run: {error}"
            ))
        })?;
        let owned_paths = owned_paths
            .into_iter()
            .map(|path| validate_relative_path(&path))
            .collect::<Result<Vec<_>, _>>()?;
        let scoped_rules = load_scoped_rules(&root)?;
        Ok(Self {
            root: Some(root),
            contract,
            owned_paths,
            scoped_rules,
        })
    }

    pub fn prompt_context(&self) -> String {
        let mut context = format!("CANONICAL AGENTS.md:\n{}", self.contract);
        if !self.owned_paths.is_empty() {
            let owned = self
                .owned_paths
                .iter()
                .map(|path| path_text(path))
                .collect::<Vec<_>>()
                .join(", ");
            context.push_str(&format!("\n\nACTIVE WRITE SCOPE:\n{owned}"));
        }
        for rule in &self.scoped_rules {
            context.push_str(&format!(
                "\n\nSCOPED RULE {} (patterns: {}):\n{}",
                rule.source,
                rule.patterns.join(", "),
                rule.content
            ));
        }
        context
    }

    pub fn owned_paths(&self) -> Vec<String> {
        self.owned_paths
            .iter()
            .map(|path| path_text(path))
            .collect()
    }

    pub fn check_conflict(&self, existing: &AuthorityLevel, incoming: &AuthorityLevel) -> bool {
        incoming.priority() >= existing.priority()
    }

    pub fn resolve_and_report(&self, conflict: (&AuthorityLevel, &AuthorityLevel)) -> String {
        let (existing, incoming) = conflict;
        let winner = AuthorityLevel::resolve(existing, incoming);
        format!(
            "Authority conflict: {:?} vs {:?} — {:?} wins",
            existing, incoming, winner
        )
    }

    /// Agent-driven tool calls: write scope is limited to approved PLAN owned_paths.
    pub fn authorize_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: &Value,
    ) -> Result<AuthorityDecision, AdeError> {
        self.authorize_tool_call(&ToolAuthRequest {
            server: server.into(),
            tool: tool.into(),
            arguments: arguments.clone(),
            input_schema: None,
            annotations: None,
            write_scope: WriteScope::PlanOwnedPaths,
            human_approved: false,
        })
    }

    /// Human-reviewed MCP console/CLI calls still obey AGENTS.md and scoped rules,
    /// but the human click/flag is the write approval instead of PLAN owned_paths.
    pub fn authorize_human_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: &Value,
    ) -> Result<AuthorityDecision, AdeError> {
        self.authorize_tool_call(&ToolAuthRequest {
            server: server.into(),
            tool: tool.into(),
            arguments: arguments.clone(),
            input_schema: None,
            annotations: None,
            write_scope: WriteScope::HumanReviewed,
            human_approved: true,
        })
    }

    pub fn authorize_tool_call(
        &self,
        request: &ToolAuthRequest,
    ) -> Result<AuthorityDecision, AdeError> {
        let effect = classify_tool_effect(request);
        let paths = extract_paths(&request.arguments, request.input_schema.as_ref());

        for candidate in &paths {
            if SensitivePathPolicy::path_is_blocked(candidate) {
                return Err(AdeError::Authorization(format!(
                    "tool {}/{} denied by sensitive-path policy: '{}'",
                    request.server, request.tool, candidate
                )));
            }
        }

        match effect {
            ToolEffect::ReadOnly => {
                return Ok(AuthorityDecision {
                    allowed: true,
                    reason: format!(
                        "{}/{} is classified read-only",
                        request.server, request.tool
                    ),
                    checked_paths: paths,
                    effect,
                });
            }
            ToolEffect::ExternalWrite | ToolEffect::ProcessExecution | ToolEffect::Unknown
                if !request.human_approved =>
            {
                return Err(AdeError::Authorization(format!(
                    "tool {}/{} classified as {:?} requires human approval",
                    request.server, request.tool, effect
                )));
            }
            ToolEffect::WorkspaceWrite
                if matches!(request.write_scope, WriteScope::PlanOwnedPaths) => {}
            ToolEffect::WorkspaceWrite
            | ToolEffect::ExternalWrite
            | ToolEffect::ProcessExecution
            | ToolEffect::Unknown => {}
        }

        if matches!(
            effect,
            ToolEffect::ExternalWrite | ToolEffect::ProcessExecution
        ) && request.human_approved
        {
            return Ok(AuthorityDecision {
                allowed: true,
                reason: format!(
                    "{}/{} ({:?}) allowed via human approval",
                    request.server, request.tool, effect
                ),
                checked_paths: paths,
                effect,
            });
        }

        if matches!(effect, ToolEffect::Unknown) && request.human_approved {
            return Ok(AuthorityDecision {
                allowed: true,
                reason: format!(
                    "{}/{} unknown effect allowed via human approval",
                    request.server, request.tool
                ),
                checked_paths: paths,
                effect,
            });
        }

        // WorkspaceWrite (and agent-path Unknown already denied above).
        let root = self.root.as_ref().ok_or_else(|| {
            AdeError::Authorization(format!(
                "write-capable tool {}/{} denied: no runtime authority policy loaded",
                request.server, request.tool
            ))
        })?;
        if matches!(request.write_scope, WriteScope::PlanOwnedPaths) && self.owned_paths.is_empty()
        {
            return Err(AdeError::Authorization(format!(
                "write-capable tool {}/{} denied: no approved PLAN owned_paths",
                request.server, request.tool
            )));
        }
        if paths.is_empty() {
            return Err(AdeError::Authorization(format!(
                "tool {}/{} denied: classified as {:?} and arguments expose no reviewable path",
                request.server, request.tool, effect
            )));
        }

        let mut checked_paths = Vec::new();
        for candidate in paths {
            let relative = workspace_relative(root, &candidate)?;
            if matches!(request.write_scope, WriteScope::PlanOwnedPaths)
                && !self
                    .owned_paths
                    .iter()
                    .any(|owned| path_is_within(&relative, owned))
            {
                return Err(AdeError::Authorization(format!(
                    "tool {}/{} path '{}' is outside approved PLAN owned_paths",
                    request.server,
                    request.tool,
                    relative.display()
                )));
            }
            let normalized = path_text(&relative);
            if let Some(rule) = self
                .scoped_rules
                .iter()
                .find(|rule| rule.deny_writes && rule.matches(&normalized))
            {
                return Err(AdeError::Authorization(format!(
                    "tool {}/{} denied by scoped rule '{}'",
                    request.server, request.tool, rule.source
                )));
            }
            checked_paths.push(normalized);
        }

        Ok(AuthorityDecision {
            allowed: true,
            reason: match request.write_scope {
                WriteScope::PlanOwnedPaths => "write is inside approved PLAN scope".into(),
                WriteScope::HumanReviewed => {
                    "write reviewed by human and allowed by AGENTS.md/scoped rules".into()
                }
            },
            checked_paths,
            effect,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ToolAuthRequest {
    pub server: String,
    pub tool: String,
    pub arguments: Value,
    pub input_schema: Option<Value>,
    pub annotations: Option<ToolAnnotations>,
    pub write_scope: WriteScope,
    pub human_approved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    ReadOnly,
    WorkspaceWrite,
    ExternalWrite,
    ProcessExecution,
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolAnnotations {
    pub read_only_hint: Option<bool>,
    pub destructive_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
    /// Explicit ADE effect annotation (`x-ade-effect` / schema extension).
    pub ade_effect: Option<ToolEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteScope {
    PlanOwnedPaths,
    HumanReviewed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityDecision {
    pub allowed: bool,
    pub reason: String,
    pub checked_paths: Vec<String>,
    pub effect: ToolEffect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFileInfo {
    pub source: String,
    pub description: String,
    pub globs: Vec<String>,
    pub deny_writes: bool,
    pub content: String,
    /// `global` or `workspace`.
    #[serde(default = "default_workspace_scope")]
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
}

fn default_workspace_scope() -> String {
    "workspace".into()
}

/// Public listing of merged Global + workspace `.ade/rules/*.mdc`.
pub fn list_rule_files(root: impl AsRef<Path>) -> Result<Vec<RuleFileInfo>, AdeError> {
    let root = root.as_ref();
    Ok(load_scoped_rules(root)?
        .into_iter()
        .map(|rule| {
            let description = frontmatter(&rule.content)
                .lines()
                .find_map(|line| line.trim().strip_prefix("description:"))
                .map(|value| value.trim().trim_matches('"').to_string())
                .unwrap_or_default();
            RuleFileInfo {
                source: rule.source.replace('\\', "/"),
                description,
                globs: rule.patterns,
                deny_writes: rule.deny_writes,
                content: rule.content,
                scope: rule.scope,
                pack: rule.pack,
            }
        })
        .collect())
}

#[derive(Debug, Clone)]
struct ScopedRule {
    source: String,
    patterns: Vec<String>,
    deny_writes: bool,
    content: String,
    scope: String,
    pack: Option<String>,
    /// Stem used for conflict resolution (workspace wins).
    stem: String,
}

impl ScopedRule {
    fn matches(&self, path: &str) -> bool {
        self.patterns.is_empty()
            || self
                .patterns
                .iter()
                .any(|pattern| wildcard_match(pattern, path))
    }
}

fn load_scoped_rules(root: &Path) -> Result<Vec<ScopedRule>, AdeError> {
    let mut by_stem: BTreeMap<String, ScopedRule> = BTreeMap::new();

    // Global first; workspace overwrites prompt body but deny union kept via merge_rule.
    for rule in load_rules_from_dir(&ade_core::guidance::global_rules_dir(), None, "global")? {
        merge_rule_into(&mut by_stem, rule);
    }
    for rule in load_rules_from_dir(&root.join(".ade").join("rules"), Some(root), "workspace")? {
        merge_rule_into(&mut by_stem, rule);
    }

    let active = active_guidance_profile(root);
    let mut rules: Vec<ScopedRule> = by_stem.into_values().collect();
    rules.retain(|rule| {
        ade_core::guidance::pack_allowed(rule.pack.as_deref(), active.as_ref(), rule.deny_writes)
    });
    rules.sort_by(|a, b| a.source.cmp(&b.source));
    Ok(rules)
}

fn active_guidance_profile(workspace: &Path) -> Option<ade_core::guidance::GuidanceProfile> {
    let id = ade_core::guidance::read_active_profile_id()?;
    ade_core::guidance::load_profiles(workspace)
        .ok()?
        .into_iter()
        .find(|p| p.id == id)
}

fn merge_rule_into(by_stem: &mut BTreeMap<String, ScopedRule>, incoming: ScopedRule) {
    match by_stem.get_mut(&incoming.stem) {
        None => {
            by_stem.insert(incoming.stem.clone(), incoming);
        }
        Some(existing) => {
            // Workspace wins body; deny-writes union.
            let deny = existing.deny_writes || incoming.deny_writes;
            if incoming.scope == "workspace" {
                *existing = incoming;
                existing.deny_writes = deny;
            } else {
                existing.deny_writes = deny;
            }
        }
    }
}

fn load_rules_from_dir(
    rules_dir: &Path,
    strip_root: Option<&Path>,
    scope: &str,
) -> Result<Vec<ScopedRule>, AdeError> {
    if !rules_dir.is_dir() {
        return Ok(vec![]);
    }
    let mut paths = std::fs::read_dir(rules_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mdc"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let content = std::fs::read_to_string(&path)?;
            let header = frontmatter(&content);
            let patterns = header
                .lines()
                .find_map(|line| line.trim().strip_prefix("globs:"))
                .map(parse_patterns)
                .unwrap_or_default();
            let deny_writes = header.lines().any(|line| {
                matches!(
                    line.trim().to_ascii_lowercase().as_str(),
                    "write: deny" | "read_only: true" | "readonly: true"
                )
            });
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            let source = if let Some(root) = strip_root {
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string()
            } else {
                format!(
                    "global/rules/{}",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or(&stem)
                )
            };
            Ok(ScopedRule {
                source,
                patterns,
                deny_writes,
                pack: ade_core::guidance::frontmatter_pack(&content),
                content,
                scope: scope.into(),
                stem,
            })
        })
        .collect()
}

fn frontmatter(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("---") else {
        return "";
    };
    rest.split_once("---")
        .map(|(header, _)| header)
        .unwrap_or("")
}

fn parse_patterns(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_matches(['[', ']'])
        .split(',')
        .map(|item| item.trim().trim_matches(['"', '\'']).replace('\\', "/"))
        .filter(|item| !item.is_empty())
        .collect()
}

pub fn classify_tool_effect(request: &ToolAuthRequest) -> ToolEffect {
    if let Some(effect) = registry_effect(&request.server, &request.tool) {
        return effect;
    }
    if let Some(effect) = request
        .annotations
        .as_ref()
        .and_then(|annotations| annotations.ade_effect)
    {
        return effect;
    }
    if let Some(schema) = &request.input_schema {
        if let Some(effect) = schema_effect(schema) {
            return effect;
        }
    }
    if let Some(annotations) = &request.annotations {
        if annotations.destructive_hint == Some(true) || annotations.open_world_hint == Some(true) {
            return if annotations.open_world_hint == Some(true) {
                ToolEffect::ExternalWrite
            } else {
                ToolEffect::WorkspaceWrite
            };
        }
        // Read-only hints are accepted only for registry/schema-trusted paths above.
        // Spoofed call-arg tags alone never grant ReadOnly.
    }
    // Argument tags are untrusted hints and never upgrade to ReadOnly.
    if let Some(hint) = argument_effect_hint(&request.arguments) {
        if !matches!(hint, ToolEffect::ReadOnly) {
            return hint;
        }
    }
    heuristic_effect(&request.tool)
}

fn registry_effect(server: &str, tool: &str) -> Option<ToolEffect> {
    let key = format!(
        "{}::{}",
        server.to_ascii_lowercase(),
        tool.to_ascii_lowercase()
    );
    tool_registry().get(&key).copied()
}

fn tool_registry() -> &'static BTreeMap<String, ToolEffect> {
    static REGISTRY: OnceLock<BTreeMap<String, ToolEffect>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut map = BTreeMap::new();
        for (server, tool, effect) in [
            ("ade", "ade_audit_status", ToolEffect::ReadOnly),
            ("ade", "ade_plan_summary", ToolEffect::ReadOnly),
            ("ade", "ade_verify_status", ToolEffect::ReadOnly),
            ("ade", "ade_recipe_list", ToolEffect::ReadOnly),
            ("ade", "ade_key_status", ToolEffect::ReadOnly),
            ("ade", "ade_lease_list", ToolEffect::ReadOnly),
            ("ade", "activate_skill", ToolEffect::ReadOnly),
            ("ade", "compact_context", ToolEffect::ReadOnly),
            ("ade", "web_fetch", ToolEffect::ReadOnly),
            ("ade", "web_search", ToolEffect::ReadOnly),
            ("fs", "read_file", ToolEffect::ReadOnly),
            ("fs", "write_file", ToolEffect::WorkspaceWrite),
            ("fs", "list_directory", ToolEffect::ReadOnly),
            ("fs", "edit_file", ToolEffect::WorkspaceWrite),
            ("filesystem", "read_file", ToolEffect::ReadOnly),
            ("filesystem", "write_file", ToolEffect::WorkspaceWrite),
            ("git", "commit", ToolEffect::ProcessExecution),
            ("git", "push", ToolEffect::ExternalWrite),
            ("shell", "run_command", ToolEffect::ProcessExecution),
            ("db", "execute_sql", ToolEffect::ProcessExecution),
            ("http", "upload", ToolEffect::ExternalWrite),
            ("http", "put", ToolEffect::ExternalWrite),
        ] {
            map.insert(format!("{server}::{tool}"), effect);
        }
        map
    })
}

fn schema_effect(schema: &Value) -> Option<ToolEffect> {
    let Value::Object(map) = schema else {
        return None;
    };
    let raw = map
        .get("x-ade-effect")
        .or_else(|| map.get("x_ade_effect"))
        .or_else(|| map.get("ade_effect"))
        .and_then(Value::as_str)?;
    parse_effect_label(raw)
}

fn argument_effect_hint(arguments: &Value) -> Option<ToolEffect> {
    let Value::Object(map) = arguments else {
        return None;
    };
    let raw = map
        .get("x_ade_capability")
        .or_else(|| map.get("ade_capability"))
        .or_else(|| map.get("x-ade-effect"))
        .and_then(Value::as_str)?;
    parse_effect_label(raw)
}

fn parse_effect_label(raw: &str) -> Option<ToolEffect> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "read" | "readonly" | "read_only" => Some(ToolEffect::ReadOnly),
        "write" | "workspace_write" | "mutate" => Some(ToolEffect::WorkspaceWrite),
        "external_write" | "network" | "upload" => Some(ToolEffect::ExternalWrite),
        "process" | "process_execution" | "execute" | "shell" => Some(ToolEffect::ProcessExecution),
        "unknown" => Some(ToolEffect::Unknown),
        _ => None,
    }
}

fn heuristic_effect(tool: &str) -> ToolEffect {
    let name = tool.to_ascii_lowercase();
    let process_verbs = [
        "run_command",
        "execute_sql",
        "exec",
        "shell",
        "bash",
        "powershell",
        "commit",
        "rebase",
        "migrate",
    ];
    if process_verbs.iter().any(|verb| name.contains(verb)) {
        return ToolEffect::ProcessExecution;
    }
    let external_verbs = [
        "upload",
        "publish",
        "deploy",
        "webhook",
        "http_post",
        "post_",
    ];
    if external_verbs.iter().any(|verb| name.contains(verb))
        || name.ends_with("_put")
        || name == "put"
    {
        return ToolEffect::ExternalWrite;
    }
    let write_verbs = [
        "write", "edit", "patch", "move", "create", "delete", "remove", "rename", "replace",
        "mkdir", "touch", "apply", "update", "append",
    ];
    if write_verbs.iter().any(|verb| name.contains(verb)) {
        return ToolEffect::WorkspaceWrite;
    }
    let read_verbs = [
        "read", "get", "list", "search", "find", "fetch", "query", "status", "describe", "show",
        "cat", "ls", "glob", "grep", "stat", "head", "tail", "open", "inspect",
    ];
    if read_verbs.iter().any(|verb| name.contains(verb)) {
        return ToolEffect::ReadOnly;
    }
    ToolEffect::Unknown
}

pub fn extract_paths(arguments: &Value, schema: Option<&Value>) -> Vec<String> {
    let mut paths = Vec::new();
    let schema_keys = schema_path_keys(schema);
    visit_paths(None, arguments, &schema_keys, &mut paths);
    paths.sort();
    paths.dedup();
    paths
}

fn schema_path_keys(schema: Option<&Value>) -> Vec<String> {
    let mut keys = Vec::new();
    fn walk(value: &Value, keys: &mut Vec<String>) {
        let Value::Object(map) = value else {
            return;
        };
        if let Some(Value::Object(properties)) = map.get("properties") {
            for (key, property) in properties {
                let marked = property
                    .get("format")
                    .and_then(Value::as_str)
                    .is_some_and(|format| {
                        matches!(format, "uri" | "path" | "file-path" | "filepath")
                    })
                    || property
                        .get("x-ade-path")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    || is_path_key(key);
                if marked {
                    keys.push(key.to_ascii_lowercase());
                }
                walk(property, keys);
            }
        }
        if let Some(items) = map.get("items") {
            walk(items, keys);
        }
        if let Some(Value::Object(defs)) = map.get("$defs").or_else(|| map.get("definitions")) {
            for def in defs.values() {
                walk(def, keys);
            }
        }
    }
    if let Some(schema) = schema {
        walk(schema, &mut keys);
    }
    keys.sort();
    keys.dedup();
    keys
}

fn visit_paths(key: Option<&str>, value: &Value, schema_keys: &[String], paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (child_key, child) in map {
                visit_paths(Some(child_key), child, schema_keys, paths);
            }
        }
        Value::Array(values) => {
            for child in values {
                visit_paths(key, child, schema_keys, paths);
            }
        }
        Value::String(text) => {
            let key_matches = key.is_some_and(|key| {
                let lower = key.to_ascii_lowercase();
                is_path_key(&lower) || schema_keys.iter().any(|candidate| candidate == &lower)
            });
            if key_matches {
                if let Some(path) = normalize_path_candidate(text) {
                    paths.push(path);
                }
            }
        }
        _ => {}
    }
}

fn is_path_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "path"
        || key == "paths"
        || key == "file"
        || key == "files"
        || key == "uri"
        || key == "url"
        || key == "resource"
        || key == "source"
        || key == "destination"
        || key == "target"
        || key == "directory"
        || key == "cwd"
        || key.ends_with("_path")
        || key.ends_with("_paths")
        || key.ends_with("_file")
        || key.ends_with("_files")
        || key.ends_with("_uri")
}

fn normalize_path_candidate(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = trimmed
        .strip_prefix("file://")
        .or_else(|| trimmed.strip_prefix("file:"))
        .unwrap_or(trimmed);
    Some(without_scheme.to_string())
}

fn validate_relative_path(path: &str) -> Result<PathBuf, AdeError> {
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AdeError::Authorization(format!(
            "invalid PLAN owned_path '{}'",
            path.display()
        )));
    }
    Ok(path)
}

fn workspace_relative(root: &Path, candidate: &str) -> Result<PathBuf, AdeError> {
    let path = PathBuf::from(candidate);
    let absolute = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let resolved = resolve_for_scope(&absolute)?;
    let relative = resolved
        .strip_prefix(root)
        .map(PathBuf::from)
        .map_err(|_| {
            AdeError::Authorization(format!(
                "tool path '{}' escapes workspace '{}'",
                absolute.display(),
                root.display()
            ))
        })?;
    validate_relative_path(&relative.display().to_string())
}

fn resolve_for_scope(path: &Path) -> Result<PathBuf, AdeError> {
    if path.exists() {
        return path.canonicalize().map_err(AdeError::from);
    }
    let mut ancestor = path;
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        let name = ancestor.file_name().ok_or_else(|| {
            AdeError::Authorization(format!("cannot resolve tool path '{}'", path.display()))
        })?;
        suffix.push(name.to_os_string());
        ancestor = ancestor.parent().ok_or_else(|| {
            AdeError::Authorization(format!("cannot resolve tool path '{}'", path.display()))
        })?;
    }
    let mut resolved = ancestor.canonicalize()?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn path_is_within(candidate: &Path, owned: &Path) -> bool {
    owned == Path::new(".") || candidate == owned || candidate.starts_with(owned)
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let value = value.replace('\\', "/");
    let (pattern, value) = (pattern.as_bytes(), value.as_bytes());
    let mut table = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    table[0][0] = true;
    for index in 1..=pattern.len() {
        if pattern[index - 1] == b'*' {
            table[index][0] = table[index - 1][0];
        }
        for offset in 1..=value.len() {
            table[index][offset] = match pattern[index - 1] {
                b'*' => table[index - 1][offset] || table[index][offset - 1],
                b'?' => table[index - 1][offset - 1],
                byte => {
                    byte.eq_ignore_ascii_case(&value[offset - 1]) && table[index - 1][offset - 1]
                }
            };
        }
    }
    table[pattern.len()][value.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("ade-authority-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join(".ade/rules")).unwrap();
        std::fs::write(
            root.join("AGENTS.md"),
            "# Contract\n\nNEVER read/quote `.env`.",
        )
        .unwrap();
        std::fs::write(
            root.join(".ade/rules/generated.mdc"),
            "---\nglobs: [generated/**]\nwrite: deny\n---\nGenerated files are read-only.",
        )
        .unwrap();
        root
    }

    #[test]
    fn allows_owned_write_and_denies_scope_escape() {
        let root = fixture();
        let policy = AuthorityEnforcer::load(&root, ["src".to_string()]).unwrap();
        assert!(policy
            .authorize_tool(
                "fs",
                "write_file",
                &json!({ "path": root.join("src/lib.rs") })
            )
            .is_ok());
        assert!(policy
            .authorize_tool("fs", "write_file", &json!({ "path": "README.md" }))
            .is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn secret_and_scoped_rule_paths_are_denied() {
        let root = fixture();
        let policy =
            AuthorityEnforcer::load(&root, [".".to_string(), "generated".to_string()]).unwrap();
        assert!(policy
            .authorize_tool("fs", "write_file", &json!({ "path": ".env" }))
            .is_err());
        assert!(policy
            .authorize_tool(
                "fs",
                "write_file",
                &json!({ "path": "generated/client.rs" })
            )
            .is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn read_only_tools_do_not_require_plan_scope() {
        let root = fixture();
        let policy = AuthorityEnforcer::load(&root, Vec::<String>::new()).unwrap();
        assert!(policy
            .authorize_tool("fs", "read_file", &json!({ "path": "AGENTS.md" }))
            .is_ok());
        assert!(policy
            .authorize_tool("fs", "read_file", &json!({ "path": ".env" }))
            .is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn human_reviewed_writes_still_obey_secret_and_scoped_rules() {
        let root = fixture();
        let policy = AuthorityEnforcer::load(&root, Vec::<String>::new()).unwrap();
        assert!(policy
            .authorize_human_tool("fs", "write_file", &json!({ "path": "README.md" }))
            .is_ok());
        assert!(policy
            .authorize_human_tool("fs", "write_file", &json!({ "path": ".env" }))
            .is_err());
        assert!(policy
            .authorize_human_tool(
                "fs",
                "write_file",
                &json!({ "path": "generated/client.rs" })
            )
            .is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_tools_are_denied_without_human_approval() {
        let root = fixture();
        let policy = AuthorityEnforcer::load(&root, ["src".to_string()]).unwrap();
        assert!(policy
            .authorize_tool("fs", "do_something", &json!({ "note": "no path" }))
            .is_err());
        assert!(policy
            .authorize_tool(
                "fs",
                "do_something",
                &json!({ "path": root.join("src/lib.rs") })
            )
            .is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn spoofed_read_tag_cannot_bypass_registry_write() {
        let root = fixture();
        let policy = AuthorityEnforcer::load(&root, Vec::<String>::new()).unwrap();
        assert!(policy
            .authorize_tool(
                "fs",
                "write_file",
                &json!({ "path": "README.md", "x_ade_capability": "read" })
            )
            .is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_nested_schema_paths_and_blocks_secrets() {
        let root = fixture();
        let policy = AuthorityEnforcer::load(&root, ["src".to_string()]).unwrap();
        let schema = json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "uri": { "type": "string", "format": "uri" }
                        }
                    }
                }
            }
        });
        let decision = policy.authorize_tool_call(&ToolAuthRequest {
            server: "fs".into(),
            tool: "write_file".into(),
            arguments: json!({ "files": [{ "uri": "file://.env" }] }),
            input_schema: Some(schema),
            annotations: None,
            write_scope: WriteScope::PlanOwnedPaths,
            human_approved: false,
        });
        assert!(decision.is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn process_and_upload_tools_require_human_approval() {
        let root = fixture();
        let policy = AuthorityEnforcer::load(&root, ["src".to_string()]).unwrap();
        assert!(policy
            .authorize_tool("shell", "run_command", &json!({ "command": "echo hi" }))
            .is_err());
        assert!(policy
            .authorize_tool("http", "upload", &json!({ "path": "src/a.rs" }))
            .is_err());
        assert!(policy
            .authorize_human_tool("shell", "run_command", &json!({ "command": "echo hi" }))
            .is_ok());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn blocks_node_modules_via_sensitive_policy() {
        let root = fixture();
        let policy = AuthorityEnforcer::load(&root, [".".to_string()]).unwrap();
        assert!(policy
            .authorize_tool(
                "fs",
                "read_file",
                &json!({ "path": "node_modules/leftpad/index.js" })
            )
            .is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
