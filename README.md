# TaskOtter Remote

TaskOtter Remote is the runner and daemon foundation for user-controlled execution hosts.
It is intended to run on a machine, server, or private network host and initiate outbound
connections to the TaskOtter control plane.

This repository intentionally keeps public documentation high level. Detailed product
roadmap, policy design, pricing, and enterprise planning live outside this repository.

## Current Scope

- Rust CLI and daemon scaffold.
- Explicit runner protocol assumptions under `docs/protocol.md`.
- Serializable runner event, status, capability, policy decision, and usage surfaces.
- Configuration loading with secret values supplied by environment variables.
- Format, lint, test, and file-size guard baseline.

## Boundaries

The control plane owns identity, authorization, policy decisions, dispatch decisions,
resource state, usage ledgers, and audit history. TaskOtter Remote enforces scoped
instructions from the control plane and reports execution evidence back.

The runner does not copy control-plane schemas. Shared contracts should be generated
or versioned through dedicated artifacts such as OpenAPI, protobuf, or JSON Schema.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
./scripts/check-file-size.sh
```

Run the scaffold locally:

```sh
cargo run -- protocol
cargo run -- capabilities
cargo run -- validate-config --config examples/runner.toml
```

`TASKOTTER_RUNNER_REGISTRATION_TOKEN` is referenced by name in configuration examples.
Do not commit token values.

## License

This repository uses PolyForm Strict License 1.0.0. See `LICENSE`.
