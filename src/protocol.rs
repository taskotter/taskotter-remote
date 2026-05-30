use serde::{Deserialize, Serialize};

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
pub struct UsageReport {
    pub job_id: String,
    pub runner_id: String,
    pub duration_ms: u64,
    pub cpu_time_ms: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
}
