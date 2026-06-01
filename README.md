# TaskOtter Remote

TaskOtter Remote is the user-controlled runner and daemon foundation for
TaskOtter. It registers with the control plane, advertises local capabilities,
leases authorized jobs, and reports heartbeat, log, artifact, usage, and outcome
contracts.

This repository currently contains the MVP foundation only. It does not collect
credentials, execute real Codex or Claude Code adapters, host local LLMs, or
perform production deployment.

Frontend clients never connect directly to this runner. The runner initiates
outbound control-plane communication, consumes scoped dispatch instructions, and
only reports execution evidence back to the control plane.

## Local Development

```sh
cargo fmt --check
cargo test
cargo run -- print-capabilities
cargo run -- run-once --config examples/remote.toml
```

## Configuration

Create a TOML config from `examples/remote.toml`.

Key boundaries:

- `control_plane_url` is the outbound control-plane endpoint.
- `runner_id` is optional before registration and stable after registration.
- `registration_token_env` names an environment variable; the token value is not
  stored in config or logs.
- `credential_placeholder` is a scoped credential reference placeholder only.
- `env_allowlist` controls which environment variable names a future job may
  receive.
- `allowed_working_groups` scopes where the runner may accept work.
- `risky_action_allowlist` is a contract placeholder for explicit local action
  approval.

## Protocol Status

Payloads are versioned with `protocol_version = "remote.v1alpha1"`. Transport is
not final; the current implementation models an outbound persistent connection
with HTTPS/WebSocket/gRPC-compatible JSON contracts so the control plane can
align schema names before a concrete transport is selected.

The initial fixture set lives under `fixtures/`:

- `fixtures/control-plane/job-dispatch-echo.json` models a policy-scoped control
  plane dispatch to the echo adapter.
- `fixtures/control-plane/job-dispatch-noop.json` models a no-op dispatch that
  preserves the side-effect-free runner bootstrap regression path.
- `fixtures/control-plane/job-cancel.json` models the cancellation message shape.
- `fixtures/control-plane/signed-dispatch-fixtures.json` documents the
  fixture-level signed dispatch wrapper while the shared runner protocol schema
  is still pending. Unit tests generate deterministic fake Ed25519 keys and
  cover valid, expired, replayed, cross-key re-sign replay, wrong-scope,
  rotated-key, and post-retirement signing behavior.
- `fixtures/runner/job-error-timeout.json` models the timeout error taxonomy
  shape reported by the runner.

`cargo run -- run-once --config examples/remote.toml` consumes the bundled echo
dispatch fixture, applies local config placeholders for workspace, environment,
credential, and risky-action boundaries, then emits deterministic registration,
capability, heartbeat, lease acceptance, progress, log, artifact manifest, usage,
and final-result envelopes. The echo adapter does not execute a shell, inherit
ambient secrets, or use network egress.

`job.usage.report` uses payload schema `remote_usage_report.v1`, which maps to
the control-plane `POST /v1/remote/usage-reports` ingestion boundary. Reports
include `job_id`, `runner_id`, optional `request_id`, optional `correlation_id`,
`status`, runtime resource fields, token counts, and estimated cost in
micro-USD. Token and cost values are zero for the current placeholder runner
because no real provider adapter is executed.

The control-plane generated OpenAPI document is the MVP source of truth for this
schema until a generated shared schema package exists.

`dispatch_auth` is the local signed instruction boundary. It canonicalizes the
dispatch envelope plus signature metadata with sorted JSON fields, verifies
Ed25519 signatures by `key_id`, enforces runner scope, rejects expired
instructions, records dispatch-identity replay keys through a cache trait, and
allows retired keys only when the signature was created before the configured
retirement time plus clock skew and is still inside the verification window. It
intentionally does not include KMS integration, production signing services,
private key storage, or transport selection.

## Runtime Adapter Scaffold

The runner now advertises three adapter capability groups in the capability
snapshot:

- `local_llm_endpoints` for OpenAI-compatible, Ollama-style, or self-hosted
  local endpoint inventory, including health status and model IDs.
- `external_agent_adapters` for Codex-style or Claude Code-style execution
  runtimes that can mutate the job workspace and must be treated as command/file
  execution boundaries rather than ordinary provider LLM calls.
- `allowlisted_command_adapters` for narrow command labels and argument/env
  allowlists.

`adapters::plan_adapter_execution` is the dispatch scaffold. It verifies that
the runner advertised a matching capability and evaluates the high-risk feature
flag plus policy allowance before any adapter execution would start. It returns
audit metadata containing only scoped credential references, isolation mode,
network mode, policy version, and working group labels. It does not pass global
TaskOtter credentials to adapters and does not execute shell commands.

Adapter results use terminal lifecycle phases compatible with the runner job
states: `completed`, `failed`, `cancelled`, `timed_out`, and `policy_denied`.
Local LLM attempts still require `job.usage.report` evidence even when provider
cost is zero.

## Compatibility Checks

`contract-compatibility.json` declares the supported control-plane and runner
protocol versions consumed by this repository. `cargo test
protocol::tests::contract_compatibility_matrix_declares_supported_versions` and
`cargo test protocol::tests::rejects_unsupported_control_plane_protocol_fixture`
are the repo-local compatibility checks used by CI; they keep the supported
`remote.v1alpha1` protocol explicit and fail when an unsupported protocol fixture
is accepted.

## MVP Limitations

- Job execution is a placeholder state transition, not process execution.
- Artifact upload and log streaming emit contract payloads only.
- Remote disable and kill hooks are contract placeholders.
- No production secret handling, installer packaging, hosted runtime, or branch
  merge is included.
