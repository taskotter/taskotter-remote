use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "remote.v1alpha1";
pub const USAGE_SCHEMA_VERSION: &str = "remote_usage_report.v1";
pub const HTTPS_FALLBACK_SCHEMA_VERSION: &str = "remote_https_fallback.v1alpha1";
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
pub struct SignedDispatchInstruction {
    pub instruction_id: String,
    pub job_id: String,
    pub lease_id: String,
    pub tenant_id: String,
    pub runner_id: String,
    pub capability_inventory_version: String,
    pub protocol_version: String,
    pub policy_decision_id: String,
    pub policy_version: String,
    pub allowed_adapter: AdapterKind,
    pub execution_mode: String,
    pub timeout_seconds: u64,
    pub max_concurrency_impact: u16,
    pub input_refs: Vec<String>,
    pub artifact_allowlist: Vec<String>,
    pub credential_ref: String,
    pub credential_expires_at: String,
    pub credential_audience: String,
    pub network: NetworkPolicy,
    pub log_redaction: String,
    pub retention_class: String,
    pub usage_correlation_id: String,
    pub audit_correlation_id: String,
    pub issued_at: String,
    pub not_before: String,
    pub expires_at: String,
    pub nonce: String,
    pub key_id: String,
    pub signature: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_instruction: Option<SignedDispatchInstruction>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpsFallbackPollRequest {
    pub schema_version: String,
    pub runner_id: String,
    pub protocol_version: String,
    pub capability_inventory_version: String,
    pub last_control_plane_event_id: Option<String>,
    pub last_runner_event_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpsFallbackPollResponse {
    pub schema_version: String,
    pub protocol_version: String,
    pub control_plane_event_id: String,
    pub commands: Vec<ProtocolEnvelope<serde_json::Value>>,
    pub next_poll_after_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpsFallbackEventBatchRequest {
    pub schema_version: String,
    pub runner_id: String,
    pub idempotency_key: String,
    pub first_sequence: u64,
    pub events: Vec<ProtocolEnvelope<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpsFallbackEventBatchResponse {
    pub schema_version: String,
    pub accepted: bool,
    pub accepted_sequences: Vec<u64>,
    pub retry_after_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use std::{fmt, path::Path};

    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[derive(Debug, Deserialize)]
    struct ContractCompatibilityMatrix {
        remote: RemoteCompatibility,
    }

    #[derive(Debug, Deserialize)]
    struct RemoteCompatibility {
        supported_protocol_versions: Vec<String>,
        usage_schema_versions: Vec<String>,
        https_fallback_schema_versions: Vec<String>,
        adapter_contract_versions: Vec<String>,
        schemas: Vec<String>,
        fixtures: Vec<String>,
        additive_tolerance_fixtures: Vec<String>,
        negative_cases: NegativeCompatibilityCases,
        consumer_followups: Vec<ConsumerFollowup>,
    }

    #[derive(Debug, Deserialize)]
    struct NegativeCompatibilityCases {
        unsupported_protocol_fixture: String,
        missing_fixture_path: String,
    }

    #[derive(Debug, Deserialize)]
    struct ConsumerFollowup {
        repo: String,
        owner: String,
        work: String,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum ManifestValidationError {
        MissingFixture(String),
    }

    impl fmt::Display for ManifestValidationError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::MissingFixture(path) => write!(f, "manifest fixture is missing: {path}"),
            }
        }
    }

    fn compatibility_matrix() -> ContractCompatibilityMatrix {
        serde_json::from_str(include_str!("../contract-compatibility.json"))
            .expect("compatibility declaration should parse")
    }

    fn validate_manifest_fixture_paths(fixtures: &[String]) -> Result<(), ManifestValidationError> {
        for fixture in fixtures {
            if !Path::new(fixture).exists() {
                return Err(ManifestValidationError::MissingFixture(fixture.clone()));
            }
        }

        Ok(())
    }

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
    fn deserializes_additive_control_plane_fixture() {
        let fixture = include_str!("../fixtures/control-plane/job-dispatch-additive-field.json");
        let envelope: ProtocolEnvelope<JobDispatch> =
            serde_json::from_str(fixture).expect("additive fixture should stay compatible");

        validate_protocol_version(&envelope.protocol_version).unwrap();
        assert_eq!(envelope.message_type, "job.dispatch");
        assert_eq!(envelope.payload.adapter, AdapterKind::Echo);
        assert_eq!(
            envelope.payload.input["echo"],
            json!("additive fixture remains compatible")
        );
        assert_eq!(
            envelope.payload.policy.workspace.artifact_path_allowlist,
            vec!["stdout.txt".to_string()]
        );
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
    fn deserializes_runner_evidence_fixtures() {
        let heartbeat: ProtocolEnvelope<Heartbeat> =
            serde_json::from_str(include_str!("../fixtures/runner/heartbeat-online.json"))
                .expect("heartbeat fixture should match runner protocol");
        let log: ProtocolEnvelope<LogChunk> =
            serde_json::from_str(include_str!("../fixtures/runner/job-log-stdout.json"))
                .expect("log fixture should match runner protocol");
        let artifacts: ProtocolEnvelope<ArtifactManifest> =
            serde_json::from_str(include_str!("../fixtures/runner/job-artifacts.json"))
                .expect("artifact fixture should match runner protocol");
        let usage: ProtocolEnvelope<UsageReport> =
            serde_json::from_str(include_str!("../fixtures/runner/job-usage-succeeded.json"))
                .expect("usage fixture should match runner protocol");
        let result: ProtocolEnvelope<AdapterResult> = serde_json::from_str(include_str!(
            "../fixtures/runner/job-final-result-succeeded.json"
        ))
        .expect("result fixture should match runner protocol");

        validate_protocol_version(&heartbeat.protocol_version).unwrap();
        assert_eq!(heartbeat.message_type, "runner.heartbeat");
        assert_eq!(heartbeat.payload.health, RunnerHealth::Online);
        assert_eq!(log.message_type, "job.log");
        assert_eq!(log.payload.sequence, 3);
        assert!(log.payload.redacted);
        assert_eq!(artifacts.message_type, "job.artifacts");
        assert_eq!(
            artifacts.payload.artifacts[0].retention_policy,
            "standard_30d"
        );
        assert_eq!(usage.message_type, "job.usage.report");
        assert_eq!(usage.payload.schema_version, USAGE_SCHEMA_VERSION);
        assert_eq!(result.message_type, "job.final_result");
        assert_eq!(result.payload.phase, JobLifecyclePhase::Completed);
    }

    #[test]
    fn deserializes_https_fallback_fixtures() {
        let poll_request: HttpsFallbackPollRequest =
            serde_json::from_str(include_str!("../fixtures/https-fallback/poll-request.json"))
                .expect("poll request fixture should match HTTPS fallback contract");
        let poll_response: HttpsFallbackPollResponse = serde_json::from_str(include_str!(
            "../fixtures/https-fallback/poll-response-dispatch.json"
        ))
        .expect("poll response fixture should match HTTPS fallback contract");
        let event_batch: HttpsFallbackEventBatchRequest = serde_json::from_str(include_str!(
            "../fixtures/https-fallback/event-batch-request.json"
        ))
        .expect("event batch request fixture should match HTTPS fallback contract");
        let event_response: HttpsFallbackEventBatchResponse = serde_json::from_str(include_str!(
            "../fixtures/https-fallback/event-batch-response.json"
        ))
        .expect("event batch response fixture should match HTTPS fallback contract");

        assert_eq!(poll_request.schema_version, HTTPS_FALLBACK_SCHEMA_VERSION);
        validate_protocol_version(&poll_request.protocol_version).unwrap();
        assert_eq!(poll_response.commands[0].message_type, "job.dispatch");
        assert_eq!(event_batch.first_sequence, 1);
        assert_eq!(event_batch.events[0].message_type, "runner.heartbeat");
        assert!(event_response.accepted);
        assert_eq!(event_response.accepted_sequences, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn canonical_schema_and_error_taxonomy_snapshots_are_parseable() {
        let runner_schema: serde_json::Value =
            serde_json::from_str(include_str!("../schemas/runner-protocol.v1alpha1.json"))
                .expect("runner protocol schema should parse");
        let https_schema: serde_json::Value =
            serde_json::from_str(include_str!("../schemas/https-fallback.v1alpha1.json"))
                .expect("HTTPS fallback schema should parse");
        let taxonomy: serde_json::Value = serde_json::from_str(include_str!(
            "../schemas/runner-error-taxonomy.v1alpha1.json"
        ))
        .expect("error taxonomy snapshot should parse");

        assert_eq!(runner_schema["protocol_version"], PROTOCOL_VERSION);
        assert_eq!(
            https_schema["schema_version"],
            HTTPS_FALLBACK_SCHEMA_VERSION
        );
        let categories = taxonomy["categories"]
            .as_array()
            .expect("error taxonomy categories should be an array");
        for expected in [
            "policy_denied",
            "capability_mismatch",
            "workspace_setup",
            "adapter_failed",
            "timeout",
            "cancelled",
            "artifact_upload",
            "usage_report",
            "internal",
        ] {
            assert!(
                categories
                    .iter()
                    .any(|category| category["name"] == expected)
            );
        }
    }

    #[test]
    fn rejects_unsupported_control_plane_protocol_fixture() {
        let matrix = compatibility_matrix();
        let fixture =
            std::fs::read_to_string(&matrix.remote.negative_cases.unsupported_protocol_fixture)
                .expect("unsupported fixture should exist for negative protocol coverage");
        let envelope: ProtocolEnvelope<serde_json::Value> =
            serde_json::from_str(&fixture).expect("unsupported fixture should remain parseable");

        assert_eq!(
            validate_protocol_version(&envelope.protocol_version),
            Err(ProtocolCompatibilityError::UnsupportedProtocolVersion(
                "remote.v2alpha1".to_string()
            ))
        );
    }

    #[test]
    fn contract_compatibility_matrix_declares_supported_versions() {
        let matrix = compatibility_matrix();

        assert!(
            matrix
                .remote
                .supported_protocol_versions
                .iter()
                .any(|version| version == PROTOCOL_VERSION)
        );
        assert!(
            matrix
                .remote
                .usage_schema_versions
                .iter()
                .any(|version| version == USAGE_SCHEMA_VERSION)
        );
        assert!(
            matrix
                .remote
                .adapter_contract_versions
                .iter()
                .any(|version| version == "remote_adapter_contract.v1alpha1")
        );
        assert!(
            matrix
                .remote
                .https_fallback_schema_versions
                .iter()
                .any(|version| version == HTTPS_FALLBACK_SCHEMA_VERSION)
        );
        assert!(matrix.remote.fixtures.len() >= 14);
        assert!(
            matrix
                .remote
                .additive_tolerance_fixtures
                .iter()
                .any(|fixture| fixture == "fixtures/control-plane/job-dispatch-additive-field.json")
        );
        assert!(
            matrix
                .remote
                .negative_cases
                .missing_fixture_path
                .ends_with("job-dispatch-missing-required-fixture.json")
        );
        assert!(matrix.remote.consumer_followups.iter().any(|followup| {
            followup.repo == "taskotter/taskotter"
                && followup.owner == "control-plane"
                && followup.work.contains("canonical contract generation")
        }));
        validate_manifest_fixture_paths(&matrix.remote.schemas).unwrap();
        validate_manifest_fixture_paths(&matrix.remote.fixtures).unwrap();
        validate_manifest_fixture_paths(&matrix.remote.additive_tolerance_fixtures).unwrap();
        validate_manifest_fixture_paths(std::slice::from_ref(
            &matrix.remote.negative_cases.unsupported_protocol_fixture,
        ))
        .unwrap();
    }

    #[test]
    fn contract_compatibility_matrix_fails_for_missing_fixture() {
        let matrix = compatibility_matrix();

        assert_eq!(
            validate_manifest_fixture_paths(std::slice::from_ref(
                &matrix.remote.negative_cases.missing_fixture_path,
            )),
            Err(ManifestValidationError::MissingFixture(
                "fixtures/control-plane/job-dispatch-missing-required-fixture.json".to_string()
            ))
        );
    }
}
