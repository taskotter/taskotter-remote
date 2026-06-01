use crate::protocol::{ArtifactDescriptor, JobDispatch, LogChunk};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobIsolationPlan {
    pub job_id: String,
    pub runner_id: String,
    pub workspace_root_label: String,
    pub artifact_root_label: String,
    pub allowed_env: BTreeMap<String, String>,
    pub scoped_credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedResource {
    pub job_id: String,
    pub kind: ScopedResourceKind,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopedResourceKind {
    Workspace,
    Artifact,
    Credential,
    EnvironmentVariable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupDisposition {
    Removed,
    Quarantined,
    Residual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupEvidence {
    pub job_id: String,
    pub path_label: String,
    pub sensitive: bool,
    pub disposition: CleanupDisposition,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IsolationViolation {
    #[error("resource is owned by another job")]
    CrossJobResourceAccess,
    #[error("environment variable is not allowlisted")]
    EnvironmentVariableNotAllowlisted,
    #[error("credential reference is not scoped to this job")]
    CredentialNotScoped,
    #[error("sensitive residual file was not removed or quarantined: {path_label}")]
    UnsafeSensitiveResidual { path_label: String },
}

pub fn plan_job_isolation(
    dispatch: &JobDispatch,
    ambient_env: &[(String, String)],
) -> JobIsolationPlan {
    let allowed_names: BTreeSet<&str> = dispatch
        .policy
        .workspace
        .allowed_env
        .iter()
        .map(String::as_str)
        .collect();
    let allowed_env = ambient_env
        .iter()
        .filter(|(name, _)| allowed_names.contains(name.as_str()))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();

    JobIsolationPlan {
        job_id: dispatch.job_id.clone(),
        runner_id: dispatch.runner_id.clone(),
        workspace_root_label: format!(
            "jobs/{}/workspaces/{}",
            dispatch.job_id, dispatch.policy.workspace.workspace_id
        ),
        artifact_root_label: format!("jobs/{}/artifacts", dispatch.job_id),
        allowed_env,
        scoped_credential_ref: dispatch.policy.workspace.scoped_credential_ref.clone(),
    }
}

impl JobIsolationPlan {
    pub fn owns(&self, resource: &ScopedResource) -> Result<(), IsolationViolation> {
        if resource.job_id != self.job_id {
            return Err(IsolationViolation::CrossJobResourceAccess);
        }

        match resource.kind {
            ScopedResourceKind::Workspace | ScopedResourceKind::Artifact => Ok(()),
            ScopedResourceKind::EnvironmentVariable => {
                if self.allowed_env.contains_key(&resource.name) {
                    Ok(())
                } else {
                    Err(IsolationViolation::EnvironmentVariableNotAllowlisted)
                }
            }
            ScopedResourceKind::Credential => {
                if self.scoped_credential_ref.as_deref() == Some(resource.name.as_str()) {
                    Ok(())
                } else {
                    Err(IsolationViolation::CredentialNotScoped)
                }
            }
        }
    }
}

pub fn assert_cleanup_safe(evidence: &[CleanupEvidence]) -> Result<(), IsolationViolation> {
    for item in evidence {
        if item.sensitive && item.disposition == CleanupDisposition::Residual {
            return Err(IsolationViolation::UnsafeSensitiveResidual {
                path_label: item.path_label.clone(),
            });
        }
    }

    Ok(())
}

pub fn redact_log_chunk(mut chunk: LogChunk) -> LogChunk {
    let (line, redacted) = redact_secret_shaped_values(&chunk.line);
    chunk.line = line;
    chunk.redacted = chunk.redacted || redacted;
    chunk
}

pub fn redact_artifact_descriptor(mut descriptor: ArtifactDescriptor) -> ArtifactDescriptor {
    let (path_label, _) = redact_secret_shaped_values(&descriptor.path_label);
    descriptor.path_label = path_label;
    descriptor
}

fn redact_secret_shaped_values(input: &str) -> (String, bool) {
    let mut output = String::with_capacity(input.len());
    let mut redacted = false;
    let mut cursor = 0;

    while cursor < input.len() {
        if let Some((start, end)) = find_secret_shaped_substring(&input[cursor..]) {
            output.push_str(&input[cursor..cursor + start]);
            output.push_str("[REDACTED]");
            cursor += end;
            redacted = true;
        } else {
            output.push_str(&input[cursor..]);
            break;
        }
    }

    (output, redacted)
}

fn find_secret_shaped_substring(input: &str) -> Option<(usize, usize)> {
    for (start, _) in input.char_indices() {
        if !SECRET_PREFIXES
            .iter()
            .any(|prefix| input[start..].starts_with(prefix))
        {
            continue;
        }

        let end = input[start..]
            .char_indices()
            .find_map(|(offset, ch)| {
                if is_secret_body_char(ch) {
                    None
                } else {
                    Some(start + offset)
                }
            })
            .unwrap_or(input.len());
        let candidate = &input[start..end];
        let secret_char_count = candidate
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .count();

        if secret_char_count >= MIN_SECRET_ALNUM_CHARS {
            return Some((start, end));
        }
    }

    None
}

const SECRET_PREFIXES: &[&str] = &["sk-", "tok_", "ghp_", "secret_"];
const MIN_SECRET_ALNUM_CHARS: usize = 20;

fn is_secret_body_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AdapterKind, ProtocolEnvelope};

    fn dispatch(job_id: &str, workspace_id: &str, credential_ref: Option<&str>) -> JobDispatch {
        let fixture = include_str!("../fixtures/control-plane/job-dispatch-echo.json");
        let mut envelope: ProtocolEnvelope<JobDispatch> = serde_json::from_str(fixture).unwrap();
        envelope.payload.job_id = job_id.to_string();
        envelope.payload.lease_id = format!("lease_{job_id}");
        envelope.payload.idempotency_key = format!("idem_{job_id}");
        envelope.payload.correlation_id = format!("corr_{job_id}");
        envelope.payload.policy.workspace.workspace_id = workspace_id.to_string();
        envelope.payload.policy.workspace.scoped_credential_ref =
            credential_ref.map(str::to_string);
        envelope.payload
    }

    fn resource(job_id: &str, kind: ScopedResourceKind, name: &str) -> ScopedResource {
        ScopedResource {
            job_id: job_id.to_string(),
            kind,
            name: name.to_string(),
        }
    }

    #[test]
    fn concurrent_jobs_do_not_share_workspace_artifact_or_unique_credential_refs() {
        let first = dispatch("job_one", "workspace_one", Some("cred_one"));
        let second = dispatch("job_two", "workspace_two", Some("cred_two"));
        let first_plan = plan_job_isolation(&first, &[]);
        let second_plan = plan_job_isolation(&second, &[]);

        assert_ne!(
            first_plan.workspace_root_label,
            second_plan.workspace_root_label
        );
        assert_ne!(
            first_plan.artifact_root_label,
            second_plan.artifact_root_label
        );
        assert_ne!(
            first_plan.scoped_credential_ref,
            second_plan.scoped_credential_ref
        );
        // Dispatcher/control-plane must not reuse scoped credential refs across jobs.
        // This contract-level regression keeps that upstream invariant explicit.

        assert_eq!(
            first_plan.owns(&resource(
                "job_two",
                ScopedResourceKind::Workspace,
                &second_plan.workspace_root_label
            )),
            Err(IsolationViolation::CrossJobResourceAccess)
        );
        assert_eq!(
            first_plan.owns(&resource(
                "job_two",
                ScopedResourceKind::Artifact,
                &second_plan.artifact_root_label
            )),
            Err(IsolationViolation::CrossJobResourceAccess)
        );
        assert_eq!(
            first_plan.owns(&resource(
                "job_two",
                ScopedResourceKind::Credential,
                "cred_two"
            )),
            Err(IsolationViolation::CrossJobResourceAccess)
        );
        assert!(
            second_plan
                .owns(&resource(
                    "job_two",
                    ScopedResourceKind::Credential,
                    "cred_two"
                ))
                .is_ok()
        );
    }

    #[test]
    fn only_allowlisted_environment_variables_are_passed_to_job_process() {
        let dispatch = dispatch("job_env", "workspace_env", None);
        let plan = plan_job_isolation(
            &dispatch,
            &[
                ("PATH".to_string(), "/bin".to_string()),
                ("TMPDIR".to_string(), "/tmp/job".to_string()),
                ("AWS_SECRET_ACCESS_KEY".to_string(), "secret".to_string()),
                (
                    "TASKOTTER_REMOTE_REGISTRATION_TOKEN".to_string(),
                    "token".to_string(),
                ),
            ],
        );

        assert_eq!(plan.allowed_env.len(), 2);
        assert_eq!(plan.allowed_env.get("PATH"), Some(&"/bin".to_string()));
        assert_eq!(
            plan.allowed_env.get("TMPDIR"),
            Some(&"/tmp/job".to_string())
        );
        assert!(!plan.allowed_env.contains_key("AWS_SECRET_ACCESS_KEY"));
        assert!(
            !plan
                .allowed_env
                .contains_key("TASKOTTER_REMOTE_REGISTRATION_TOKEN")
        );
        assert_eq!(
            plan.owns(&resource(
                "job_env",
                ScopedResourceKind::EnvironmentVariable,
                "AWS_SECRET_ACCESS_KEY"
            )),
            Err(IsolationViolation::EnvironmentVariableNotAllowlisted)
        );
    }

    #[test]
    fn cancelled_or_failed_cleanup_allows_removed_or_quarantined_sensitive_files_only() {
        assert_cleanup_safe(&[
            CleanupEvidence {
                job_id: "job_cancelled".to_string(),
                path_label: "workspace/token.txt".to_string(),
                sensitive: true,
                disposition: CleanupDisposition::Removed,
            },
            CleanupEvidence {
                job_id: "job_failed".to_string(),
                path_label: "workspace/crash-dump.txt".to_string(),
                sensitive: true,
                disposition: CleanupDisposition::Quarantined,
            },
            CleanupEvidence {
                job_id: "job_failed".to_string(),
                path_label: "workspace/build.log".to_string(),
                sensitive: false,
                disposition: CleanupDisposition::Residual,
            },
        ])
        .expect("removed or quarantined sensitive files are safe");

        assert_eq!(
            assert_cleanup_safe(&[CleanupEvidence {
                job_id: "job_failed".to_string(),
                path_label: "workspace/leftover-token.txt".to_string(),
                sensitive: true,
                disposition: CleanupDisposition::Residual,
            }]),
            Err(IsolationViolation::UnsafeSensitiveResidual {
                path_label: "workspace/leftover-token.txt".to_string(),
            })
        );
    }

    #[test]
    fn fake_secret_shaped_substrings_are_redacted_from_logs_and_artifact_labels() {
        let bare_log = redact_log_chunk(LogChunk {
            job_id: "job_redact".to_string(),
            runner_id: "runner".to_string(),
            sequence: 1,
            stream: "stdout".to_string(),
            redacted: false,
            line: "adapter output sk-fake1234567890abcdef leaked".to_string(),
        });
        let key_value_log = redact_log_chunk(LogChunk {
            job_id: "job_redact".to_string(),
            runner_id: "runner".to_string(),
            sequence: 2,
            stream: "stdout".to_string(),
            redacted: false,
            line: "OPENAI_API_KEY=sk-fake1234567890abcdef".to_string(),
        });
        let json_like_log = redact_log_chunk(LogChunk {
            job_id: "job_redact".to_string(),
            runner_id: "runner".to_string(),
            sequence: 3,
            stream: "stdout".to_string(),
            redacted: false,
            line: r#"{"token":"tok_fake1234567890abcdef"}"#.to_string(),
        });
        let label_like_log = redact_log_chunk(LogChunk {
            job_id: "job_redact".to_string(),
            runner_id: "runner".to_string(),
            sequence: 4,
            stream: "stdout".to_string(),
            redacted: false,
            line: "token:ghp_fake1234567890abcdef".to_string(),
        });
        let artifact_with_extension = redact_artifact_descriptor(ArtifactDescriptor {
            path_label: "stdout tok_fake1234567890abcdef.txt".to_string(),
            content_type: "text/plain".to_string(),
            size_bytes: 1,
            checksum_sha256: "checksum".to_string(),
            retention_policy: "working_group_private".to_string(),
        });
        let artifact_with_path_and_extension = redact_artifact_descriptor(ArtifactDescriptor {
            path_label: "logs/job-redact/sk-fake1234567890abcdef.stderr.log".to_string(),
            content_type: "text/plain".to_string(),
            size_bytes: 1,
            checksum_sha256: "checksum".to_string(),
            retention_policy: "working_group_private".to_string(),
        });

        assert!(bare_log.redacted);
        assert!(key_value_log.redacted);
        assert!(json_like_log.redacted);
        assert!(label_like_log.redacted);
        assert_eq!(bare_log.line, "adapter output [REDACTED] leaked");
        assert_eq!(key_value_log.line, "OPENAI_API_KEY=[REDACTED]");
        assert_eq!(json_like_log.line, r#"{"token":"[REDACTED]"}"#);
        assert_eq!(label_like_log.line, "token:[REDACTED]");
        assert_eq!(artifact_with_extension.path_label, "stdout [REDACTED].txt");
        assert_eq!(
            artifact_with_path_and_extension.path_label,
            "logs/job-redact/[REDACTED].stderr.log"
        );
    }

    #[test]
    fn isolation_plan_is_contract_only_and_does_not_enable_host_execution() {
        let dispatch = dispatch("job_contract", "workspace_contract", None);
        let plan = plan_job_isolation(&dispatch, &[]);

        assert_eq!(dispatch.adapter, AdapterKind::Echo);
        assert!(plan.allowed_env.is_empty());
        assert!(plan.scoped_credential_ref.is_none());
    }
}
