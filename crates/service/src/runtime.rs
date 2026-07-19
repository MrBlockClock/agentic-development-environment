pub use ade_api::auth::ApiScope;

use ade_api::auth;
use ade_api::router::ApiState;
use ade_core::error::AdeError;
use std::collections::HashSet;
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub workspace_root: PathBuf,
    pub bind: SocketAddr,
    pub auth_token: Option<String>,
    pub auth_scopes: HashSet<ApiScope>,
}

impl ServiceConfig {
    pub fn local(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            bind: SocketAddr::from(([127, 0, 0, 1], 3210)),
            auth_token: None,
            auth_scopes: HashSet::new(),
        }
    }

    /// Resolve bearer token + scopes from `ADE_API_TOKEN` / `ADE_API_SCOPES`.
    pub fn auth_from_env() -> Result<(Option<String>, HashSet<ApiScope>), AdeError> {
        auth::auth_from_env().map_err(AdeError::Config)
    }

    pub fn validate(&self) -> Result<(), AdeError> {
        if !self.bind.ip().is_loopback() {
            return Err(AdeError::Authorization(format!(
                "ADE service refuses non-loopback bind {}; use a policy-enforcing reverse proxy",
                self.bind
            )));
        }
        if !self.workspace_root.is_dir() {
            return Err(AdeError::Config(format!(
                "workspace root does not exist: {}",
                self.workspace_root.display()
            )));
        }
        Ok(())
    }
}

pub struct BoundService {
    listener: TcpListener,
    state: ApiState,
    local_addr: SocketAddr,
}

impl BoundService {
    pub async fn bind(config: ServiceConfig) -> Result<Self, AdeError> {
        config.validate()?;
        let listener = TcpListener::bind(config.bind)
            .await
            .map_err(|error| AdeError::Other(format!("failed to bind ADE API: {error}")))?;
        let local_addr = listener.local_addr().map_err(|error| {
            AdeError::Other(format!("failed to inspect ADE API socket: {error}"))
        })?;
        let mut state = ApiState::new(config.workspace_root);
        if let Some(token) = config.auth_token {
            state = state.with_auth_token(token);
        }
        if !config.auth_scopes.is_empty() {
            state = state.with_scopes(config.auth_scopes);
        }
        Ok(Self {
            listener,
            state,
            local_addr,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn auth_required(&self) -> bool {
        self.state.auth_token().is_some()
    }

    pub async fn serve<F>(self, shutdown: F) -> Result<(), AdeError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        ade_api::router::serve(self.listener, self.state, shutdown)
            .await
            .map_err(|error| AdeError::Other(format!("ADE API server failed: {error}")))
    }
}

pub async fn run_until_signal(config: ServiceConfig) -> Result<(), AdeError> {
    BoundService::bind(config)
        .await?
        .serve(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn binds_ephemeral_loopback_and_tracks_auth_mode() {
        let root = std::env::temp_dir().join(format!("ade-service-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let service = BoundService::bind(ServiceConfig {
            workspace_root: root.clone(),
            bind: SocketAddr::from(([127, 0, 0, 1], 0)),
            auth_token: Some("test-token".into()),
            auth_scopes: ApiScope::coordination_defaults(),
        })
        .await
        .unwrap();
        assert!(service.local_addr().ip().is_loopback());
        assert_ne!(service.local_addr().port(), 0);
        assert!(service.auth_required());
        drop(service);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_non_loopback_bind() {
        let config = ServiceConfig {
            workspace_root: PathBuf::from("."),
            bind: SocketAddr::from(([0, 0, 0, 0], 3210)),
            auth_token: None,
            auth_scopes: HashSet::new(),
        };
        assert!(config.validate().is_err());
    }
}
