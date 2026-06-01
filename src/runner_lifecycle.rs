use crate::protocol::JobLeaseAcceptance;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RunnerLifecycleState {
    Registered,
    Unregistered,
    Expired,
    Disabled,
    Draining,
    Shared,
    Quarantined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerLifecycleScenario {
    pub now_tick: u64,
    pub registration_tokens: Vec<RegistrationTokenFixture>,
    pub credentials: Vec<RunnerCredentialFixture>,
    pub runners: Vec<RunnerFixture>,
    pub lease_attempts: Vec<LeaseAttemptFixture>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrationTokenFixture {
    pub token_id: String,
    pub expires_at_tick: u64,
    pub used_by_runner_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerCredentialFixture {
    pub credential_ref: String,
    pub expires_at_tick: u64,
    pub active: bool,
    pub rotated_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunnerFixture {
    pub runner_id: String,
    pub state: RunnerLifecycleState,
    pub registration_token_id: Option<String>,
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseAttemptFixture {
    pub runner_id: String,
    pub job_id: String,
    pub lease_id: String,
    pub expect_accepted: bool,
    pub expect_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerCredentialGrant {
    pub runner_id: String,
    pub credential_ref: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RunnerLifecycleError {
    #[error("registration token expired")]
    RegistrationTokenExpired,
    #[error("registration token already used")]
    RegistrationTokenAlreadyUsed,
    #[error("registration token not found")]
    RegistrationTokenNotFound,
    #[error("runner not found")]
    RunnerNotFound,
    #[error("runner credential not found")]
    RunnerCredentialNotFound,
    #[error("runner credential expired")]
    RunnerCredentialExpired,
    #[error("runner credential inactive")]
    RunnerCredentialInactive,
    #[error("runner credential rotation mismatch")]
    RunnerCredentialRotationMismatch,
}

pub struct RunnerLifecycleSimulator {
    now_tick: u64,
    registration_tokens: HashMap<String, RegistrationTokenFixture>,
    credentials: HashMap<String, RunnerCredentialFixture>,
    runners: HashMap<String, RunnerFixture>,
}

impl RunnerLifecycleSimulator {
    pub fn from_scenario(scenario: RunnerLifecycleScenario) -> Self {
        Self {
            now_tick: scenario.now_tick,
            registration_tokens: scenario
                .registration_tokens
                .into_iter()
                .map(|token| (token.token_id.clone(), token))
                .collect(),
            credentials: scenario
                .credentials
                .into_iter()
                .map(|credential| (credential.credential_ref.clone(), credential))
                .collect(),
            runners: scenario
                .runners
                .into_iter()
                .map(|runner| (runner.runner_id.clone(), runner))
                .collect(),
        }
    }

    pub fn register_runner(
        &mut self,
        token_id: &str,
        runner_id: &str,
        credential_ref: &str,
    ) -> Result<RunnerCredentialGrant, RunnerLifecycleError> {
        let token = self
            .registration_tokens
            .get_mut(token_id)
            .ok_or(RunnerLifecycleError::RegistrationTokenNotFound)?;
        if token.expires_at_tick <= self.now_tick {
            return Err(RunnerLifecycleError::RegistrationTokenExpired);
        }
        if token.used_by_runner_id.is_some() {
            return Err(RunnerLifecycleError::RegistrationTokenAlreadyUsed);
        }
        let credential = self
            .credentials
            .get(credential_ref)
            .ok_or(RunnerLifecycleError::RunnerCredentialNotFound)?;
        validate_credential(credential, self.now_tick)?;

        token.used_by_runner_id = Some(runner_id.to_string());
        self.runners.insert(
            runner_id.to_string(),
            RunnerFixture {
                runner_id: runner_id.to_string(),
                state: RunnerLifecycleState::Registered,
                registration_token_id: Some(token_id.to_string()),
                credential_ref: Some(credential_ref.to_string()),
            },
        );

        Ok(RunnerCredentialGrant {
            runner_id: runner_id.to_string(),
            credential_ref: credential_ref.to_string(),
        })
    }

    pub fn rotate_credential(
        &mut self,
        runner_id: &str,
        current_credential_ref: &str,
        next_credential_ref: &str,
    ) -> Result<RunnerCredentialGrant, RunnerLifecycleError> {
        let runner = self
            .runners
            .get_mut(runner_id)
            .ok_or(RunnerLifecycleError::RunnerNotFound)?;
        if runner.credential_ref.as_deref() != Some(current_credential_ref) {
            return Err(RunnerLifecycleError::RunnerCredentialRotationMismatch);
        }
        let current = self
            .credentials
            .get_mut(current_credential_ref)
            .ok_or(RunnerLifecycleError::RunnerCredentialNotFound)?;
        current.active = false;

        let next = self
            .credentials
            .get(next_credential_ref)
            .ok_or(RunnerLifecycleError::RunnerCredentialNotFound)?;
        validate_credential(next, self.now_tick)?;

        runner.state = RunnerLifecycleState::Registered;
        runner.credential_ref = Some(next_credential_ref.to_string());
        Ok(RunnerCredentialGrant {
            runner_id: runner_id.to_string(),
            credential_ref: next_credential_ref.to_string(),
        })
    }

    pub fn evaluate_lease(&self, attempt: &LeaseAttemptFixture) -> JobLeaseAcceptance {
        let reason = self
            .lease_rejection_reason(&attempt.runner_id)
            .map(str::to_string);

        JobLeaseAcceptance {
            lease_id: attempt.lease_id.clone(),
            job_id: attempt.job_id.clone(),
            runner_id: attempt.runner_id.clone(),
            accepted: reason.is_none(),
            reason,
        }
    }

    fn lease_rejection_reason(&self, runner_id: &str) -> Option<&'static str> {
        let Some(runner) = self.runners.get(runner_id) else {
            return Some("runner is not registered");
        };
        match runner.state {
            RunnerLifecycleState::Registered => {}
            RunnerLifecycleState::Unregistered => return Some("runner is not registered"),
            RunnerLifecycleState::Expired => {
                return Some("runner credential expired; rotate before accepting leases");
            }
            RunnerLifecycleState::Disabled => return Some("runner is disabled"),
            RunnerLifecycleState::Draining => return Some("runner is draining"),
            RunnerLifecycleState::Shared => {
                return Some("runner credential is shared; rotate before accepting leases");
            }
            RunnerLifecycleState::Quarantined => return Some("runner is quarantined"),
        }

        let Some(credential_ref) = runner.credential_ref.as_deref() else {
            return Some("runner credential unavailable");
        };
        match self.credentials.get(credential_ref) {
            Some(credential) if validate_credential(credential, self.now_tick).is_ok() => None,
            Some(credential) if credential.expires_at_tick <= self.now_tick => {
                Some("runner credential expired; rotate before accepting leases")
            }
            Some(_) => Some("runner credential unavailable"),
            None => Some("runner credential unavailable"),
        }
    }
}

fn validate_credential(
    credential: &RunnerCredentialFixture,
    now_tick: u64,
) -> Result<(), RunnerLifecycleError> {
    if !credential.active {
        return Err(RunnerLifecycleError::RunnerCredentialInactive);
    }
    if credential.expires_at_tick <= now_tick {
        return Err(RunnerLifecycleError::RunnerCredentialExpired);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario() -> RunnerLifecycleScenario {
        serde_json::from_str(include_str!(
            "../fixtures/runner/credential-lifecycle-simulator.json"
        ))
        .expect("credential lifecycle simulator fixture should parse")
    }

    #[test]
    fn simulator_fixture_covers_runner_credential_states() {
        let scenario = scenario();
        let states = scenario
            .runners
            .iter()
            .map(|runner| runner.state)
            .collect::<std::collections::HashSet<_>>();

        for state in [
            RunnerLifecycleState::Registered,
            RunnerLifecycleState::Unregistered,
            RunnerLifecycleState::Expired,
            RunnerLifecycleState::Disabled,
            RunnerLifecycleState::Draining,
            RunnerLifecycleState::Shared,
            RunnerLifecycleState::Quarantined,
        ] {
            assert!(states.contains(&state), "fixture missing {state:?}");
        }
    }

    #[test]
    fn registration_token_is_single_use_and_short_lived() {
        let mut simulator = RunnerLifecycleSimulator::from_scenario(scenario());

        let grant = simulator
            .register_runner(
                "reg-token-fixture-available",
                "runner-newly-registered",
                "credential-ref-active",
            )
            .expect("unused token should register runner");
        assert_eq!(grant.runner_id, "runner-newly-registered");

        let reused = simulator
            .register_runner(
                "reg-token-fixture-available",
                "runner-reused-token",
                "credential-ref-active",
            )
            .expect_err("registration token must be single-use");
        assert_eq!(reused, RunnerLifecycleError::RegistrationTokenAlreadyUsed);

        let expired = simulator
            .register_runner(
                "reg-token-fixture-expired",
                "runner-expired-token",
                "credential-ref-active",
            )
            .expect_err("expired token must not register a runner");
        assert_eq!(expired, RunnerLifecycleError::RegistrationTokenExpired);
    }

    #[test]
    fn lease_attempts_match_deterministic_fixture_expectations() {
        let scenario = scenario();
        let simulator = RunnerLifecycleSimulator::from_scenario(scenario.clone());

        for attempt in scenario.lease_attempts {
            let acceptance = simulator.evaluate_lease(&attempt);

            assert_eq!(acceptance.accepted, attempt.expect_accepted);
            assert_eq!(acceptance.reason, attempt.expect_reason);
        }
    }

    #[test]
    fn disabled_and_quarantined_runners_do_not_receive_new_leases() {
        let scenario = scenario();
        let simulator = RunnerLifecycleSimulator::from_scenario(scenario.clone());

        for runner_id in ["runner-disabled", "runner-quarantined"] {
            let attempt = scenario
                .lease_attempts
                .iter()
                .find(|attempt| attempt.runner_id == runner_id)
                .expect("fixture should include state-specific lease attempt");
            let acceptance = simulator.evaluate_lease(attempt);

            assert!(!acceptance.accepted);
            assert!(acceptance.reason.is_some());
        }
    }

    #[test]
    fn credential_expiry_and_rotation_return_safe_errors_without_secret_material() {
        let scenario = scenario();
        let mut simulator = RunnerLifecycleSimulator::from_scenario(scenario.clone());
        let expired_attempt = scenario
            .lease_attempts
            .iter()
            .find(|attempt| attempt.runner_id == "runner-expired")
            .expect("fixture should include expired runner");

        let expired = simulator.evaluate_lease(expired_attempt);
        let reason = expired
            .reason
            .expect("expired credential should reject lease");
        assert!(!reason.contains("credential-ref-expired"));
        assert!(!reason.contains("global"));
        assert_eq!(
            reason,
            "runner credential expired; rotate before accepting leases"
        );

        let rotated = simulator
            .rotate_credential(
                "runner-registered",
                "credential-ref-active",
                "credential-ref-rotated",
            )
            .expect("valid rotation should update runner credential ref");
        assert_eq!(rotated.credential_ref, "credential-ref-rotated");

        let accepted = simulator.evaluate_lease(&LeaseAttemptFixture {
            runner_id: "runner-registered".to_string(),
            job_id: "job_after_rotation".to_string(),
            lease_id: "lease_after_rotation".to_string(),
            expect_accepted: true,
            expect_reason: None,
        });
        assert!(accepted.accepted);

        let mismatch = simulator
            .rotate_credential(
                "runner-registered",
                "credential-ref-active",
                "credential-ref-rotated",
            )
            .expect_err("stale credential ref must not rotate twice");
        assert_eq!(
            mismatch,
            RunnerLifecycleError::RunnerCredentialRotationMismatch
        );
    }
}
