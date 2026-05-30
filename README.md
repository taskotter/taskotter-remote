# TaskOtter Remote

TaskOtter Remote is the user-controlled runner and daemon foundation for
TaskOtter. It registers with the control plane, advertises local capabilities,
leases authorized jobs, and reports heartbeat, log, artifact, usage, and outcome
contracts.

This repository currently contains the MVP foundation only. It does not collect
credentials, execute real Codex or Claude Code adapters, host local LLMs, or
perform production deployment.

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

## MVP Limitations

- Job execution is a placeholder state transition, not process execution.
- Artifact upload and log streaming emit contract payloads only.
- Remote disable and kill hooks are contract placeholders.
- No production secret handling, installer packaging, hosted runtime, or branch
  merge is included.
