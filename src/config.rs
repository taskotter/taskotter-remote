use std::{env, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub runner_name: String,
    pub working_group_id: Uuid,
    pub max_concurrency: u16,
    pub control_plane: ControlPlaneConfig,
    pub capabilities: CapabilityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlPlaneConfig {
    pub endpoint: String,
    pub registration_token_env: String,
    pub transport: Transport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    GrpcStream,
    HttpsPolling,
    WebSocket,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityConfig {
    pub runtimes: Vec<String>,
    pub tools: Vec<String>,
    pub mcp_host_modes: Vec<String>,
    pub local_llm_endpoints: Vec<String>,
    pub network_zone: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse config {path}: {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
    #[error("runner_name must not be empty")]
    EmptyRunnerName,
    #[error("control_plane.endpoint must not be empty")]
    EmptyEndpoint,
    #[error("max_concurrency must be greater than zero")]
    EmptyConcurrency,
    #[error("registration token env var name must not be empty")]
    EmptyTokenEnv,
}

impl Config {
    pub async fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        let path_display = path_ref.display().to_string();
        let raw = tokio::fs::read_to_string(path_ref)
            .await
            .map_err(|source| ConfigError::Read {
                path: path_display.clone(),
                source,
            })?;
        let config = toml::from_str::<Self>(&raw).map_err(|source| ConfigError::Parse {
            path: path_display,
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.runner_name.trim().is_empty() {
            return Err(ConfigError::EmptyRunnerName);
        }
        if self.control_plane.endpoint.trim().is_empty() {
            return Err(ConfigError::EmptyEndpoint);
        }
        if self.max_concurrency == 0 {
            return Err(ConfigError::EmptyConcurrency);
        }
        if self.control_plane.registration_token_env.trim().is_empty() {
            return Err(ConfigError::EmptyTokenEnv);
        }
        Ok(())
    }

    pub fn registration_token_is_available(&self) -> bool {
        env::var_os(&self.control_plane.registration_token_env).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parses_example_config() {
        let config = Config::from_path("examples/runner.toml").await.unwrap();

        assert_eq!(config.runner_name, "local-dev-runner");
        assert_eq!(config.control_plane.transport, Transport::GrpcStream);
        assert_eq!(config.max_concurrency, 1);
    }
}
