use crate::{
    capabilities::RunnerCapabilities,
    config::RemoteConfig,
    job::{JobLifecycle, JobOutcome, JobState},
    protocol::{ProtocolEnvelope, UsageReport},
};
use anyhow::Context;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RunnerDaemon {
    config: RemoteConfig,
    capabilities: RunnerCapabilities,
}

impl RunnerDaemon {
    pub fn new(config: RemoteConfig) -> Self {
        let capabilities = RunnerCapabilities::discover(&config);
        Self {
            config,
            capabilities,
        }
    }

    pub fn registration_payload(&self) -> ProtocolEnvelope<RunnerCapabilities> {
        ProtocolEnvelope::new("runner.registration.request", self.capabilities.clone())
    }

    pub async fn run_once(&self) -> anyhow::Result<Vec<ProtocolEnvelope<serde_json::Value>>> {
        let runner_id = self
            .config
            .runner_id
            .clone()
            .unwrap_or_else(|| format!("unregistered-{}", Uuid::new_v4()));

        info!(
            control_plane_url = %self.config.control_plane_url,
            runner_id = %runner_id,
            "starting one-shot runner lifecycle"
        );

        if std::env::var(&self.config.registration_token_env).is_err() {
            warn!(
                registration_token_env = %self.config.registration_token_env,
                "registration token env var is not set; continuing with contract-only dry run"
            );
        }

        let mut lifecycle = JobLifecycle::default();
        lifecycle.transition(JobState::Heartbeating)?;
        sleep(Duration::from_millis(1)).await;
        lifecycle.transition(JobState::Leased)?;
        lifecycle.transition(JobState::PreparingWorkspace)?;
        lifecycle.transition(JobState::Running)?;
        lifecycle.transition(JobState::StreamingLogs)?;
        lifecycle.transition(JobState::UploadingArtifacts)?;
        lifecycle.transition(JobState::ReportingUsage)?;
        lifecycle.transition(JobState::Succeeded)?;

        let job_id = format!("placeholder-job-{}", Uuid::new_v4());
        let outcome = JobOutcome {
            job_id: job_id.clone(),
            state: lifecycle.state(),
            exit_code: Some(0),
            message: "placeholder runner execution completed".to_string(),
        };
        let usage = UsageReport {
            job_id,
            runner_id,
            duration_ms: 1,
            cpu_time_ms: None,
            peak_memory_bytes: None,
        };

        let payloads = vec![
            json_envelope(self.registration_payload())?,
            json_envelope(ProtocolEnvelope::new(
                "runner.heartbeat",
                self.capabilities.clone(),
            ))?,
            json_envelope(ProtocolEnvelope::new(
                "job.log.chunk",
                "placeholder log chunk",
            ))?,
            json_envelope(ProtocolEnvelope::new(
                "job.artifact.placeholder",
                "artifact upload contract placeholder",
            ))?,
            json_envelope(ProtocolEnvelope::new("job.usage.report", usage))?,
            json_envelope(ProtocolEnvelope::new("job.outcome.report", outcome))?,
        ];

        Ok(payloads)
    }
}

fn json_envelope<T: serde::Serialize>(
    envelope: ProtocolEnvelope<T>,
) -> anyhow::Result<ProtocolEnvelope<serde_json::Value>> {
    Ok(ProtocolEnvelope {
        protocol_version: envelope.protocol_version,
        message_type: envelope.message_type,
        payload: serde_json::to_value(envelope.payload)
            .context("failed to serialize protocol envelope payload")?,
    })
}
