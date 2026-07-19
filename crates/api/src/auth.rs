use std::collections::HashSet;

/// Capability scopes for the local ADE HTTP API.
///
/// EXECUTE and unbounded lease acquire are intentionally absent — those stay
/// CLI/MCP-only so the loopback API cannot become a remote write engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiScope {
    Read,
    TasksWrite,
    LeasesWrite,
}

impl ApiScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::TasksWrite => "tasks:write",
            Self::LeasesWrite => "leases:write",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "read" => Some(Self::Read),
            "tasks:write" => Some(Self::TasksWrite),
            "leases:write" => Some(Self::LeasesWrite),
            _ => None,
        }
    }

    /// Default scopes when `ADE_API_TOKEN` is set and `ADE_API_SCOPES` is omitted.
    pub fn coordination_defaults() -> HashSet<Self> {
        HashSet::from([Self::Read, Self::TasksWrite, Self::LeasesWrite])
    }
}

/// Parse a comma-separated `ADE_API_SCOPES` value.
pub fn parse_scopes(raw: &str) -> Result<HashSet<ApiScope>, String> {
    let mut scopes = HashSet::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some(scope) = ApiScope::parse(part) else {
            return Err(format!(
                "unknown ADE_API_SCOPES entry '{part}' (expected read, tasks:write, leases:write)"
            ));
        };
        scopes.insert(scope);
    }
    if scopes.is_empty() {
        return Err("ADE_API_SCOPES must list at least one scope".into());
    }
    Ok(scopes)
}

/// Resolve token + scopes from environment (`ADE_API_TOKEN`, optional `ADE_API_SCOPES`).
pub fn auth_from_env() -> Result<(Option<String>, HashSet<ApiScope>), String> {
    let token = std::env::var("ADE_API_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty());
    let scopes = match std::env::var("ADE_API_SCOPES") {
        Ok(raw) if !raw.trim().is_empty() => parse_scopes(&raw)?,
        _ if token.is_some() => ApiScope::coordination_defaults(),
        _ => HashSet::new(),
    };
    Ok((token, scopes))
}

pub struct AuthHandler;

impl Default for AuthHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthHandler {
    pub fn new() -> Self {
        Self
    }

    /// Placeholder for future JWT/session validation. Local API v1 uses the
    /// shared bearer token + scopes on `ApiState` instead.
    pub fn validate_session(&self, _token: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_scopes() {
        let scopes = parse_scopes("read, tasks:write").unwrap();
        assert!(scopes.contains(&ApiScope::Read));
        assert!(scopes.contains(&ApiScope::TasksWrite));
        assert!(!scopes.contains(&ApiScope::LeasesWrite));
    }

    #[test]
    fn rejects_unknown_scope() {
        assert!(parse_scopes("execute").is_err());
    }
}
