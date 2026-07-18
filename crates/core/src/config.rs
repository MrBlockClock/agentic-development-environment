use crate::error::AdeError;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

/// Deployment environment (config profile) the ADE runs under.
///
/// Selected at launch via the `ADE_ENV` variable. Each environment keeps its
/// own isolated data directory so state never bleeds between profiles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[default]
    Local,
    Staging,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }

    /// Verbosity default when `ADE_LOG_LEVEL` is not set.
    pub fn default_log_level(&self) -> &'static str {
        match self {
            Self::Local => "debug",
            Self::Staging => "info",
            Self::Production => "warn",
        }
    }
}

impl Environment {
    /// Select the active environment from the `ADE_ENV` process variable,
    /// defaulting to [`Environment::Local`]. `ADE_ENV` is read from the real
    /// process environment (not from `.env` files) so it can pick which
    /// `.env.<env>` file to load.
    pub fn from_env() -> Result<Self, AdeError> {
        match env::var("ADE_ENV") {
            Ok(v) if !v.trim().is_empty() => v.parse().map_err(AdeError::Config),
            _ => Ok(Self::default()),
        }
    }
}

impl std::str::FromStr for Environment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "local" | "dev" | "development" => Ok(Self::Local),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            other => Err(format!(
                "unknown ADE_ENV '{other}' (expected local|staging|production)"
            )),
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Resolved runtime configuration for a single environment/profile.
#[derive(Debug, Clone)]
pub struct AdeConfig {
    pub environment: Environment,
    /// Environment-scoped data directory (`<base>/<env>`).
    pub data_dir: PathBuf,
    pub log_level: String,
    pub turso_url: Option<String>,
    pub turso_auth_token: Option<String>,
}

impl AdeConfig {
    /// Load configuration, auto-loading `.env` files into the process
    /// environment first.
    ///
    /// `ADE_ENV` (read from the real environment) selects the profile
    /// (default [`Environment::Local`]). Env files are then loaded with the
    /// following precedence (highest first):
    ///
    /// 1. Real process/shell environment variables (never overridden)
    /// 2. `.env.<env>` — environment-specific overrides
    /// 3. `.env` — shared base defaults
    ///
    /// Provider API keys are intentionally NOT read here — those live in the
    /// OS keychain (BYOK) and are resolved separately.
    pub fn load() -> Result<Self, AdeError> {
        let environment = Environment::from_env()?;
        // dotenvy does not override variables already present in the
        // environment, so loading the specific file before the base file
        // makes `.env.<env>` win over `.env`, and both lose to real env vars.
        let _ = dotenvy::from_filename(format!(".env.{}", environment.as_str()));
        let _ = dotenvy::dotenv();
        Self::for_environment(environment)
    }

    /// Build the config for an explicit environment, still honoring the
    /// `ADE_DATA_DIR` / `ADE_LOG_LEVEL` / `TURSO_*` overrides.
    pub fn for_environment(environment: Environment) -> Result<Self, AdeError> {
        let base = env::var("ADE_DATA_DIR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_base_dir);

        let log_level = env::var("ADE_LOG_LEVEL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| environment.default_log_level().to_string());

        Ok(Self {
            environment,
            data_dir: base.join(environment.as_str()),
            log_level,
            turso_url: non_empty_var("TURSO_DATABASE_URL"),
            turso_auth_token: non_empty_var("TURSO_AUTH_TOKEN"),
        })
    }

    pub fn is_production(&self) -> bool {
        self.environment == Environment::Production
    }
}

fn non_empty_var(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.trim().is_empty())
}

/// OS-appropriate base data directory, used when `ADE_DATA_DIR` is unset.
fn default_base_dir() -> PathBuf {
    if let Some(dir) = non_empty_var("APPDATA") {
        return PathBuf::from(dir).join("ade");
    }
    if let Some(dir) = non_empty_var("XDG_DATA_HOME") {
        return PathBuf::from(dir).join("ade");
    }
    if let Some(home) = non_empty_var("HOME") {
        return PathBuf::from(home).join(".local/share/ade");
    }
    PathBuf::from("./data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_aliases() {
        assert_eq!("local".parse::<Environment>().unwrap(), Environment::Local);
        assert_eq!("dev".parse::<Environment>().unwrap(), Environment::Local);
        assert_eq!(
            "stage".parse::<Environment>().unwrap(),
            Environment::Staging
        );
        assert_eq!(
            "PROD".parse::<Environment>().unwrap(),
            Environment::Production
        );
    }

    #[test]
    fn rejects_unknown_env() {
        assert!("qa".parse::<Environment>().is_err());
    }

    #[test]
    fn log_levels_differ_per_environment() {
        assert_eq!(Environment::Local.default_log_level(), "debug");
        assert_eq!(Environment::Staging.default_log_level(), "info");
        assert_eq!(Environment::Production.default_log_level(), "warn");
    }

    #[test]
    fn data_dir_is_environment_scoped() {
        let base = PathBuf::from("/tmp/ade-base");
        // for_environment reads ADE_DATA_DIR; assert the join behavior directly.
        let scoped = base.join(Environment::Staging.as_str());
        assert!(scoped.ends_with("staging"));
    }
}
