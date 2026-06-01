use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "remote.v1alpha1";
pub const USAGE_SCHEMA_VERSION: &str = "remote_usage_report.v1";
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[PROTOCOL_VERSION];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolCompatibilityError {
    #[error("unsupported remote protocol version: {0}")]
    UnsupportedProtocolVersion(String),
}

pub fn validate_protocol_version(version: &str) -> Result<(), ProtocolCompatibilityError> {
    if SUPPORTED_PROTOCOL_VERSIONS.contains(&version) {
        Ok(())
    } else {
        Err(ProtocolCompatibilityError::UnsupportedProtocolVersion(
            version.to_string(),
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolEnvelope<T> {
    pub protocol_version: String,
    pub message_type: String,
    pub payload: T,
}

impl<T> ProtocolEnvelope<T> {
    pub fn new(message_type: impl Into<String>, payload: T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_string(),
            message_type: message_type.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrationRequest {
    pub runner_id: Option<String>,
    pub display_name: String,
    pub requested_working_groups: Vec<String>,
    pub registration_token_env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerHealth {
    Online,
    Degraded,
    Draining,
    Offline,
    Disabled,
    Quarantined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Heartbeat {
    pub runner_id: String,
    pub capability_inventory_version: String,
    pub daemon_version: String,
    pub health: RunnerHealth,
    pub active_job_ids: Vec<String>,
    pub available_concurrency: u16,
    pub clock_skew_ms: i64,
    pub resource_pressure: Vec<String>,
    pub last_error_category: Option<ErrorCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspacePolicy {
    pub workspace_id: String,
    pub root_strategy: String,
    pub allowed_env: Vec<String>,
    pub scoped_credential_ref: Option<String>,
    pub artifact_path_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkPolicy {
    pub mode: String,
    pub allowed_targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySnapshot {
    pub policy_version: String,
    pub working_group_id: String,
    pub allowed_capabilities: Vec<String>,
    pub risky_actions: Vec<String>,
    pub workspace: WorkspacePolicy,
    pub network: NetworkPolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    Noop,
    Echo,
    OpenAiCompatibleLocalLlm,
    OllamaLocalLlm,
    SelfHostedLocalLlm,
    CodexAgent,
    ClaudeCodeAgent,
    AllowlistedCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobDispatch {
    pub lease_id: String,
    pub job_id: String,
    pub runner_id: String,
    pub correlation_id: String,
    pub idempotency_key: String,
    pub adapter: AdapterKind,
    pub input: serde_json::Value,
    pub timeout_seconds: u64,
    pub policy: PolicySnapshot,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobLifecyclePhase {
    Queued,
    Leased,
    Preparing,
    Running,
    Finishing,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
    PolicyDenied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterExecutionMetadata {
    pub adapter_id: String,
    pub adapter_kind: AdapterKind,
    pub isolation_mode: String,
    pub network_mode: String,
    pub scoped_credential_refs: Vec<String>,
    pub audit_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterResult {
    pub job_id: String,
    pub runner_id: String,
    pub phase: JobLifecyclePhase,
    pub exit_code: Option<i32>,
    pub usage: UsageReport,
    pub artifacts: Vec<ArtifactDescriptor>,
    pub audit_metadata: AdapterExecutionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobLeaseAcceptance {
    pub lease_id: String,
    pub job_id: String,
    pub runner_id: String,
    pub accepted: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancellationRequest {
    pub lease_id: String,
    pub job_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgressUpdate {
    pub job_id: String,
    pub runner_id: String,
    pub sequence: u64,
    pub phase: String,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogChunk {
    pub job_id: String,
    pub runner_id: String,
    pub sequence: u64,
    pub stream: String,
    pub redacted: bool,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactManifest {
    pub job_id: String,
    pub runner_id: String,
    pub artifacts: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactDescriptor {
    pub path_label: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub checksum_sha256: String,
    pub retention_policy: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    PolicyDenied,
    CapabilityMismatch,
    WorkspaceSetup,
    AdapterFailed,
    Timeout,
    Cancelled,
    ArtifactUpload,
    UsageReport,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerError {
    pub job_id: Option<String>,
    pub runner_id: String,
    pub category: ErrorCategory,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageAttemptStatus {
    Succeeded,
    Denied,
    Timeout,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageReport {
    pub schema_version: String,
    pub job_id: String,
    pub runner_id: String,
    pub request_id: Option<Uuid>,
    pub correlation_id: Option<String>,
    pub status: UsageAttemptStatus,
    pub duration_ms: u64,
    pub cpu_time_ms: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub estimated_cost_micro_usd: u64,
}

#[cfg(test)]
mod tests {
    use crate::job::{JobOutcome, JobState};
    use serde_json::json;

    use super::*;

    #[test]
    fn serializes_remote_usage_report_contract() {
        let report = UsageReport {
            schema_version: USAGE_SCHEMA_VERSION.to_string(),
            job_id: "job_1".to_string(),
            runner_id: "runner_1".to_string(),
            request_id: None,
            correlation_id: Some("corr_1".to_string()),
            status: UsageAttemptStatus::Succeeded,
            duration_ms: 10,
            cpu_time_ms: None,
            peak_memory_bytes: None,
            prompt_tokens: 0,
            completion_tokens: 0,
            estimated_cost_micro_usd: 0,
        };

        let value = serde_json::to_value(report).unwrap();

        assert_eq!(value["schema_version"], "remote_usage_report.v1");
        assert_eq!(value["status"], "succeeded");
        assert_eq!(value["correlation_id"], json!("corr_1"));
    }

    #[test]
    fn deserializes_control_plane_job_dispatch_fixture() {
        let fixture = include_str!("../fixtures/control-plane/job-dispatch-echo.json");
        let envelope: ProtocolEnvelope<JobDispatch> =
            serde_json::from_str(fixture).expect("fixture should match job dispatch contract");

        validate_protocol_version(&envelope.protocol_version).unwrap();
        assert_eq!(envelope.protocol_version, PROTOCOL_VERSION);
        assert_eq!(envelope.message_type, "job.dispatch");
        assert_eq!(envelope.payload.adapter, AdapterKind::Echo);
        assert_eq!(envelope.payload.policy.network.mode, "deny_all");
        assert!(
            envelope
                .payload
                .policy
                .workspace
                .allowed_env
                .contains(&"PATH".to_string())
        );
    }

    #[test]
    fn deserializes_noop_job_dispatch_fixture() {
        let fixture = include_str!("../fixtures/control-plane/job-dispatch-noop.json");
        let envelope: ProtocolEnvelope<JobDispatch> =
            serde_json::from_str(fixture).expect("noop fixture should match job dispatch contract");

        validate_protocol_version(&envelope.protocol_version).unwrap();
        assert_eq!(envelope.message_type, "job.dispatch");
        assert_eq!(envelope.payload.adapter, AdapterKind::Noop);
        assert_eq!(envelope.payload.policy.network.mode, "deny_all");
        assert!(envelope.payload.policy.risky_actions.is_empty());
    }

    #[test]
    fn deserializes_cancellation_and_error_taxonomy_fixtures() {
        let cancel: ProtocolEnvelope<CancellationRequest> =
            serde_json::from_str(include_str!("../fixtures/control-plane/job-cancel.json"))
                .expect("cancel fixture should match cancellation contract");
        let error: ProtocolEnvelope<RunnerError> =
            serde_json::from_str(include_str!("../fixtures/runner/job-error-timeout.json"))
                .expect("error fixture should match runner error contract");
        let policy_denied: ProtocolEnvelope<RunnerError> = serde_json::from_str(include_str!(
            "../fixtures/runner/job-error-policy-denied.json"
        ))
        .expect("policy denied fixture should match runner error contract");

        assert_eq!(cancel.message_type, "job.cancel");
        assert_eq!(cancel.payload.reason, "operator_requested");
        assert_eq!(error.message_type, "job.error");
        assert_eq!(error.payload.category, ErrorCategory::Timeout);
        assert!(error.payload.retryable);
        assert_eq!(policy_denied.payload.category, ErrorCategory::PolicyDenied);
        assert!(!policy_denied.payload.retryable);
    }

    #[test]
    fn deserializes_noop_lifecycle_evidence_fixtures() {
        for (fixture, expected_status, expected_state) in [
            (
                include_str!("../fixtures/runner/noop-lifecycle-success.json"),
                UsageAttemptStatus::Succeeded,
                JobState::Succeeded,
            ),
            (
                include_str!("../fixtures/runner/noop-lifecycle-failure.json"),
                UsageAttemptStatus::Failed,
                JobState::Failed,
            ),
            (
                include_str!("../fixtures/runner/noop-lifecycle-cancelled.json"),
                UsageAttemptStatus::Cancelled,
                JobState::Cancelled,
            ),
            (
                include_str!("../fixtures/runner/noop-lifecycle-timeout.json"),
                UsageAttemptStatus::Timeout,
                JobState::TimedOut,
            ),
        ] {
            let envelopes: Vec<ProtocolEnvelope<serde_json::Value>> =
                serde_json::from_str(fixture).expect("lifecycle fixture should parse");
            let message_types: Vec<&str> = envelopes
                .iter()
                .map(|envelope| envelope.message_type.as_str())
                .collect();

            assert_eq!(
                message_types,
                vec![
                    "job.log.chunk",
                    "job.artifact.manifest",
                    "job.usage.report",
                    "job.final_result"
                ]
            );
            for envelope in &envelopes {
                validate_protocol_version(&envelope.protocol_version).unwrap();
            }

            let log: LogChunk = serde_json::from_value(envelopes[0].payload.clone())
                .expect("log chunk should match contract");
            let artifacts: ArtifactManifest = serde_json::from_value(envelopes[1].payload.clone())
                .expect("artifact manifest should match contract");
            let usage: UsageReport = serde_json::from_value(envelopes[2].payload.clone())
                .expect("usage report should match contract");
            let result: JobOutcome = serde_json::from_value(envelopes[3].payload.clone())
                .expect("final result should match contract");

            assert_eq!(log.sequence, 1);
            assert_eq!(artifacts.job_id, "job_fixture_noop_001");
            assert_eq!(usage.schema_version, USAGE_SCHEMA_VERSION);
            assert_eq!(usage.status, expected_status);
            assert_eq!(result.state, expected_state);
            assert_eq!(result.correlation_id, "corr_fixture_noop_001");
        }
    }

    #[test]
    fn rejects_unsupported_control_plane_protocol_fixture() {
        let fixture =
            include_str!("../fixtures/control-plane/job-dispatch-unsupported-protocol.json");
        let envelope: ProtocolEnvelope<serde_json::Value> =
            serde_json::from_str(fixture).expect("unsupported fixture should remain parseable");

        assert_eq!(
            validate_protocol_version(&envelope.protocol_version),
            Err(ProtocolCompatibilityError::UnsupportedProtocolVersion(
                "remote.v2alpha1".to_string()
            ))
        );
    }

    #[test]
    fn contract_compatibility_matrix_declares_supported_versions() {
        let matrix: serde_json::Value =
            serde_json::from_str(include_str!("../contract-compatibility.json"))
                .expect("compatibility declaration should parse");
        let versions = matrix["remote"]["supported_protocol_versions"]
            .as_array()
            .expect("supported protocol versions must be declared");
        let usage_versions = matrix["remote"]["usage_schema_versions"]
            .as_array()
            .expect("usage schema versions must be declared");

        assert!(versions.iter().any(|version| version == PROTOCOL_VERSION));
        assert!(
            usage_versions
                .iter()
                .any(|version| version == USAGE_SCHEMA_VERSION)
        );
        assert!(
            matrix["remote"]["adapter_contract_versions"]
                .as_array()
                .expect("adapter contract versions must be declared")
                .iter()
                .any(|version| version == "remote_adapter_contract.v1alpha1")
        );
    }
}
