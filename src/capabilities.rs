use crate::{config::RemoteConfig, protocol::PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerCapabilities {
    pub protocol_version: String,
    pub os: String,
    pub architecture: String,
    pub runtimes: Vec<String>,
    pub local_tools: Vec<String>,
    pub mcp_host_modes: Vec<String>,
    pub local_llm_endpoints: Vec<String>,
    pub network_zone: String,
    pub labels: Vec<String>,
    pub max_concurrency: u16,
    pub allowed_working_groups: Vec<String>,
}

impl RunnerCapabilities {
    pub fn discover(config: &RemoteConfig) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_string(),
            os: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            runtimes: discover_runtimes(),
            local_tools: config.capability_overrides.local_tools.clone(),
            mcp_host_modes: config.capability_overrides.mcp_host_modes.clone(),
            local_llm_endpoints: config.capability_overrides.local_llm_endpoints.clone(),
            network_zone: config
                .capability_overrides
                .network_zone
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            labels: config.labels.clone(),
            max_concurrency: config.max_concurrency,
            allowed_working_groups: config.allowed_working_groups.clone(),
        }
    }
}

fn discover_runtimes() -> Vec<String> {
    ["rustc", "cargo", "node", "python3"]
        .into_iter()
        .filter(|binary| command_exists(binary))
        .map(str::to_string)
        .collect()
}

fn command_exists(binary: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CapabilityOverrides, RemoteConfig};

    #[test]
    fn serializes_capabilities_with_protocol_version() {
        let config = RemoteConfig {
            control_plane_url: "https://taskotter.local".into(),
            runner_id: Some("runner-1".into()),
            registration_token_env: "TASKOTTER_REMOTE_REGISTRATION_TOKEN".into(),
            credential_placeholder: None,
            labels: vec!["gpu".into()],
            max_concurrency: 2,
            allowed_working_groups: vec!["wg-1".into()],
            env_allowlist: vec![],
            risky_action_allowlist: vec![],
            heartbeat_interval_seconds: 30,
            job_timeout_seconds: 60,
            capability_overrides: CapabilityOverrides {
                network_zone: Some("private".into()),
                local_llm_endpoints: vec!["http://localhost:11434".into()],
                local_tools: vec!["git".into()],
                mcp_host_modes: vec!["stdio".into()],
            },
        };

        let capabilities = RunnerCapabilities::discover(&config);
        let json = serde_json::to_string(&capabilities).expect("capabilities serialize");

        assert!(json.contains("remote.v1alpha1"));
        assert_eq!(capabilities.network_zone, "private");
        assert_eq!(capabilities.max_concurrency, 2);
    }
}
