use ade_core::config::AdeConfig;
use ade_core::error::AdeError;
use std::path::PathBuf;
use turso::{Connection, Database};

pub struct DbConfig {
    pub data_dir: PathBuf,
    pub url: Option<String>,
    pub auth_token: Option<String>,
}

impl DbConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            url: None,
            auth_token: None,
        }
    }

    /// Derive DB settings from the active environment profile so each
    /// environment (local/staging/production) uses its own isolated data
    /// directory and Turso endpoint.
    pub fn from_ade_config(cfg: &AdeConfig) -> Self {
        Self {
            data_dir: cfg.data_dir.clone(),
            url: cfg.turso_url.clone(),
            auth_token: cfg.turso_auth_token.clone(),
        }
    }
}

/// Environment-scoped local Turso database.
///
/// Remote synchronization is intentionally deferred until the configured
/// Turso endpoint can be authenticated through the BYOK/keychain layer.
#[derive(Clone)]
pub struct AdeDatabase {
    database: Database,
    path: PathBuf,
}

impl AdeDatabase {
    pub async fn open(config: &DbConfig) -> Result<Self, AdeError> {
        std::fs::create_dir_all(&config.data_dir)?;
        Self::open_path(config.data_dir.join("ade.db")).await
    }

    pub async fn open_path(path: impl Into<PathBuf>) -> Result<Self, AdeError> {
        let path = path.into();
        let path_string = path.to_string_lossy().into_owned();
        let database = turso::Builder::new_local(&path_string)
            .build()
            .await
            .map_err(|error| AdeError::Database(error.to_string()))?;
        let instance = Self { database, path };
        let connection = instance.connect()?;
        crate::schema::migrate(&connection).await?;
        Ok(instance)
    }

    pub fn connect(&self) -> Result<Connection, AdeError> {
        self.database
            .connect()
            .map_err(|error| AdeError::Database(error.to_string()))
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}
