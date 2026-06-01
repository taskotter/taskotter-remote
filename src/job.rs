use crate::protocol::{
    AdapterKind, ArtifactDescriptor, ArtifactManifest, JobDispatch, JobLifecyclePhase, LogChunk,
};
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
    pub state: JobLifecyclePhase,
    pub exit_code: Option<i32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterRunOutput {
    pub outcome: JobOutcome,
    pub logs: Vec<LogChunk>,
    pub artifacts: ArtifactManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulatedTerminalOutcome {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
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
            checksum_sha256: if dispatch.adapter == AdapterKind::Noop {
                "08d9d37a1d9dbc2108aad0cecc04503b2a6bbd699a266f151f928988af025d5f".to_string()
            } else {
                "placeholder-not-uploaded".to_string()
            },
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
            state: JobLifecyclePhase::Completed,
            exit_code: Some(0),
            message: "local echo lifecycle completed".to_string(),
        },
        logs,
        artifacts,
    })
}

pub fn simulate_noop_lifecycle(
    dispatch: &JobDispatch,
    terminal: SimulatedTerminalOutcome,
) -> Result<AdapterRunOutput, JobStateError> {
    if dispatch.adapter != AdapterKind::Noop {
        return Err(JobStateError::UnsupportedAdapter(dispatch.adapter));
    }

    let mut lifecycle = JobLifecycle::default();
    lifecycle.transition(JobState::Heartbeating)?;
    lifecycle.transition(JobState::Leased)?;
    lifecycle.transition(JobState::PreparingWorkspace)?;
    lifecycle.transition(JobState::Running)?;

    let (final_state, exit_code, message, stream, line) = match terminal {
        SimulatedTerminalOutcome::Succeeded => (
            JobLifecyclePhase::Completed,
            Some(0),
            "local noop lifecycle completed",
            "stdout",
            "noop adapter completed without side effects",
        ),
        SimulatedTerminalOutcome::Failed => (
            JobLifecyclePhase::Failed,
            Some(1),
            "local noop lifecycle failed before result upload",
            "stderr",
            "noop simulation injected deterministic failure",
        ),
        SimulatedTerminalOutcome::Cancelled => (
            JobLifecyclePhase::Cancelled,
            None,
            "local noop lifecycle cancelled by control plane request",
            "stderr",
            "noop simulation observed operator_requested cancellation",
        ),
        SimulatedTerminalOutcome::TimedOut => (
            JobLifecyclePhase::TimedOut,
            None,
            "local noop lifecycle exceeded dispatch timeout",
            "stderr",
            "noop simulation exceeded 30 second timeout",
        ),
    };

    match terminal {
        SimulatedTerminalOutcome::Succeeded => {
            lifecycle.transition(JobState::StreamingLogs)?;
            lifecycle.transition(JobState::UploadingArtifacts)?;
            lifecycle.transition(JobState::ReportingUsage)?;
            lifecycle.transition(JobState::Succeeded)?;
        }
        SimulatedTerminalOutcome::Failed => lifecycle.transition(JobState::Failed)?,
        SimulatedTerminalOutcome::Cancelled => lifecycle.transition(JobState::Cancelled)?,
        SimulatedTerminalOutcome::TimedOut => lifecycle.transition(JobState::TimedOut)?,
    }

    let logs = vec![LogChunk {
        job_id: dispatch.job_id.clone(),
        runner_id: dispatch.runner_id.clone(),
        sequence: 1,
        stream: stream.to_string(),
        redacted: false,
        line: line.to_string(),
    }];

    let artifacts = ArtifactManifest {
        job_id: dispatch.job_id.clone(),
        runner_id: dispatch.runner_id.clone(),
        artifacts: match terminal {
            SimulatedTerminalOutcome::Succeeded => vec![ArtifactDescriptor {
                path_label: "stdout.txt".to_string(),
                content_type: "text/plain".to_string(),
                size_bytes: line.len() as u64,
                checksum_sha256: "08d9d37a1d9dbc2108aad0cecc04503b2a6bbd699a266f151f928988af025d5f"
                    .to_string(),
                retention_policy: "working_group_private".to_string(),
            }],
            SimulatedTerminalOutcome::Failed
            | SimulatedTerminalOutcome::Cancelled
            | SimulatedTerminalOutcome::TimedOut => vec![],
        },
    };

    Ok(AdapterRunOutput {
        outcome: JobOutcome {
            job_id: dispatch.job_id.clone(),
            runner_id: dispatch.runner_id.clone(),
            correlation_id: dispatch.correlation_id.clone(),
            state: final_state,
            exit_code,
            message: message.to_string(),
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

        assert_eq!(output.outcome.state, JobLifecyclePhase::Completed);
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

        assert_eq!(output.outcome.state, JobLifecyclePhase::Completed);
        assert_eq!(output.outcome.exit_code, Some(0));
        assert_eq!(
            output.logs[0].line,
            "noop adapter completed without side effects"
        );
        assert_eq!(output.artifacts.artifacts[0].path_label, "stdout.txt");
        assert_eq!(
            output.artifacts.artifacts[0].checksum_sha256,
            "08d9d37a1d9dbc2108aad0cecc04503b2a6bbd699a266f151f928988af025d5f"
        );
    }

    #[test]
    fn simulates_noop_terminal_lifecycle_outcomes_deterministically() {
        let fixture = include_str!("../fixtures/control-plane/job-dispatch-noop.json");
        let envelope: crate::protocol::ProtocolEnvelope<JobDispatch> =
            serde_json::from_str(fixture).unwrap();
        let dispatch = envelope.payload;

        for (terminal, state, exit_code, stream) in [
            (
                SimulatedTerminalOutcome::Succeeded,
                JobLifecyclePhase::Completed,
                Some(0),
                "stdout",
            ),
            (
                SimulatedTerminalOutcome::Failed,
                JobLifecyclePhase::Failed,
                Some(1),
                "stderr",
            ),
            (
                SimulatedTerminalOutcome::Cancelled,
                JobLifecyclePhase::Cancelled,
                None,
                "stderr",
            ),
            (
                SimulatedTerminalOutcome::TimedOut,
                JobLifecyclePhase::TimedOut,
                None,
                "stderr",
            ),
        ] {
            let output =
                simulate_noop_lifecycle(&dispatch, terminal).expect("simulation should run");

            assert_eq!(output.outcome.state, state);
            assert_eq!(output.outcome.exit_code, exit_code);
            assert_eq!(output.logs[0].sequence, 1);
            assert_eq!(output.logs[0].stream, stream);
            assert_eq!(output.artifacts.job_id, dispatch.job_id);
            assert_eq!(output.artifacts.runner_id, dispatch.runner_id);
        }
    }
}
