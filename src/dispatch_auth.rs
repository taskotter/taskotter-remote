use std::collections::{BTreeMap, BTreeSet};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::protocol::{JobDispatch, ProtocolEnvelope, validate_protocol_version};

pub const DISPATCH_SIGNATURE_ALGORITHM: &str = "ed25519";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchSignature {
    pub key_id: String,
    pub algorithm: String,
    pub signed_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
    pub signature_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedDispatchInstruction {
    pub dispatch: ProtocolEnvelope<JobDispatch>,
    pub signature: DispatchSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchSigningContext {
    pub key_id: String,
    pub signed_at_unix_seconds: u64,
    pub expires_at_unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchKeyStatus {
    Active,
    Retired { verify_until_unix_seconds: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchVerificationKey {
    pub key_id: String,
    pub verifying_key: VerifyingKey,
    pub status: DispatchKeyStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispatchKeyRegistry {
    keys: BTreeMap<String, DispatchVerificationKey>,
}

impl DispatchKeyRegistry {
    pub fn new(keys: impl IntoIterator<Item = DispatchVerificationKey>) -> Self {
        Self {
            keys: keys
                .into_iter()
                .map(|key| (key.key_id.clone(), key))
                .collect(),
        }
    }

    pub fn get(&self, key_id: &str) -> Option<&DispatchVerificationKey> {
        self.keys.get(key_id)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchVerificationError {
    #[error("dispatch_signature_missing_required_field")]
    MissingRequiredField(&'static str),
    #[error("dispatch_signature_unsupported_protocol")]
    UnsupportedProtocol,
    #[error("dispatch_signature_unsupported_algorithm")]
    UnsupportedAlgorithm,
    #[error("dispatch_signature_unknown_key")]
    UnknownKey,
    #[error("dispatch_signature_expired")]
    Expired,
    #[error("dispatch_signature_key_retired")]
    KeyRetired,
    #[error("dispatch_signature_invalid")]
    InvalidSignature,
    #[error("dispatch_replay_detected")]
    ReplayDetected,
    #[error("dispatch_scope_mismatch")]
    ScopeMismatch,
    #[error("dispatch_signature_malformed")]
    MalformedSignature,
}

pub trait DispatchReplayCache {
    fn has_seen(&self, replay_key: &str) -> bool;
    fn record(&mut self, replay_key: String);
}

#[derive(Debug, Default)]
pub struct InMemoryDispatchReplayCache {
    seen: BTreeSet<String>,
}

impl DispatchReplayCache for InMemoryDispatchReplayCache {
    fn has_seen(&self, replay_key: &str) -> bool {
        self.seen.contains(replay_key)
    }

    fn record(&mut self, replay_key: String) {
        self.seen.insert(replay_key);
    }
}

pub fn sign_dispatch_instruction(
    dispatch: ProtocolEnvelope<JobDispatch>,
    signing_key: &SigningKey,
    context: DispatchSigningContext,
) -> Result<SignedDispatchInstruction, DispatchVerificationError> {
    validate_required_dispatch_fields(&dispatch.payload)?;

    let mut signature = DispatchSignature {
        key_id: context.key_id,
        algorithm: DISPATCH_SIGNATURE_ALGORITHM.to_string(),
        signed_at_unix_seconds: context.signed_at_unix_seconds,
        expires_at_unix_seconds: context.expires_at_unix_seconds,
        signature_base64: String::new(),
    };
    let canonical_payload = canonical_signing_payload(&dispatch, &signature)?;
    let signed = signing_key.sign(canonical_payload.as_bytes());
    signature.signature_base64 = STANDARD.encode(signed.to_bytes());

    Ok(SignedDispatchInstruction {
        dispatch,
        signature,
    })
}

pub fn verify_dispatch_instruction(
    instruction: &SignedDispatchInstruction,
    registry: &DispatchKeyRegistry,
    replay_cache: &mut impl DispatchReplayCache,
    now_unix_seconds: u64,
    expected_runner_id: &str,
) -> Result<(), DispatchVerificationError> {
    validate_protocol_version(&instruction.dispatch.protocol_version)
        .map_err(|_| DispatchVerificationError::UnsupportedProtocol)?;
    validate_required_dispatch_fields(&instruction.dispatch.payload)?;

    if instruction.dispatch.payload.runner_id != expected_runner_id {
        return Err(DispatchVerificationError::ScopeMismatch);
    }
    if instruction.signature.algorithm != DISPATCH_SIGNATURE_ALGORITHM {
        return Err(DispatchVerificationError::UnsupportedAlgorithm);
    }
    if instruction.signature.expires_at_unix_seconds <= now_unix_seconds {
        return Err(DispatchVerificationError::Expired);
    }

    let key = registry
        .get(&instruction.signature.key_id)
        .ok_or(DispatchVerificationError::UnknownKey)?;
    if let DispatchKeyStatus::Retired {
        verify_until_unix_seconds,
    } = key.status
        && now_unix_seconds > verify_until_unix_seconds
    {
        return Err(DispatchVerificationError::KeyRetired);
    }

    let signature_bytes = STANDARD
        .decode(&instruction.signature.signature_base64)
        .map_err(|_| DispatchVerificationError::MalformedSignature)?;
    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| DispatchVerificationError::MalformedSignature)?;
    let signature = Signature::from_bytes(&signature);
    let canonical_payload =
        canonical_signing_payload(&instruction.dispatch, &instruction.signature)?;

    key.verifying_key
        .verify(canonical_payload.as_bytes(), &signature)
        .map_err(|_| DispatchVerificationError::InvalidSignature)?;

    let replay_key = replay_key(&instruction.dispatch.payload, &instruction.signature.key_id);
    if replay_cache.has_seen(&replay_key) {
        return Err(DispatchVerificationError::ReplayDetected);
    }
    replay_cache.record(replay_key);

    Ok(())
}

pub fn canonical_signing_payload(
    dispatch: &ProtocolEnvelope<JobDispatch>,
    signature: &DispatchSignature,
) -> Result<String, DispatchVerificationError> {
    let value = json!({
        "dispatch": dispatch,
        "signature": {
            "algorithm": signature.algorithm,
            "expires_at_unix_seconds": signature.expires_at_unix_seconds,
            "key_id": signature.key_id,
            "signed_at_unix_seconds": signature.signed_at_unix_seconds
        }
    });
    serde_json::to_string(&sort_json_value(value))
        .map_err(|_| DispatchVerificationError::MalformedSignature)
}

fn validate_required_dispatch_fields(
    dispatch: &JobDispatch,
) -> Result<(), DispatchVerificationError> {
    let required = [
        ("lease_id", dispatch.lease_id.as_str()),
        ("job_id", dispatch.job_id.as_str()),
        ("runner_id", dispatch.runner_id.as_str()),
        ("correlation_id", dispatch.correlation_id.as_str()),
        ("idempotency_key", dispatch.idempotency_key.as_str()),
        ("policy_version", dispatch.policy.policy_version.as_str()),
        (
            "working_group_id",
            dispatch.policy.working_group_id.as_str(),
        ),
        (
            "workspace_id",
            dispatch.policy.workspace.workspace_id.as_str(),
        ),
    ];

    for (field, value) in required {
        if value.trim().is_empty() {
            return Err(DispatchVerificationError::MissingRequiredField(field));
        }
    }

    Ok(())
}

fn replay_key(dispatch: &JobDispatch, key_id: &str) -> String {
    format!(
        "{}:{}:{}:{}",
        dispatch.runner_id, dispatch.job_id, dispatch.idempotency_key, key_id
    )
}

fn sort_json_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json_value).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, sort_json_value(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect::<Map<_, _>>(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNNER_ID: &str = "local-dev-runner";
    const NOW: u64 = 1_800_000_000;
    const ACTIVE_KEY_ID: &str = "fixture-ed25519-active-2026-06";
    const RETIRED_KEY_ID: &str = "fixture-ed25519-previous-2026-05";

    fn active_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn retired_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[11; 32])
    }

    fn registry() -> DispatchKeyRegistry {
        DispatchKeyRegistry::new([
            DispatchVerificationKey {
                key_id: ACTIVE_KEY_ID.to_string(),
                verifying_key: active_signing_key().verifying_key(),
                status: DispatchKeyStatus::Active,
            },
            DispatchVerificationKey {
                key_id: RETIRED_KEY_ID.to_string(),
                verifying_key: retired_signing_key().verifying_key(),
                status: DispatchKeyStatus::Retired {
                    verify_until_unix_seconds: NOW + 600,
                },
            },
        ])
    }

    fn dispatch_fixture() -> ProtocolEnvelope<JobDispatch> {
        serde_json::from_str(include_str!(
            "../fixtures/control-plane/job-dispatch-echo.json"
        ))
        .expect("job dispatch fixture should parse")
    }

    fn signed_fixture() -> SignedDispatchInstruction {
        sign_dispatch_instruction(
            dispatch_fixture(),
            &active_signing_key(),
            DispatchSigningContext {
                key_id: ACTIVE_KEY_ID.to_string(),
                signed_at_unix_seconds: NOW - 10,
                expires_at_unix_seconds: NOW + 120,
            },
        )
        .expect("fixture should sign")
    }

    #[test]
    fn builds_canonical_payload_with_sorted_signature_fields() {
        let instruction = signed_fixture();
        let canonical =
            canonical_signing_payload(&instruction.dispatch, &instruction.signature).unwrap();

        assert!(canonical.contains("\"dispatch\""));
        assert!(canonical.contains("\"key_id\":\"fixture-ed25519-active-2026-06\""));
        assert!(canonical.find("\"algorithm\"").unwrap() < canonical.find("\"key_id\"").unwrap());
        assert!(!canonical.contains("signature_base64"));
    }

    #[test]
    fn signed_dispatch_fixture_contract_declares_required_cases() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../fixtures/control-plane/signed-dispatch-fixtures.json"
        ))
        .expect("signed dispatch fixture contract should parse");
        let cases = fixture["cases"]
            .as_array()
            .expect("signed dispatch fixture cases should be listed");
        let case_names: BTreeSet<&str> = cases
            .iter()
            .map(|case| case["name"].as_str().expect("case should have a name"))
            .collect();

        for expected in ["valid", "expired", "replayed", "wrong-scope", "rotated-key"] {
            assert!(
                case_names.contains(expected),
                "fixture contract should include {expected}"
            );
        }
    }

    #[test]
    fn verifies_valid_signed_dispatch_fixture() {
        let instruction = signed_fixture();
        let mut replay_cache = InMemoryDispatchReplayCache::default();

        verify_dispatch_instruction(&instruction, &registry(), &mut replay_cache, NOW, RUNNER_ID)
            .expect("valid fixture should verify");
    }

    #[test]
    fn rejects_expired_signed_dispatch_fixture() {
        let mut instruction = signed_fixture();
        instruction.signature.expires_at_unix_seconds = NOW - 1;
        let mut replay_cache = InMemoryDispatchReplayCache::default();

        assert_eq!(
            verify_dispatch_instruction(
                &instruction,
                &registry(),
                &mut replay_cache,
                NOW,
                RUNNER_ID,
            ),
            Err(DispatchVerificationError::Expired)
        );
    }

    #[test]
    fn rejects_replayed_signed_dispatch_fixture() {
        let instruction = signed_fixture();
        let mut replay_cache = InMemoryDispatchReplayCache::default();

        verify_dispatch_instruction(&instruction, &registry(), &mut replay_cache, NOW, RUNNER_ID)
            .expect("first verification should record replay key");

        assert_eq!(
            verify_dispatch_instruction(
                &instruction,
                &registry(),
                &mut replay_cache,
                NOW,
                RUNNER_ID,
            ),
            Err(DispatchVerificationError::ReplayDetected)
        );
    }

    #[test]
    fn rejects_wrong_runner_scope_fixture() {
        let instruction = signed_fixture();
        let mut replay_cache = InMemoryDispatchReplayCache::default();

        assert_eq!(
            verify_dispatch_instruction(
                &instruction,
                &registry(),
                &mut replay_cache,
                NOW,
                "different-runner",
            ),
            Err(DispatchVerificationError::ScopeMismatch)
        );
    }

    #[test]
    fn accepts_rotated_key_inside_verify_window() {
        let instruction = sign_dispatch_instruction(
            dispatch_fixture(),
            &retired_signing_key(),
            DispatchSigningContext {
                key_id: RETIRED_KEY_ID.to_string(),
                signed_at_unix_seconds: NOW - 300,
                expires_at_unix_seconds: NOW + 60,
            },
        )
        .expect("rotated fixture should sign");
        let mut replay_cache = InMemoryDispatchReplayCache::default();

        verify_dispatch_instruction(&instruction, &registry(), &mut replay_cache, NOW, RUNNER_ID)
            .expect("retired key should verify inside rotation window");
    }

    #[test]
    fn rejects_rotated_key_after_verify_window() {
        let instruction = sign_dispatch_instruction(
            dispatch_fixture(),
            &retired_signing_key(),
            DispatchSigningContext {
                key_id: RETIRED_KEY_ID.to_string(),
                signed_at_unix_seconds: NOW - 300,
                expires_at_unix_seconds: NOW + 2_000,
            },
        )
        .expect("rotated fixture should sign");
        let mut replay_cache = InMemoryDispatchReplayCache::default();

        assert_eq!(
            verify_dispatch_instruction(
                &instruction,
                &registry(),
                &mut replay_cache,
                NOW + 700,
                RUNNER_ID,
            ),
            Err(DispatchVerificationError::KeyRetired)
        );
    }

    #[test]
    fn rejects_tampered_payload_after_signature() {
        let mut instruction = signed_fixture();
        instruction.dispatch.payload.policy.working_group_id = "wg-other".to_string();
        let mut replay_cache = InMemoryDispatchReplayCache::default();

        assert_eq!(
            verify_dispatch_instruction(
                &instruction,
                &registry(),
                &mut replay_cache,
                NOW,
                RUNNER_ID,
            ),
            Err(DispatchVerificationError::InvalidSignature)
        );
    }
}
