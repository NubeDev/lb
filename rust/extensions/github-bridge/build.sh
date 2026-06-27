#!/usr/bin/env bash
# Build the `github-bridge` wasm extension — a pure-transform Tier-1 wasm32-wasip2 component
# (excluded from the workspace, built in its own target/). Produces:
#   target/wasm32-wasip2/release/github_bridge_ext.wasm   (the path crates/host tests load).
set -euo pipefail
cd "$(dirname "$0")"
cargo build --target wasm32-wasip2 --release
echo "built: $(pwd)/target/wasm32-wasip2/release/github_bridge_ext.wasm"
