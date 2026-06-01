use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteConfig {
    pub control_plane_url: String,
    pub runner_id: Option<String>,
    pub registration_token_env: String,
    pub credential_placeholder: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u16,
    #[serde(default)]
    pub allowed_working_groups: Vec<String>,
    #[serde(default)]
    pub env_allowlist: Vec<String>,
    #[serde(default)]
    pub risky_action_allowlist: Vec<String>,
    #[serde(default = "default_heartbeat_interval_seconds")]
    pub heartbeat_interval_seconds: u64,
    #[serde(default = "default_job_timeout_seconds")]
    pub job_timeout_seconds: u64,
    #[serde(default)]
    pub capability_overrides: CapabilityOverrides,
    #[serde(default)]
    pub feature_flags: RuntimeFeatureFlags,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityOverrides {
    pub network_zone: Option<String>,
    #[serde(default)]
    pub local_llm_endpoints: Vec<LocalLlmEndpointConfig>,
    #[serde(default)]
    pub external_agent_adapters: Vec<ExternalAgentAdapterConfig>,
    #[serde(default)]
    pub allowlisted_command_adapters: Vec<AllowlistedCommandAdapterConfig>,
    #[serde(default)]
    pub local_tools: Vec<String>,
    #[serde(default)]
    pub mcp_host_modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalLlmEndpointConfig {
    pub id: String,
    pub provider: String,
    pub base_url: String,
    #[serde(default)]
    pub model_ids: Vec<String>,
    #[serde(default)]
    pub health_status: EndpointHealthStatus,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EndpointHealthStatus {
    Healthy,
    Degraded,
    Unreachable,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalAgentAdapterConfig {
    pub id: String,
    pub runtime: String,
    pub command_label: String,
    #[serde(default)]
    pub supported_isolation_modes: Vec<String>,
    #[serde(default)]
    pub supported_network_modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AllowlistedCommandAdapterConfig {
    pub id: String,
    pub command_label: String,
    #[serde(default)]
    pub allowed_args: Vec<String>,
    #[serde(default)]
    pub allowed_env: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeFeatureFlags {
    #[serde(default)]
    pub local_tools_enabled: bool,
    #[serde(default)]
    pub local_llm_enabled: bool,
    #[serde(default)]
    pub external_agent_adapters_enabled: bool,
    #[serde(default)]
    pub computer_use_enabled: bool,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse TOML config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("control_plane_url must not be empty")]
    EmptyControlPlaneUrl,
    #[error("registration_token_env must name an environment variable")]
    EmptyRegistrationTokenEnv,
    #[error("max_concurrency must be greater than zero")]
    EmptyConcurrency,
}

impl RemoteConfig {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path)?;
        Self::from_toml_str(&raw)
    }

    pub fn from_toml_str(raw: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(raw)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.control_plane_url.trim().is_empty() {
            return Err(ConfigError::EmptyControlPlaneUrl);
        }
        if self.registration_token_env.trim().is_empty() {
            return Err(ConfigError::EmptyRegistrationTokenEnv);
        }
        if self.max_concurrency == 0 {
            return Err(ConfigError::EmptyConcurrency);
        }
        Ok(())
    }
}

fn default_max_concurrency() -> u16 {
    1
}

fn default_heartbeat_interval_seconds() -> u64 {
    30
}

fn default_job_timeout_seconds() -> u64 {
    1800
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config_with_defaults() {
        let config = RemoteConfig::from_toml_str(
            r#"
control_plane_url = "https://taskotter.local"
registration_token_env = "TASKOTTER_REMOTE_REGISTRATION_TOKEN"
"#,
        )
        .expect("config should parse");

        assert_eq!(config.max_concurrency, 1);
        assert_eq!(config.heartbeat_interval_seconds, 30);
        assert_eq!(config.job_timeout_seconds, 1800);
        assert!(config.runner_id.is_none());
        assert_eq!(config.feature_flags, RuntimeFeatureFlags::default());
    }

    #[test]
    fn rejects_empty_control_plane_url() {
        let error = RemoteConfig::from_toml_str(
            r#"
control_plane_url = ""
registration_token_env = "TASKOTTER_REMOTE_REGISTRATION_TOKEN"
"#,
        )
        .expect_err("config should reject an empty URL");

        assert!(matches!(error, ConfigError::EmptyControlPlaneUrl));
    }
}
