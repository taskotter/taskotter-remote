#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test protocol::tests::contract_compatibility_matrix_declares_supported_versions
cargo test protocol::tests::deserializes_additive_control_plane_fixture
cargo test protocol::tests::deserializes_https_fallback_fixtures
cargo test protocol::tests::canonical_schema_and_error_taxonomy_snapshots_are_parseable
cargo test protocol::tests::rejects_unsupported_control_plane_protocol_fixture
cargo test protocol::tests::contract_compatibility_matrix_fails_for_missing_fixture
cargo test
