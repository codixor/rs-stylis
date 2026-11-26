#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo test --locked --workspace --no-default-features
cargo test --locked --workspace --features browserslist
cargo check --locked --benches --no-default-features
cargo check --locked --benches --features browserslist
