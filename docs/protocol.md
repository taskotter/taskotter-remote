# Runner Protocol

Protocol version: `runner.v1`

This document defines the MVP assumptions for the TaskOtter Remote runner contract.
The Rust types in `src/protocol.rs` are the local scaffold for this version. The
schema in `proto/taskotter/runner/v1/runner.proto` is the intended cross-repository
contract source for streaming transports once the control plane contract is ready.

## Transport Assumptions

- Primary transport: persistent outbound connection from runner to control plane.
- Preferred candidate: gRPC bidirectional streaming.
- Fallback candidates: HTTPS polling or WebSocket.
- The runner should not require inbound ports for normal private-network deployment.

## Security Assumptions

- Runner registration uses a registration token supplied outside source control.
- Job execution uses scoped job credentials or signed dispatch instructions.
- Control-plane policy decisions are authoritative.
- The runner enforces received decisions but does not own product policy.
- No implicit access to global secrets is available to a job.
- Risky local actions require explicit capability and policy allowance.

## Event Surfaces

Every runner event carries:

- `protocol_version`
- `runner_id`
- `sequence`
- `sent_at`
- `event`

Initial event variants:

- `status`: daemon lifecycle, health, and current concurrency.
- `capability_inventory`: host capability advertisement.
- `job_accepted`: job lease acceptance tied to a policy decision.
- `job_progress`: progress message for a job.
- `job_completed`: terminal job result.
- `usage`: metered usage emitted from the execution plane.

## Control-Plane Integration Points

The control plane is expected to provide:

- Runner registration and scoped runner identity.
- Policy decision payloads or signed dispatch instructions.
- Job leases with timeout, cancellation, and workspace boundaries.
- Upload targets for logs, artifacts, and final output.
- Usage and audit ingestion endpoints.

The runner emits usage events but does not calculate billing rules. Usage events
must include enough dimensions for the control plane to apply limits by tenant,
working group, agent, integration, provider, model, workflow, and runner where
those dimensions are available.

## Known Gaps

- Final transport choice is not locked; gRPC streaming is the current default.
- The policy decision schema must be generated or versioned with the control plane.
- Artifact upload signing and retry semantics are not implemented yet.
- Per-job isolation is represented as a boundary in code but not yet enforced.
- Local MCP, local LLM, and external agent runtime adapters are placeholders.
- mTLS or channel-binding details remain open for the next security review.
