use crate::protocol::{AdapterKind, ArtifactDescriptor, ArtifactManifest, JobDispatch, LogChunk};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Registered,
    Heartbeating,
    Leased,
    PreparingWorkspace,
    Running,
    StreamingLogs,
    UploadingArtifacts,
    ReportingUsage,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobLease {
    pub lease_id: String,
    pub job_id: String,
    pub runner_id: String,
    pub working_group_id: String,
    pub correlation_id: String,
    pub timeout_seconds: u64,
    pub allowed_env: Vec<String>,
    pub risky_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobOutcome {
    pub job_id: String,
    pub runner_id: String,
    pub correlation_id: String,
    pub state: JobState,
    pub exit_code: Option<i32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterRunOutput {
    pub outcome: JobOutcome,
    pub logs: Vec<LogChunk>,
    pub artifacts: ArtifactManifest,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JobStateError {
    #[error("invalid job transition from {from:?} to {to:?}")]
    InvalidTransition { from: JobState, to: JobState },
    #[error("unsupported adapter kind: {0:?}")]
    UnsupportedAdapter(AdapterKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobLifecycle {
    state: JobState,
}

impl Default for JobLifecycle {
    fn default() -> Self {
        Self {
            state: JobState::Registered,
        }
    }
}

impl JobLifecycle {
    pub fn state(&self) -> JobState {
        self.state
    }

    pub fn transition(&mut self, to: JobState) -> Result<(), JobStateError> {
        if !can_transition(self.state, to) {
            return Err(JobStateError::InvalidTransition {
                from: self.state,
                to,
            });
        }
        self.state = to;
        Ok(())
    }
}

fn can_transition(from: JobState, to: JobState) -> bool {
    use JobState::*;

    matches!(
        (from, to),
        (Registered, Heartbeating)
            | (Heartbeating, Leased)
            | (Leased, PreparingWorkspace)
            | (PreparingWorkspace, Running)
            | (Running, StreamingLogs)
            | (StreamingLogs, UploadingArtifacts)
            | (UploadingArtifacts, ReportingUsage)
            | (ReportingUsage, Succeeded)
            | (Heartbeating, Cancelled)
            | (Leased, Cancelled)
            | (PreparingWorkspace, Cancelled)
            | (Running, Cancelled)
            | (Running, TimedOut)
            | (PreparingWorkspace, Failed)
            | (Running, Failed)
            | (StreamingLogs, Failed)
            | (UploadingArtifacts, Failed)
            | (ReportingUsage, Failed)
    )
}

pub fn lease_from_dispatch(dispatch: &JobDispatch) -> JobLease {
    JobLease {
        lease_id: dispatch.lease_id.clone(),
        job_id: dispatch.job_id.clone(),
        runner_id: dispatch.runner_id.clone(),
        working_group_id: dispatch.policy.working_group_id.clone(),
        correlation_id: dispatch.correlation_id.clone(),
        timeout_seconds: dispatch.timeout_seconds,
        allowed_env: dispatch.policy.workspace.allowed_env.clone(),
        risky_actions: dispatch.policy.risky_actions.clone(),
    }
}

pub fn run_noop_or_echo(dispatch: &JobDispatch) -> Result<AdapterRunOutput, JobStateError> {
    let mut lifecycle = JobLifecycle::default();
    lifecycle.transition(JobState::Heartbeating)?;
    lifecycle.transition(JobState::Leased)?;
    lifecycle.transition(JobState::PreparingWorkspace)?;
    lifecycle.transition(JobState::Running)?;

    let line = match dispatch.adapter {
        AdapterKind::Noop => "noop adapter completed without side effects".to_string(),
        AdapterKind::Echo => dispatch
            .input
            .get("echo")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("echo adapter completed")
            .to_string(),
        adapter => return Err(JobStateError::UnsupportedAdapter(adapter)),
    };

    lifecycle.transition(JobState::StreamingLogs)?;
    let logs = vec![LogChunk {
        job_id: dispatch.job_id.clone(),
        runner_id: dispatch.runner_id.clone(),
        sequence: 1,
        stream: "stdout".to_string(),
        redacted: false,
        line,
    }];

    lifecycle.transition(JobState::UploadingArtifacts)?;
    let artifacts = ArtifactManifest {
        job_id: dispatch.job_id.clone(),
        runner_id: dispatch.runner_id.clone(),
        artifacts: vec![ArtifactDescriptor {
            path_label: "stdout.txt".to_string(),
            content_type: "text/plain".to_string(),
            size_bytes: logs[0].line.len() as u64,
            checksum_sha256: "placeholder-not-uploaded".to_string(),
            retention_policy: "working_group_private".to_string(),
        }],
    };

    lifecycle.transition(JobState::ReportingUsage)?;
    lifecycle.transition(JobState::Succeeded)?;

    Ok(AdapterRunOutput {
        outcome: JobOutcome {
            job_id: dispatch.job_id.clone(),
            runner_id: dispatch.runner_id.clone(),
            correlation_id: dispatch.correlation_id.clone(),
            state: lifecycle.state(),
            exit_code: Some(0),
            message: "local echo lifecycle completed".to_string(),
        },
        logs,
        artifacts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_happy_path_transitions() {
        let mut lifecycle = JobLifecycle::default();

        for state in [
            JobState::Heartbeating,
            JobState::Leased,
            JobState::PreparingWorkspace,
            JobState::Running,
            JobState::StreamingLogs,
            JobState::UploadingArtifacts,
            JobState::ReportingUsage,
            JobState::Succeeded,
        ] {
            lifecycle.transition(state).expect("transition should work");
        }

        assert_eq!(lifecycle.state(), JobState::Succeeded);
    }

    #[test]
    fn rejects_skipping_workspace_preparation() {
        let mut lifecycle = JobLifecycle::default();
        lifecycle.transition(JobState::Heartbeating).unwrap();

        let error = lifecycle
            .transition(JobState::Running)
            .expect_err("cannot skip lease and workspace preparation");

        assert_eq!(
            error,
            JobStateError::InvalidTransition {
                from: JobState::Heartbeating,
                to: JobState::Running,
            }
        );
    }

    #[test]
    fn echo_adapter_runs_deterministic_lifecycle_from_dispatch_fixture() {
        let fixture = include_str!("../fixtures/control-plane/job-dispatch-echo.json");
        let envelope: crate::protocol::ProtocolEnvelope<JobDispatch> =
            serde_json::from_str(fixture).unwrap();
        let dispatch = envelope.payload;

        let lease = lease_from_dispatch(&dispatch);
        assert_eq!(lease.allowed_env, vec!["PATH", "TMPDIR"]);
        assert_eq!(lease.risky_actions, vec!["write_job_workspace"]);

        let output = run_noop_or_echo(&dispatch).expect("echo lifecycle should succeed");

        assert_eq!(output.outcome.state, JobState::Succeeded);
        assert_eq!(output.logs[0].sequence, 1);
        assert_eq!(output.logs[0].line, "hello from fixture");
        assert_eq!(output.artifacts.artifacts[0].path_label, "stdout.txt");
    }

    #[test]
    fn noop_adapter_runs_side_effect_free_lifecycle_from_dispatch_fixture() {
        let fixture = include_str!("../fixtures/control-plane/job-dispatch-noop.json");
        let envelope: crate::protocol::ProtocolEnvelope<JobDispatch> =
            serde_json::from_str(fixture).unwrap();
        let dispatch = envelope.payload;

        let lease = lease_from_dispatch(&dispatch);
        assert!(lease.allowed_env.is_empty());
        assert!(lease.risky_actions.is_empty());

        let output = run_noop_or_echo(&dispatch).expect("noop lifecycle should succeed");

        assert_eq!(output.outcome.state, JobState::Succeeded);
        assert_eq!(output.outcome.exit_code, Some(0));
        assert_eq!(
            output.logs[0].line,
            "noop adapter completed without side effects"
        );
        assert_eq!(output.artifacts.artifacts[0].path_label, "stdout.txt");
    }
}
