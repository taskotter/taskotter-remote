use crate::{
    capabilities::RunnerCapabilities,
    config::RemoteConfig,
    job::{lease_from_dispatch, run_noop_or_echo},
    protocol::{
        Heartbeat, JobDispatch, JobLeaseAcceptance, ProgressUpdate, ProtocolEnvelope,
        RegistrationRequest, RunnerHealth, USAGE_SCHEMA_VERSION, UsageAttemptStatus, UsageReport,
    },
};
use anyhow::Context;
use tracing::{info, warn};

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

    pub fn registration_payload(&self) -> ProtocolEnvelope<RegistrationRequest> {
        ProtocolEnvelope::new(
            "runner.registration.request",
            RegistrationRequest {
                runner_id: self.config.runner_id.clone(),
                display_name: self
                    .config
                    .runner_id
                    .clone()
                    .unwrap_or_else(|| "unregistered-local-runner".to_string()),
                requested_working_groups: self.config.allowed_working_groups.clone(),
                registration_token_env: self.config.registration_token_env.clone(),
            },
        )
    }

    pub fn capability_snapshot_payload(&self) -> ProtocolEnvelope<RunnerCapabilities> {
        ProtocolEnvelope::new("runner.capability.snapshot", self.capabilities.clone())
    }

    pub async fn run_once(&self) -> anyhow::Result<Vec<ProtocolEnvelope<serde_json::Value>>> {
        let runner_id = self
            .config
            .runner_id
            .clone()
            .unwrap_or_else(|| "unregistered-local-runner".to_string());
        let dispatch = self.example_dispatch(&runner_id)?;

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

        let lease = lease_from_dispatch(&dispatch);
        let output = run_noop_or_echo(&dispatch)?;
        let usage = UsageReport {
            schema_version: USAGE_SCHEMA_VERSION.to_string(),
            job_id: lease.job_id.clone(),
            runner_id: runner_id.clone(),
            request_id: None,
            correlation_id: Some(lease.correlation_id),
            status: UsageAttemptStatus::Succeeded,
            duration_ms: 0,
            cpu_time_ms: None,
            peak_memory_bytes: None,
            prompt_tokens: 0,
            completion_tokens: 0,
            estimated_cost_micro_usd: 0,
        };

        let payloads = vec![
            json_envelope(self.registration_payload())?,
            json_envelope(self.capability_snapshot_payload())?,
            json_envelope(ProtocolEnvelope::new(
                "runner.heartbeat",
                Heartbeat {
                    runner_id: runner_id.clone(),
                    capability_inventory_version: self.capabilities.protocol_version.clone(),
                    daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                    health: RunnerHealth::Online,
                    active_job_ids: vec![],
                    available_concurrency: self.config.max_concurrency,
                    clock_skew_ms: 0,
                    resource_pressure: vec![],
                    last_error_category: None,
                },
            ))?,
            json_envelope(ProtocolEnvelope::new(
                "job.lease.accepted",
                JobLeaseAcceptance {
                    lease_id: lease.lease_id,
                    job_id: lease.job_id,
                    runner_id: runner_id.clone(),
                    accepted: true,
                    reason: None,
                },
            ))?,
            json_envelope(ProtocolEnvelope::new(
                "job.progress",
                ProgressUpdate {
                    job_id: output.outcome.job_id.clone(),
                    runner_id: runner_id.clone(),
                    sequence: 1,
                    phase: "running".to_string(),
                    percent: 100,
                },
            ))?,
            json_envelope(ProtocolEnvelope::new(
                "job.log.chunk",
                output.logs[0].clone(),
            ))?,
            json_envelope(ProtocolEnvelope::new(
                "job.artifact.manifest",
                output.artifacts,
            ))?,
            json_envelope(ProtocolEnvelope::new("job.usage.report", usage))?,
            json_envelope(ProtocolEnvelope::new("job.final_result", output.outcome))?,
        ];

        Ok(payloads)
    }

    fn example_dispatch(&self, runner_id: &str) -> anyhow::Result<JobDispatch> {
        let mut envelope: ProtocolEnvelope<JobDispatch> = serde_json::from_str(include_str!(
            "../fixtures/control-plane/job-dispatch-echo.json"
        ))
        .context("failed to load bundled job dispatch fixture")?;
        envelope.payload.runner_id = runner_id.to_string();
        envelope.payload.timeout_seconds = self.config.job_timeout_seconds;
        envelope.payload.policy.workspace.allowed_env = self.config.env_allowlist.clone();
        envelope.payload.policy.risky_actions = self.config.risky_action_allowlist.clone();
        if let Some(working_group_id) = self.config.allowed_working_groups.first() {
            envelope.payload.policy.working_group_id = working_group_id.clone();
        }
        envelope.payload.policy.workspace.scoped_credential_ref =
            self.config.credential_placeholder.clone();
        Ok(envelope.payload)
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
