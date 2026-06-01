use crate::{config::RuntimeFeatureFlags, protocol::PolicySnapshot};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HighRiskCapability {
    LocalToolExecution,
    LocalLlmExposure,
    ExternalAgentRuntimeAdapter,
    ComputerUseExecution,
}

impl HighRiskCapability {
    pub fn contract_name(self) -> &'static str {
        match self {
            Self::LocalToolExecution => "remote.local_tool_execution",
            Self::LocalLlmExposure => "remote.local_llm_exposure",
            Self::ExternalAgentRuntimeAdapter => "remote.external_agent_runtime_adapter",
            Self::ComputerUseExecution => "remote.computer_use_execution",
        }
    }

    pub fn feature_flag(self) -> &'static str {
        match self {
            Self::LocalToolExecution => "remote.local_tools.enabled",
            Self::LocalLlmExposure => "remote.local_llm.enabled",
            Self::ExternalAgentRuntimeAdapter => "remote.external_agent_adapters.enabled",
            Self::ComputerUseExecution => "remote.computer_use.enabled",
        }
    }

    fn flag_enabled(self, flags: &RuntimeFeatureFlags) -> bool {
        match self {
            Self::LocalToolExecution => flags.local_tools_enabled,
            Self::LocalLlmExposure => flags.local_llm_enabled,
            Self::ExternalAgentRuntimeAdapter => flags.external_agent_adapters_enabled,
            Self::ComputerUseExecution => flags.computer_use_enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityGateDecision {
    Allowed,
    Denied {
        capability: HighRiskCapability,
        feature_flag: &'static str,
        reason_code: &'static str,
    },
}

impl CapabilityGateDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

pub fn evaluate_high_risk_capability(
    flags: &RuntimeFeatureFlags,
    policy: &PolicySnapshot,
    capability: HighRiskCapability,
) -> CapabilityGateDecision {
    if !capability.flag_enabled(flags) {
        return CapabilityGateDecision::Denied {
            capability,
            feature_flag: capability.feature_flag(),
            reason_code: "feature_flag_disabled",
        };
    }

    if !policy
        .allowed_capabilities
        .iter()
        .any(|allowed| allowed == capability.contract_name())
    {
        return CapabilityGateDecision::Denied {
            capability,
            feature_flag: capability.feature_flag(),
            reason_code: "policy_capability_not_allowed",
        };
    }

    CapabilityGateDecision::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{NetworkPolicy, WorkspacePolicy};

    fn policy(allowed_capabilities: Vec<&str>) -> PolicySnapshot {
        PolicySnapshot {
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
                allowed_env: vec![],
                scoped_credential_ref: None,
                artifact_path_allowlist: vec![],
            },
            network: NetworkPolicy {
                mode: "deny_all".to_string(),
                allowed_targets: vec![],
            },
        }
    }

    #[test]
    fn denies_high_risk_capability_when_feature_flag_is_disabled() {
        let flags = RuntimeFeatureFlags::default();
        let decision = evaluate_high_risk_capability(
            &flags,
            &policy(vec!["remote.local_tool_execution"]),
            HighRiskCapability::LocalToolExecution,
        );

        assert_eq!(
            decision,
            CapabilityGateDecision::Denied {
                capability: HighRiskCapability::LocalToolExecution,
                feature_flag: "remote.local_tools.enabled",
                reason_code: "feature_flag_disabled",
            }
        );
    }

    #[test]
    fn denies_high_risk_capability_without_policy_allowance() {
        let flags = RuntimeFeatureFlags {
            local_tools_enabled: true,
            ..RuntimeFeatureFlags::default()
        };
        let decision = evaluate_high_risk_capability(
            &flags,
            &policy(vec!["echo"]),
            HighRiskCapability::LocalToolExecution,
        );

        assert_eq!(
            decision,
            CapabilityGateDecision::Denied {
                capability: HighRiskCapability::LocalToolExecution,
                feature_flag: "remote.local_tools.enabled",
                reason_code: "policy_capability_not_allowed",
            }
        );
    }

    #[test]
    fn allows_high_risk_capability_only_with_flag_and_policy() {
        let flags = RuntimeFeatureFlags {
            local_tools_enabled: true,
            ..RuntimeFeatureFlags::default()
        };
        let decision = evaluate_high_risk_capability(
            &flags,
            &policy(vec!["remote.local_tool_execution"]),
            HighRiskCapability::LocalToolExecution,
        );

        assert!(decision.is_allowed());
    }
}
