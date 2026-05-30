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
    pub job_id: String,
    pub working_group_id: String,
    pub timeout_seconds: u64,
    pub allowed_env: Vec<String>,
    pub risky_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobOutcome {
    pub job_id: String,
    pub state: JobState,
    pub exit_code: Option<i32>,
    pub message: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JobStateError {
    #[error("invalid job transition from {from:?} to {to:?}")]
    InvalidTransition { from: JobState, to: JobState },
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
}
