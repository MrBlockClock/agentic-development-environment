use ade_core::config::AdeConfig;
use std::path::PathBuf;

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
