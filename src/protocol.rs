use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "remote.v1alpha1";

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
    use serde_json::json;

    use super::*;

    #[test]
    fn serializes_remote_usage_report_contract() {
        let report = UsageReport {
            schema_version: "remote_usage_report.v1".to_string(),
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
}
