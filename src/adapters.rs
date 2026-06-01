use crate::{
    capabilities::{
        AllowlistedCommandAdapterCapability, ExternalAgentAdapterCapability,
        LocalLlmEndpointCapability, RunnerCapabilities,
    },
    config::RuntimeFeatureFlags,
    gates::{CapabilityGateDecision, HighRiskCapability, evaluate_high_risk_capability},
    protocol::{AdapterExecutionMetadata, AdapterKind, JobDispatch, JobLifecyclePhase},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterExecutionPlan {
    pub adapter_id: String,
    pub adapter_kind: AdapterKind,
    pub required_capability: Option<HighRiskCapability>,
    pub phase: JobLifecyclePhase,
    pub metadata: AdapterExecutionMetadata,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdapterContractError {
    #[error("missing runner capability for adapter kind: {0:?}")]
    MissingCapability(AdapterKind),
    #[error("adapter execution denied by policy: {reason_code}")]
    PolicyDenied {
        capability: HighRiskCapability,
        feature_flag: &'static str,
        reason_code: &'static str,
    },
    #[error("adapter boundary denied for {adapter_kind:?}: {reason_code}")]
    BoundaryDenied {
        adapter_kind: AdapterKind,
        reason_code: &'static str,
    },
    #[error("malformed adapter result: {0}")]
    MalformedResult(&'static str),
}

pub fn plan_adapter_execution(
    dispatch: &JobDispatch,
    capabilities: &RunnerCapabilities,
    flags: &RuntimeFeatureFlags,
) -> Result<AdapterExecutionPlan, AdapterContractError> {
    let required_capability = required_capability(dispatch.adapter);
    let adapter_id = resolve_adapter_id(dispatch, capabilities)?;

    if let Some(capability) = required_capability {
        match evaluate_high_risk_capability(flags, &dispatch.policy, capability) {
            CapabilityGateDecision::Allowed => {}
            CapabilityGateDecision::Denied {
                capability,
                feature_flag,
                reason_code,
            } => {
                return Err(AdapterContractError::PolicyDenied {
                    capability,
                    feature_flag,
                    reason_code,
                });
            }
        }
    }

    validate_capability_boundaries(dispatch, &adapter_id, capabilities)?;

    Ok(AdapterExecutionPlan {
        adapter_id: adapter_id.clone(),
        adapter_kind: dispatch.adapter,
        required_capability,
        phase: JobLifecyclePhase::Preparing,
        metadata: AdapterExecutionMetadata {
            adapter_id,
            adapter_kind: dispatch.adapter,
            isolation_mode: dispatch.policy.workspace.root_strategy.clone(),
            network_mode: dispatch.policy.network.mode.clone(),
            scoped_credential_refs: dispatch
                .policy
                .workspace
                .scoped_credential_ref
                .clone()
                .into_iter()
                .collect(),
            audit_labels: vec![
                format!("policy:{}", dispatch.policy.policy_version),
                format!("working_group:{}", dispatch.policy.working_group_id),
            ],
        },
    })
}

pub fn validate_adapter_result_phase(phase: JobLifecyclePhase) -> Result<(), AdapterContractError> {
    match phase {
        JobLifecyclePhase::Completed
        | JobLifecyclePhase::Failed
        | JobLifecyclePhase::Cancelled
        | JobLifecyclePhase::TimedOut
        | JobLifecyclePhase::PolicyDenied => Ok(()),
        JobLifecyclePhase::Queued
        | JobLifecyclePhase::Leased
        | JobLifecyclePhase::Preparing
        | JobLifecyclePhase::Running
        | JobLifecyclePhase::Finishing => Err(AdapterContractError::MalformedResult(
            "adapter result must use a terminal lifecycle phase",
        )),
    }
}

fn required_capability(adapter: AdapterKind) -> Option<HighRiskCapability> {
    match adapter {
        AdapterKind::Noop | AdapterKind::Echo => None,
        AdapterKind::OpenAiCompatibleLocalLlm
        | AdapterKind::OllamaLocalLlm
        | AdapterKind::SelfHostedLocalLlm => Some(HighRiskCapability::LocalLlmExposure),
        AdapterKind::CodexAgent | AdapterKind::ClaudeCodeAgent => {
            Some(HighRiskCapability::ExternalAgentRuntimeAdapter)
        }
        AdapterKind::AllowlistedCommand => Some(HighRiskCapability::LocalToolExecution),
    }
}

fn resolve_adapter_id(
    dispatch: &JobDispatch,
    capabilities: &RunnerCapabilities,
) -> Result<String, AdapterContractError> {
    let requested_id = dispatch
        .input
        .get("adapter_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());

    match dispatch.adapter {
        AdapterKind::Noop => Ok("noop".to_string()),
        AdapterKind::Echo => Ok("echo".to_string()),
        AdapterKind::OpenAiCompatibleLocalLlm
        | AdapterKind::OllamaLocalLlm
        | AdapterKind::SelfHostedLocalLlm => capabilities
            .local_llm_endpoints
            .iter()
            .find(|capability| Some(capability.id.as_str()) == requested_id)
            .map(|capability| capability.id.clone())
            .ok_or(AdapterContractError::MissingCapability(dispatch.adapter)),
        AdapterKind::CodexAgent | AdapterKind::ClaudeCodeAgent => capabilities
            .external_agent_adapters
            .iter()
            .find(|capability| Some(capability.id.as_str()) == requested_id)
            .map(|capability| capability.id.clone())
            .ok_or(AdapterContractError::MissingCapability(dispatch.adapter)),
        AdapterKind::AllowlistedCommand => capabilities
            .allowlisted_command_adapters
            .iter()
            .find(|capability| Some(capability.id.as_str()) == requested_id)
            .map(|capability| capability.id.clone())
            .ok_or(AdapterContractError::MissingCapability(dispatch.adapter)),
    }
}

fn validate_capability_boundaries(
    dispatch: &JobDispatch,
    adapter_id: &str,
    capabilities: &RunnerCapabilities,
) -> Result<(), AdapterContractError> {
    match dispatch.adapter {
        AdapterKind::Noop | AdapterKind::Echo => Ok(()),
        AdapterKind::OpenAiCompatibleLocalLlm
        | AdapterKind::OllamaLocalLlm
        | AdapterKind::SelfHostedLocalLlm => {
            let _capability = local_llm_capability(adapter_id, capabilities, dispatch.adapter)?;
            Ok(())
        }
        AdapterKind::CodexAgent | AdapterKind::ClaudeCodeAgent => {
            let capability = external_agent_capability(adapter_id, capabilities, dispatch.adapter)?;
            require_contains(
                &capability.supported_isolation_modes,
                &dispatch.policy.workspace.root_strategy,
                dispatch.adapter,
                "unsupported_isolation_mode",
            )?;
            require_contains(
                &capability.supported_network_modes,
                &dispatch.policy.network.mode,
                dispatch.adapter,
                "unsupported_network_mode",
            )
        }
        AdapterKind::AllowlistedCommand => {
            let capability =
                allowlisted_command_capability(adapter_id, capabilities, dispatch.adapter)?;
            for arg in requested_args(dispatch)? {
                require_contains(
                    &capability.allowed_args,
                    &arg,
                    dispatch.adapter,
                    "command_arg_not_allowlisted",
                )?;
            }
            for env_name in &dispatch.policy.workspace.allowed_env {
                require_contains(
                    &capability.allowed_env,
                    env_name,
                    dispatch.adapter,
                    "env_not_allowlisted",
                )?;
            }
            Ok(())
        }
    }
}

fn local_llm_capability<'a>(
    adapter_id: &str,
    capabilities: &'a RunnerCapabilities,
    adapter_kind: AdapterKind,
) -> Result<&'a LocalLlmEndpointCapability, AdapterContractError> {
    capabilities
        .local_llm_endpoints
        .iter()
        .find(|capability| capability.id == adapter_id)
        .ok_or(AdapterContractError::MissingCapability(adapter_kind))
}

fn external_agent_capability<'a>(
    adapter_id: &str,
    capabilities: &'a RunnerCapabilities,
    adapter_kind: AdapterKind,
) -> Result<&'a ExternalAgentAdapterCapability, AdapterContractError> {
    capabilities
        .external_agent_adapters
        .iter()
        .find(|capability| capability.id == adapter_id)
        .ok_or(AdapterContractError::MissingCapability(adapter_kind))
}

