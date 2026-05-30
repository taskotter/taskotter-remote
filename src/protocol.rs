use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

pub const RUNNER_PROTOCOL_VERSION: &str = "runner.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunnerEnvelope {
    pub protocol_version: String,
    pub runner_id: Uuid,
    pub sequence: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub sent_at: OffsetDateTime,
    pub event: RunnerEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnerEvent {
    Status(RunnerStatus),
    CapabilityInventory(CapabilityInventory),
    JobAccepted(JobAccepted),
    JobProgress(JobProgress),
    JobCompleted(JobCompleted),
    Usage(UsageEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerState {
    Starting,
    Idle,
    Busy,
    Draining,
    Disabled,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerStatus {
    pub state: RunnerState,
    pub active_jobs: u16,
    pub max_concurrency: u16,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityInventory {
    pub os: String,
    pub arch: String,
    pub runtimes: Vec<String>,
    pub tools: Vec<String>,
    pub mcp_host_modes: Vec<String>,
    pub local_llm_endpoints: Vec<String>,
    pub network_zone: String,
    pub labels: Vec<String>,
    pub max_concurrency: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecisionRef {
    pub decision_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub issued_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobAccepted {
    pub job_id: String,
    pub policy_decision: PolicyDecisionRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobProgress {
    pub job_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobCompleted {
    pub job_id: String,
    pub outcome: JobOutcome,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobOutcome {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    PolicyDenied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageEvent {
    pub usage_event_id: Uuid,
    pub job_id: String,
    pub meter: UsageMeter,
    pub quantity: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageMeter {
    ProcessRuntime,
    ToolInvocation,
    ArtifactBytes,
    LogBytes,
    LocalModelTokens,
}

impl RunnerEnvelope {
    pub fn new(runner_id: Uuid, sequence: u64, event: RunnerEvent) -> Self {
        Self {
            protocol_version: RUNNER_PROTOCOL_VERSION.to_string(),
            runner_id,
            sequence,
            sent_at: OffsetDateTime::now_utc(),
            event,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_versioned_status_event() {
        let envelope = RunnerEnvelope::new(
            Uuid::now_v7(),
            7,
            RunnerEvent::Status(RunnerStatus {
                state: RunnerState::Idle,
                active_jobs: 0,
                max_concurrency: 2,
                warnings: Vec::new(),
            }),
        );

        let value = serde_json::to_value(envelope).unwrap();
        assert_eq!(value["protocol_version"], RUNNER_PROTOCOL_VERSION);
        assert_eq!(value["event"]["type"], "status");
    }
}
