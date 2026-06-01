use crate::{
    config::{
        AllowlistedCommandAdapterConfig, EndpointHealthStatus, ExternalAgentAdapterConfig,
        LocalLlmEndpointConfig, RemoteConfig,
    },
    gates::HighRiskCapability,
    protocol::PROTOCOL_VERSION,
};
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
    pub local_llm_endpoints: Vec<LocalLlmEndpointCapability>,
    pub external_agent_adapters: Vec<ExternalAgentAdapterCapability>,
    pub allowlisted_command_adapters: Vec<AllowlistedCommandAdapterCapability>,
    pub network_zone: String,
    pub labels: Vec<String>,
    pub max_concurrency: u16,
    pub allowed_working_groups: Vec<String>,
    pub high_risk_capabilities: Vec<RuntimeCapabilityGate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeCapabilityGate {
    pub capability: HighRiskCapability,
    pub feature_flag: String,
    pub enabled: bool,
    pub default_policy_effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalLlmEndpointCapability {
    pub id: String,
    pub provider: String,
    pub base_url: String,
    pub model_ids: Vec<String>,
    pub health_status: EndpointHealthStatus,
    pub usage_event_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalAgentAdapterCapability {
    pub id: String,
    pub runtime: String,
    pub command_label: String,
    pub can_mutate_workspace: bool,
    pub supported_isolation_modes: Vec<String>,
    pub supported_network_modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AllowlistedCommandAdapterCapability {
    pub id: String,
    pub command_label: String,
    pub allowed_args: Vec<String>,
    pub allowed_env: Vec<String>,
    pub can_spawn_process: bool,
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
            local_llm_endpoints: config
                .capability_overrides
                .local_llm_endpoints
                .iter()
                .map(LocalLlmEndpointCapability::from)
                .collect(),
            external_agent_adapters: config
                .capability_overrides
                .external_agent_adapters
                .iter()
                .map(ExternalAgentAdapterCapability::from)
                .collect(),
            allowlisted_command_adapters: config
                .capability_overrides
                .allowlisted_command_adapters
                .iter()
                .map(AllowlistedCommandAdapterCapability::from)
                .collect(),
            network_zone: config
                .capability_overrides
                .network_zone
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            labels: config.labels.clone(),
            max_concurrency: config.max_concurrency,
            allowed_working_groups: config.allowed_working_groups.clone(),
            high_risk_capabilities: vec![
                RuntimeCapabilityGate {
                    capability: HighRiskCapability::LocalToolExecution,
                    feature_flag: "remote.local_tools.enabled".to_string(),
                    enabled: config.feature_flags.local_tools_enabled,
                    default_policy_effect: "deny".to_string(),
                },
                RuntimeCapabilityGate {
                    capability: HighRiskCapability::LocalLlmExposure,
                    feature_flag: "remote.local_llm.enabled".to_string(),
                    enabled: config.feature_flags.local_llm_enabled,
                    default_policy_effect: "deny".to_string(),
                },
                RuntimeCapabilityGate {
                    capability: HighRiskCapability::ExternalAgentRuntimeAdapter,
                    feature_flag: "remote.external_agent_adapters.enabled".to_string(),
                    enabled: config.feature_flags.external_agent_adapters_enabled,
                    default_policy_effect: "deny".to_string(),
                },
                RuntimeCapabilityGate {
                    capability: HighRiskCapability::ComputerUseExecution,
                    feature_flag: "remote.computer_use.enabled".to_string(),
                    enabled: config.feature_flags.computer_use_enabled,
                    default_policy_effect: "deny".to_string(),
                },
            ],
        }
    }
}

impl From<&LocalLlmEndpointConfig> for LocalLlmEndpointCapability {
    fn from(config: &LocalLlmEndpointConfig) -> Self {
        Self {
            id: config.id.clone(),
            provider: config.provider.clone(),
            base_url: config.base_url.clone(),
            model_ids: config.model_ids.clone(),
            health_status: config.health_status,
            usage_event_required: true,
        }
    }
}

impl From<&ExternalAgentAdapterConfig> for ExternalAgentAdapterCapability {
    fn from(config: &ExternalAgentAdapterConfig) -> Self {
        Self {
            id: config.id.clone(),
            runtime: config.runtime.clone(),
            command_label: config.command_label.clone(),
            can_mutate_workspace: true,
            supported_isolation_modes: config.supported_isolation_modes.clone(),
            supported_network_modes: config.supported_network_modes.clone(),
        }
    }
}

impl From<&AllowlistedCommandAdapterConfig> for AllowlistedCommandAdapterCapability {
    fn from(config: &AllowlistedCommandAdapterConfig) -> Self {
        Self {
            id: config.id.clone(),
            command_label: config.command_label.clone(),
            allowed_args: config.allowed_args.clone(),
            allowed_env: config.allowed_env.clone(),
            can_spawn_process: true,
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
    use crate::config::{
        AllowlistedCommandAdapterConfig, CapabilityOverrides, EndpointHealthStatus,
        ExternalAgentAdapterConfig, LocalLlmEndpointConfig, RemoteConfig, RuntimeFeatureFlags,
    };

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
                local_llm_endpoints: vec![LocalLlmEndpointConfig {
                    id: "ollama-dev".into(),
                    provider: "ollama".into(),
                    base_url: "http://localhost:11434".into(),
                    model_ids: vec!["llama3.2".into()],
                    health_status: EndpointHealthStatus::Healthy,
                }],
                external_agent_adapters: vec![ExternalAgentAdapterConfig {
                    id: "codex-local".into(),
                    runtime: "codex".into(),
                    command_label: "codex".into(),
                    supported_isolation_modes: vec!["temporary_isolated".into()],
                    supported_network_modes: vec!["deny_all".into()],
                }],
                allowlisted_command_adapters: vec![AllowlistedCommandAdapterConfig {
                    id: "cargo-test".into(),
                    command_label: "cargo".into(),
                    allowed_args: vec!["test".into()],
                    allowed_env: vec!["PATH".into()],
                }],
                local_tools: vec!["git".into()],
                mcp_host_modes: vec!["stdio".into()],
            },
            feature_flags: RuntimeFeatureFlags {
                local_tools_enabled: true,
                local_llm_enabled: false,
                external_agent_adapters_enabled: false,
                computer_use_enabled: false,
            },
        };

        let capabilities = RunnerCapabilities::discover(&config);
        let json = serde_json::to_string(&capabilities).expect("capabilities serialize");

        assert!(json.contains("remote.v1alpha1"));
        assert_eq!(capabilities.network_zone, "private");
        assert_eq!(capabilities.max_concurrency, 2);
        assert_eq!(capabilities.local_llm_endpoints[0].provider, "ollama");
        assert!(capabilities.local_llm_endpoints[0].usage_event_required);
        assert!(capabilities.external_agent_adapters[0].can_mutate_workspace);
        assert!(capabilities.allowlisted_command_adapters[0].can_spawn_process);
        assert!(
            capabilities
                .high_risk_capabilities
                .iter()
                .any(
                    |gate| gate.capability == HighRiskCapability::LocalToolExecution
                        && gate.enabled
                        && gate.default_policy_effect == "deny"
                )
        );
    }

    #[test]
    fn high_risk_capabilities_are_disabled_by_default() {
        let config = RemoteConfig::from_toml_str(
            r#"
control_plane_url = "https://taskotter.local"
registration_token_env = "TASKOTTER_REMOTE_REGISTRATION_TOKEN"
"#,
        )
        .expect("config should parse");

        let capabilities = RunnerCapabilities::discover(&config);

        assert!(
            capabilities
                .high_risk_capabilities
                .iter()
                .all(|gate| !gate.enabled && gate.default_policy_effect == "deny")
        );
    }
}
