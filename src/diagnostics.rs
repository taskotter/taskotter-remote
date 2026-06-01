use crate::{
    capabilities::RunnerCapabilities,
    config::RemoteConfig,
    protocol::{ErrorCategory, PROTOCOL_VERSION, RunnerHealth},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DIAGNOSTICS_SCHEMA_VERSION: &str = "runner_operator_diagnostics.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorDiagnosticsSnapshot {
    pub schema_version: String,
    pub protocol_version: String,
    pub capability_inventory_version: String,
    pub daemon_version: String,
    pub runner_id: String,
    pub control_plane_url: String,
    pub registration_token_env_name: String,
    pub credential_ref: Option<String>,
    pub last_valid_instruction: Option<LastValidInstruction>,
    pub heartbeat_state: OperatorHeartbeatState,
    pub capability_inventory: RunnerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LastValidInstruction {
    pub instruction_id: String,
    pub message_type: String,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorHeartbeatState {
    pub health: RunnerHealth,
    pub active_job_ids: Vec<String>,
    pub available_concurrency: u16,
    pub resource_pressure: Vec<String>,
    pub last_error_category: Option<ErrorCategory>,
    pub quarantine_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsOptions {
    pub health: RunnerHealth,
    pub quarantine_reason: Option<String>,
    pub last_valid_instruction: Option<LastValidInstruction>,
}

impl Default for DiagnosticsOptions {
    fn default() -> Self {
        Self {
            health: RunnerHealth::Online,
            quarantine_reason: None,
            last_valid_instruction: Some(LastValidInstruction {
                instruction_id: "fixture:control-plane/job-dispatch-echo".to_string(),
                message_type: "job.dispatch".to_string(),
                protocol_version: PROTOCOL_VERSION.to_string(),
            }),
        }
    }
}

pub fn operator_diagnostics(config: &RemoteConfig, options: DiagnosticsOptions) -> Value {
    let capabilities = RunnerCapabilities::discover(config);
    let snapshot = OperatorDiagnosticsSnapshot {
        schema_version: DIAGNOSTICS_SCHEMA_VERSION.to_string(),
        protocol_version: PROTOCOL_VERSION.to_string(),
        capability_inventory_version: capabilities.protocol_version.clone(),
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        runner_id: config
            .runner_id
            .clone()
            .unwrap_or_else(|| "unregistered-local-runner".to_string()),
        control_plane_url: config.control_plane_url.clone(),
        registration_token_env_name: config.registration_token_env.clone(),
        credential_ref: config
            .credential_placeholder
            .as_ref()
            .map(|_| "[REDACTED:credential_ref]".to_string()),
        last_valid_instruction: options.last_valid_instruction,
        heartbeat_state: OperatorHeartbeatState {
            health: options.health,
            active_job_ids: vec![],
            available_concurrency: config.max_concurrency,
            resource_pressure: vec![],
            last_error_category: None,
            quarantine_reason: options.quarantine_reason,
        },
        capability_inventory: capabilities,
    };

    let mut value = serde_json::to_value(snapshot).expect("operator diagnostics should serialize");
    redact_sensitive_values(&mut value);
    value
}

fn redact_sensitive_values(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *child = Value::String(format!("[REDACTED:{key}]"));
                } else {
                    redact_sensitive_values(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_sensitive_values(item);
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "token"
            | "secret"
            | "password"
            | "passwd"
            | "api_key"
            | "apikey"
            | "private_key"
            | "credential"
            | "credentials"
            | "credential_ref"
            | "scoped_credential_ref"
    ) || normalized.ends_with("_token")
        || normalized.ends_with("_secret")
        || normalized.ends_with("_password")
        || normalized.ends_with("_api_key")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CapabilityOverrides, RemoteConfig, RuntimeFeatureFlags};

    fn config_with_sensitive_values() -> RemoteConfig {
        RemoteConfig {
            control_plane_url: "https://taskotter.local".into(),
            runner_id: Some("runner-1".into()),
            registration_token_env: "TASKOTTER_REMOTE_REGISTRATION_TOKEN".into(),
            credential_placeholder: Some("prod-credential-ref-should-not-render".into()),
            labels: vec!["local".into()],
            max_concurrency: 2,
            allowed_working_groups: vec!["wg-1".into()],
            env_allowlist: vec!["PATH".into()],
            risky_action_allowlist: vec![],
            heartbeat_interval_seconds: 30,
            job_timeout_seconds: 60,
            capability_overrides: CapabilityOverrides::default(),
            feature_flags: RuntimeFeatureFlags::default(),
        }
    }

    #[test]
    fn diagnostics_redacts_credential_material_but_keeps_env_name() {
        let diagnostics = operator_diagnostics(
            &config_with_sensitive_values(),
            DiagnosticsOptions::default(),
        );
        let rendered = serde_json::to_string(&diagnostics).expect("diagnostics render");

        assert!(rendered.contains("TASKOTTER_REMOTE_REGISTRATION_TOKEN"));
        assert!(rendered.contains("[REDACTED:credential_ref]"));
        assert!(!rendered.contains("prod-credential-ref-should-not-render"));
    }

    #[test]
    fn diagnostics_reports_non_online_health_states() {
        for health in [
            RunnerHealth::Offline,
            RunnerHealth::Degraded,
            RunnerHealth::Draining,
            RunnerHealth::Quarantined,
        ] {
            let diagnostics = operator_diagnostics(
                &config_with_sensitive_values(),
                DiagnosticsOptions {
                    health: health.clone(),
                    quarantine_reason: (health == RunnerHealth::Quarantined)
                        .then(|| "policy_hold".to_string()),
                    ..DiagnosticsOptions::default()
                },
            );

            assert_eq!(
                diagnostics["heartbeat_state"]["health"],
                serde_json::to_value(health).expect("health should serialize")
            );
        }
    }

    #[test]
    fn diagnostics_fixtures_cover_required_health_states() {
        for (fixture, expected) in [
            (
                include_str!("../fixtures/runner/operator-diagnostics-offline.json"),
                RunnerHealth::Offline,
            ),
            (
                include_str!("../fixtures/runner/operator-diagnostics-degraded.json"),
                RunnerHealth::Degraded,
            ),
            (
                include_str!("../fixtures/runner/operator-diagnostics-draining.json"),
                RunnerHealth::Draining,
            ),
            (
                include_str!("../fixtures/runner/operator-diagnostics-quarantined.json"),
                RunnerHealth::Quarantined,
            ),
        ] {
            let parsed: Value = serde_json::from_str(fixture).expect("fixture should parse");
            assert_eq!(
                parsed["heartbeat_state"]["health"],
                serde_json::to_value(expected).expect("health should serialize")
            );
        }
    }
}