fn allowlisted_command_capability<'a>(
    adapter_id: &str,
    capabilities: &'a RunnerCapabilities,
    adapter_kind: AdapterKind,
) -> Result<&'a AllowlistedCommandAdapterCapability, AdapterContractError> {
    capabilities
        .allowlisted_command_adapters
        .iter()
        .find(|capability| capability.id == adapter_id)
        .ok_or(AdapterContractError::MissingCapability(adapter_kind))
}

fn requested_args(dispatch: &JobDispatch) -> Result<Vec<String>, AdapterContractError> {
    match dispatch.input.get("args") {
        None => Ok(vec![]),
        Some(value) => value
            .as_array()
            .ok_or(AdapterContractError::MalformedResult(
                "allowlisted command args must be an array of strings",
            ))?
            .iter()
            .map(|arg| {
                arg.as_str()
                    .map(str::to_string)
                    .ok_or(AdapterContractError::MalformedResult(
                        "allowlisted command args must be an array of strings",
                    ))
            })
            .collect(),
    }
}

fn require_contains(
    allowed: &[String],
    requested: &str,
    adapter_kind: AdapterKind,
    reason_code: &'static str,
) -> Result<(), AdapterContractError> {
    if allowed.iter().any(|value| value == requested) {
        Ok(())
    } else {
        Err(AdapterContractError::BoundaryDenied {
            adapter_kind,
            reason_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        capabilities::RunnerCapabilities,
        config::{
            AllowlistedCommandAdapterConfig, CapabilityOverrides, EndpointHealthStatus,
            ExternalAgentAdapterConfig, LocalLlmEndpointConfig, RemoteConfig, RuntimeFeatureFlags,
        },
        gates::HighRiskCapability,
        protocol::{AdapterKind, JobDispatch, NetworkPolicy, PolicySnapshot, WorkspacePolicy},
    };
    use serde_json::json;

    use super::*;

    fn dispatch(adapter: AdapterKind, allowed_capabilities: Vec<&str>) -> JobDispatch {
        dispatch_with_input(
            adapter,
            allowed_capabilities,
            json!({ "adapter_id": "ollama-dev" }),
        )
    }

    fn dispatch_with_input(
        adapter: AdapterKind,
        allowed_capabilities: Vec<&str>,
        input: serde_json::Value,
    ) -> JobDispatch {
        JobDispatch {
            lease_id: "lease_runtime_001".to_string(),
            job_id: "job_runtime_001".to_string(),
            runner_id: "runner_runtime_001".to_string(),
            correlation_id: "corr_runtime_001".to_string(),
            idempotency_key: "idem_runtime_001".to_string(),
            adapter,
            input,
            timeout_seconds: 60,
            policy: PolicySnapshot {
                policy_version: "policy_fixture_v1".to_string(),
                working_group_id: "wg-dev".to_string(),
                allowed_capabilities: allowed_capabilities
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                risky_actions: vec![],
                workspace: WorkspacePolicy {
                    workspace_id: "workspace_fixture_001".to_string(),
                    root_strategy: "temporary_isolated".to_string(),
                    allowed_env: vec!["PATH".to_string()],
                    scoped_credential_ref: Some("credential-ref-only".to_string()),
                    artifact_path_allowlist: vec!["stdout.txt".to_string()],
                },
                network: NetworkPolicy {
                    mode: "deny_all".to_string(),
                    allowed_targets: vec![],
                },
            },
        }
    }

    fn capabilities() -> RunnerCapabilities {
        let config = RemoteConfig {
            control_plane_url: "https://taskotter.local".into(),
            runner_id: Some("runner_runtime_001".into()),
            registration_token_env: "TASKOTTER_REMOTE_REGISTRATION_TOKEN".into(),
            credential_placeholder: None,
            labels: vec![],
            max_concurrency: 1,
            allowed_working_groups: vec!["wg-dev".into()],
            env_allowlist: vec![],
            risky_action_allowlist: vec![],
            heartbeat_interval_seconds: 30,
            job_timeout_seconds: 60,
            capability_overrides: CapabilityOverrides {
                network_zone: Some("private".into()),
                local_llm_endpoints: vec![LocalLlmEndpointConfig {
                    id: "ollama-dev".into(),
                    provider: "ollama".into(),
                    base_url: "http://127.0.0.1:11434".into(),
                    model_ids: vec!["llama3.2".into()],
                    health_status: EndpointHealthStatus::Healthy,
                }],
                external_agent_adapters: vec![ExternalAgentAdapterConfig {
                    id: "codex-local".into(),
                    runtime: "codex".into(),
                    command_label: "codex".into(),
                    supported_isolation_modes: vec!["temporary_isolated".into()],
                    supported_network_modes: vec!["deny_all".into()],
                }],
                allowlisted_command_adapters: vec![AllowlistedCommandAdapterConfig {
                    id: "cargo-test".into(),
                    command_label: "cargo".into(),
                    allowed_args: vec!["test".into()],
                    allowed_env: vec!["PATH".into()],
                }],
                local_tools: vec!["cargo".into()],
                mcp_host_modes: vec![],
            },
            feature_flags: RuntimeFeatureFlags::default(),
        };

        RunnerCapabilities::discover(&config)
    }

    #[test]
    fn denies_external_agent_before_dispatch_when_feature_flag_is_disabled() {
        let error = plan_adapter_execution(
            &dispatch_with_input(
                AdapterKind::CodexAgent,
                vec!["remote.external_agent_runtime_adapter"],
                json!({ "adapter_id": "codex-local" }),
            ),
            &capabilities(),
            &RuntimeFeatureFlags::default(),
        )
        .expect_err("external agent adapter must be gated before dispatch");

        assert_eq!(
            error,
            AdapterContractError::PolicyDenied {
                capability: HighRiskCapability::ExternalAgentRuntimeAdapter,
                feature_flag: "remote.external_agent_adapters.enabled",
                reason_code: "feature_flag_disabled",
            }
        );
    }

    #[test]
    fn plans_local_llm_with_zero_cost_usage_audit_boundary() {
        let flags = RuntimeFeatureFlags {
            local_llm_enabled: true,
            ..RuntimeFeatureFlags::default()
        };
        let plan = plan_adapter_execution(
            &dispatch(
                AdapterKind::OllamaLocalLlm,
                vec!["remote.local_llm_exposure"],
            ),
            &capabilities(),
            &flags,
        )
        .expect("local llm should plan when flag and policy allow it");

        assert_eq!(
            plan.required_capability,
            Some(HighRiskCapability::LocalLlmExposure)
        );
        assert_eq!(plan.phase, JobLifecyclePhase::Preparing);
        assert_eq!(plan.metadata.network_mode, "deny_all");
        assert_eq!(
            plan.metadata.scoped_credential_refs,
            vec!["credential-ref-only"]
        );
    }

    #[test]
    fn rejects_bogus_adapter_id_before_dispatch() {
        let flags = RuntimeFeatureFlags {
            local_llm_enabled: true,
            ..RuntimeFeatureFlags::default()
        };
        let error = plan_adapter_execution(
            &dispatch_with_input(
                AdapterKind::OllamaLocalLlm,
                vec!["remote.local_llm_exposure"],
                json!({ "adapter_id": "missing-local-endpoint" }),
            ),
            &capabilities(),
            &flags,
        )
        .expect_err("bogus adapter_id must not plan");

        assert_eq!(
            error,
            AdapterContractError::MissingCapability(AdapterKind::OllamaLocalLlm)
        );
    }

    #[test]
    fn rejects_wrong_kind_adapter_id_before_dispatch() {
        let flags = RuntimeFeatureFlags {
            external_agent_adapters_enabled: true,
            ..RuntimeFeatureFlags::default()
        };
        let error = plan_adapter_execution(
            &dispatch_with_input(
                AdapterKind::CodexAgent,
                vec!["remote.external_agent_runtime_adapter"],
                json!({ "adapter_id": "ollama-dev" }),
            ),
            &capabilities(),
            &flags,
        )
        .expect_err("local llm adapter id must not resolve for codex adapter");

        assert_eq!(
            error,
            AdapterContractError::MissingCapability(AdapterKind::CodexAgent)
        );
    }

    #[test]
    fn rejects_external_agent_unsupported_isolation_and_network_modes() {
        let flags = RuntimeFeatureFlags {
            external_agent_adapters_enabled: true,
            ..RuntimeFeatureFlags::default()
        };
        let mut dispatch = dispatch_with_input(
            AdapterKind::CodexAgent,
            vec!["remote.external_agent_runtime_adapter"],
            json!({ "adapter_id": "codex-local" }),
        );
        dispatch.policy.workspace.root_strategy = "host_workspace".to_string();

        let error = plan_adapter_execution(&dispatch, &capabilities(), &flags)
            .expect_err("unsupported isolation mode must be denied");
        assert_eq!(
            error,
            AdapterContractError::BoundaryDenied {
                adapter_kind: AdapterKind::CodexAgent,
                reason_code: "unsupported_isolation_mode",
            }
        );

        dispatch.policy.workspace.root_strategy = "temporary_isolated".to_string();
        dispatch.policy.network.mode = "public_internet".to_string();
        let error = plan_adapter_execution(&dispatch, &capabilities(), &flags)
            .expect_err("unsupported network mode must be denied");
        assert_eq!(
            error,
            AdapterContractError::BoundaryDenied {
                adapter_kind: AdapterKind::CodexAgent,
                reason_code: "unsupported_network_mode",
            }
        );
    }

    #[test]
    fn rejects_allowlisted_command_unapproved_args_and_env() {
        let flags = RuntimeFeatureFlags {
            local_tools_enabled: true,
            ..RuntimeFeatureFlags::default()
        };
        let mut dispatch = dispatch_with_input(
            AdapterKind::AllowlistedCommand,
            vec!["remote.local_tool_execution"],
            json!({
                "adapter_id": "cargo-test",
                "args": ["run"]
            }),
        );

        let error = plan_adapter_execution(&dispatch, &capabilities(), &flags)
            .expect_err("unapproved command arg must be denied");
        assert_eq!(
            error,
            AdapterContractError::BoundaryDenied {
                adapter_kind: AdapterKind::AllowlistedCommand,
                reason_code: "command_arg_not_allowlisted",
            }
        );

        dispatch.input = json!({
            "adapter_id": "cargo-test",
            "args": ["test"]
        });
        dispatch.policy.workspace.allowed_env = vec!["SECRET_TOKEN".to_string()];
        let error = plan_adapter_execution(&dispatch, &capabilities(), &flags)
            .expect_err("unapproved env must be denied");
        assert_eq!(
            error,
            AdapterContractError::BoundaryDenied {
                adapter_kind: AdapterKind::AllowlistedCommand,
                reason_code: "env_not_allowlisted",
            }
        );
    }

    #[test]
    fn rejects_missing_adapter_capability() {
        let mut runner_capabilities = capabilities();
        runner_capabilities.local_llm_endpoints.clear();
        let flags = RuntimeFeatureFlags {
            local_llm_enabled: true,
            ..RuntimeFeatureFlags::default()
        };

        let error = plan_adapter_execution(
            &dispatch(
                AdapterKind::OpenAiCompatibleLocalLlm,
                vec!["remote.local_llm_exposure"],
            ),
            &runner_capabilities,
            &flags,
        )
        .expect_err("missing local endpoint must fail before dispatch");

        assert_eq!(
            error,
            AdapterContractError::MissingCapability(AdapterKind::OpenAiCompatibleLocalLlm)
        );
    }

    #[test]
    fn rejects_malformed_non_terminal_adapter_result_phase() {
        let error = validate_adapter_result_phase(JobLifecyclePhase::Running)
            .expect_err("adapter result must not be non-terminal");

        assert_eq!(
            error,
            AdapterContractError::MalformedResult(
                "adapter result must use a terminal lifecycle phase"
            )
        );
        assert!(validate_adapter_result_phase(JobLifecyclePhase::Failed).is_ok());
        assert!(validate_adapter_result_phase(JobLifecyclePhase::TimedOut).is_ok());
        assert!(validate_adapter_result_phase(JobLifecyclePhase::Cancelled).is_ok());
        assert!(validate_adapter_result_phase(JobLifecyclePhase::PolicyDenied).is_ok());
    }
}
